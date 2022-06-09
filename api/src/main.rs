use std::fs;

use anyhow::Context;
use clap::Parser;
use rand::Rng;
use rocket::fs::FileServer;
use rocket::serde::{
    json::{Json, Value},
    Deserialize,
};
use rocket::{delete, form::*, get, post, put, response::Redirect, routes, State};
use rocket_auth::{prelude::Error, *};
use rocket_dyn_templates::Template;
use serde_json::json;
use std::*;
use std::{convert::TryInto, path::PathBuf, result::Result};
use tokio::time;
use tokio_postgres::{connect, Client, NoTls};

use ehall::{
    CohortMessage, Meeting, MeetingMessage, MeetingParticipantsMessage, NewMeeting,
    NewTopicMessage, ParticipateMeetingMessage, RegisteredMeetingsMessage, ScoreMessage, UserTopic,
    UserTopicsMessage,
};

mod chance;

const N_RETRIES: usize = 10;
const RETRY_SLEEP_MS: u64 = 100;

#[derive(Deserialize)]
struct Config {
    static_path: String,
    postgres_user: String,
    postgres_password: String,
}

#[derive(Parser)]
struct Cli {
    #[clap(long, value_name = "FILE")]
    config_file: PathBuf,
}

#[get("/login")]
fn get_login() -> Template {
    Template::render("login", json!({}))
}

#[post("/login", data = "<form>")]
async fn post_login(auth: Auth<'_>, form: Form<Login>) -> Result<Redirect, Error> {
    let result = auth.login(&form).await;
    println!("login attempt: {:?}", result);
    result?;
    Ok(Redirect::to("/"))
}

#[get("/signup")]
async fn get_signup() -> Template {
    Template::render("signup", json!({}))
}

#[post("/signup", data = "<form>")]
async fn post_signup(auth: Auth<'_>, form: Form<Signup>) -> Result<Redirect, Error> {
    auth.signup(&form).await?;
    auth.login(&form.into()).await?;

    Ok(Redirect::to("/"))
}

#[get("/")]
async fn index(user: Option<User>) -> Template {
    Template::render("index", json!({ "user": user }))
}

#[get("/logout")]
fn logout(auth: Auth<'_>) -> Result<Template, Error> {
    auth.logout()?;
    Ok(Template::render("logout", json!({})))
}

#[get("/delete")]
async fn delete(auth: Auth<'_>) -> Result<Template, Error> {
    auth.delete().await?;
    Ok(Template::render("deleted", json!({})))
}

const CREATE_DB_ASSETS: [&str; 12] = [
    "
    drop function if exists epeers
    ",
    "
    CREATE FUNCTION epeers(uid varchar, mtg bigint) RETURNS table (email varchar) AS $$
    << outerblock >>
    DECLARE
        cgrp bigint;
        cht bigint;
    BEGIN
        select cohort_group into strict cgrp
        from testtbl1
    where meeting = mtg;
        select cohort into strict cht
        from testtbl2
    where cohort_group = cgrp and testtbl2.email = uid;
    RETURN query (
        select testtbl2.email
            from testtbl2
        where cohort_group = cgrp and cohort = cht
    );
    END;
    $$ LANGUAGE plpgsql;
    ",
    "
    create table if not exists cohort_groups (
        id bigserial primary key,
        meeting bigint not null
    );
    ",
    "
    create unique index if not exists cohort_groups_meeting_idx
    on cohort_groups (meeting);
    ",
    "
    create table if not exists cohort_members (
        cohort_group bigint not null,
        cohort integer not null,
        email varchar (254) not null
    )
    ",
    "
    create table if not exists user_topics (
        email varchar (254) not null,
        topic varchar (254) not null,
        id bigserial primary key,
        score integer default 0
    );
    ",
    "
    create table if not exists meetings (
        name varchar (254) primary key,
        id bigserial
    );
    ",
    "
    create table if not exists meeting_attendees (
        meeting bigint not null,
        email varchar (254) not null
    );
    ",
    "
    create unique index if not exists user_mtg_attendee_idx
    on meeting_attendees (meeting, email);
    ",
    "
    create table if not exists meeting_participants (
        meeting bigint not null,
        email varchar (254) not null
    );
    ",
    "
    create table if not exists meeting_scores (
        meeting bigint not null,
        email varchar (254) not null,
        score integer default 0
    );
    ",
    "
    create unique index if not exists user_mtg_score_idx
    on meeting_scores (meeting, email);
    ",
];

const NEW_TOPIC: &str = "
    insert into user_topics (email, topic)
    values ($1, $2)
    returning id;
";

const NEW_MEETING: &str = "
    insert into meetings (name)
    values ($1)
    returning id;
";

async fn store_cohorts_for_group(client: &Client, cohort_group: i64, meeting_id: i64) {
    let sql = "
        select (email) from meeting_attendees
        where meeting = $1
    ";
    let stmt = client.prepare(sql).await.unwrap();
    let emails: Vec<String> = client
        .query(&stmt, &[&meeting_id])
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect();
    let cohorts = chance::cohorts(emails.len(), 3).unwrap();
    let cohort_rows: Vec<_> = cohorts
        .into_iter()
        .enumerate()
        .flat_map(|(cohort_id, members)| {
            members
                .into_iter()
                .zip(std::iter::repeat(cohort_id))
                .map(|(email_idx, cohort_id)| {
                    let cohort_id = cohort_id as i64;
                    (cohort_id, &emails[email_idx])
                })
        })
        .collect();
    let sql = "
        insert into cohort_members
            (cohort_group, cohort, email)
        values
            ($1, $2, $3)
    ";
    for (cohort, email) in cohort_rows {
        client
            .execute(sql, &[&cohort_group, &cohort, &email])
            .await
            .unwrap();
    }
}

async fn cohort_for_user(client: &Client, meeting_id: i64, email: &str) -> Option<Vec<String>> {
    let sql = "
        select epeers($1, $2)
    ";
    let stmt = client.prepare(sql).await.unwrap();
    for _ in 0..N_RETRIES {
        let rows = client.query(&stmt, &[&email, &meeting_id]).await.unwrap();
        if !rows.is_empty() {
            return Some(rows.iter().map(|row| row.get::<_, String>(0)).collect());
        }
        // Use randomness to disperse timings (overkill, but fun)
        let sleep_ms = RETRY_SLEEP_MS + rand::thread_rng().gen_range(0..20);
        time::sleep(time::Duration::from_millis(sleep_ms)).await;
    }
    None
}

#[put("/meeting/<id>/start")]
async fn start_meeting(
    client: &State<sync::Arc<Client>>,
    user: User,
    id: u32,
) -> Json<CohortMessage> {
    let id = id as i64;
    let sql = "
        begin transaction serializable;
        insert into cohort_groups
            (meeting)
            values
            ($1)
        on conflict (meeting) do nothing
        returning id;
        commit
    ";
    let stmt = client.prepare(sql).await.unwrap();
    let rows = client.query(&stmt, &[&id]).await.unwrap();
    if rows.len() == 1 {
        let cohort_group = rows[0].get::<_, i64>(0);
        store_cohorts_for_group(client, cohort_group, id).await;
        eprintln!("created");
    } else {
        eprintln!("not created");
    }
    CohortMessage {
        cohort: cohort_for_user(client, id, user.email()).await,
    }
    .into()
}

#[post("/meeting/<id>/participants", data = "<msg>", format = "json")]
async fn meeting_participate(
    client: &State<sync::Arc<Client>>,
    user: User,
    id: u32,
    msg: Json<ParticipateMeetingMessage>,
) -> Result<Value, Error> {
    eprintln!(
        "meeting {id} user {} participate? {}",
        user.email(),
        msg.participate
    );
    let sql = if msg.participate {
        "
        insert into meeting_participants
        (meeting, email) values
        ($1, $2) on conflict do nothing
        "
    } else {
        "
        delete from meeting_participants
        where email = $2 and meeting = $1
        "
    };
    let id = id as i64;
    client.execute(sql, &[&id, &user.email()]).await.unwrap();
    Ok(json!({ "updated_meeting": id }))
}

#[post("/meetings", data = "<meeting>", format = "json")]
async fn add_new_meeting(
    client: &State<sync::Arc<Client>>,
    user: User,
    meeting: Json<NewMeeting<'_>>,
) -> Result<Value, Error> {
    let stmt = client.prepare(NEW_MEETING).await?;
    let rows = client.query(&stmt, &[&meeting.name]).await?;
    let id = rows[0].get::<_, i64>(0);
    println!("new meeting {} with id {id}", &meeting.name);
    let sql = "
        insert into meeting_scores (meeting, email, score)
        values ($1, $2::varchar,
            (select 1 +
                (select coalesce(max(score), -1) as score
                    from meeting_scores where email = $2
                )
            )
        );
    ";
    // XXXdebug: remove unwrap when done debugging.
    client.execute(sql, &[&id, &user.email()]).await.unwrap();
    Ok(json!({ "inserted": id as u32 }))
}

#[post("/topics", data = "<topic>", format = "json")]
async fn add_new_topic(
    client: &State<sync::Arc<Client>>,
    user: User,
    topic: Json<NewTopicMessage>,
) -> Result<Value, Error> {
    let stmt = client.prepare(NEW_TOPIC).await?;
    let rows = client
        .query(&stmt, &[&user.email(), &topic.new_topic])
        .await?;
    let id = rows[0].get::<_, i64>(0);
    println!("new topic {} with id {id}", &topic.new_topic);
    let sql = "
        update user_topics
            set score = (
                select 1 + coalesce(max(score), -1)
                from user_topics where email = $2
            )
            where id = $1;
    ";
    client.execute(sql, &[&id, &user.email()]).await?;
    Ok(json!({ "inserted": id as u32 }))
}

#[post("/meeting/<id>/attendees")]
async fn attend_meeting(user: User, client: &State<sync::Arc<Client>>, id: u32) -> Value {
    let identifier = id as i64;
    client
        .execute(
            "
            insert into meeting_attendees
            (meeting, email)
            values
            ($1, $2)
            on conflict (meeting, email) do nothing
        ",
            &[&identifier, &user.email()],
        )
        .await
        .unwrap();
    json!({ "attending": id })
}

#[delete("/meetings/<id>")]
async fn delete_meeting(_user: User, client: &State<sync::Arc<Client>>, id: u32) -> Value {
    let identifier = id as i64;
    client
        .execute("delete from meetings where id = $1", &[&identifier])
        .await
        .unwrap();
    json!({ "deleted": id })
}

#[delete("/topics/<id>")]
async fn delete_topic(user: User, client: &State<sync::Arc<Client>>, id: u32) -> Value {
    let identifier = id as i64;
    client
        .execute(
            "delete from user_topics where id = $1 and email = $2",
            &[&identifier, &user.email()],
        )
        .await
        .unwrap();
    json!({ "deleted": id })
}

#[put("/meeting/<id>/score", format = "json", data = "<score_msg>")]
async fn store_meeting_score(
    user: User,
    client: &State<sync::Arc<Client>>,
    id: u32,
    score_msg: Json<ScoreMessage>,
) -> Value {
    let identifier = id as i64;
    let score = score_msg.score as i32;
    client
        .execute(
            "insert into meeting_scores
                (meeting, email, score)
                values
                ($1, $2, $3)
            on conflict (meeting, email) do update
                set score = excluded.score
            ",
            &[&identifier, &user.email(), &score],
        )
        .await
        .unwrap();
    json!({ "stored": score })
}

const GET_SCORED_MEETINGS: &str = "
    select meetings.name, meetings.id, coalesce(score,0) as score
    from meetings left outer join
        (select id, score
            from meetings left outer join meeting_scores
            on meetings.id = meeting_scores.meeting
            where email = $1) q
    on meetings.id = q.id;
";

#[get("/meeting/<id>/participant_counts")]
async fn get_meeting_participants(
    _user: User,
    client: &State<sync::Arc<Client>>,
    id: u32,
) -> Json<MeetingParticipantsMessage> {
    let sql = "
        select (
            select count(*) from meeting_attendees
            where meeting = $1
        ) as n_joined,
        (select count(*) from meeting_participants
            where meeting = $1
        ) as n_registered
    ";
    let id = id as i64;
    let stmt = client.prepare(sql).await.unwrap();
    let rows = client.query(&stmt, &[&id]).await.unwrap();
    let row = rows.get(0).unwrap();
    let n_joined = row.get::<_, i64>(0);
    let n_registered = row.get::<_, i64>(1);
    MeetingParticipantsMessage {
        n_joined: n_joined as u32,
        n_registered: n_registered as u32,
    }
    .into()
}

#[get("/meeting/<id>/topics")]
async fn get_meeting_topics(
    _user: User,
    client: &State<sync::Arc<Client>>,
    id: u32,
) -> Json<UserTopicsMessage> {
    let id = id as i64;
    let stmt = client
        .prepare(
            "
            select topic, id, 0 from
                (select row_number()
                    over (partition by email order by score desc)
                as r, t.* from user_topics t
                    where t.email in
                        (select distinct email from meeting_attendees
                            where meeting = $1)
                ) x
            where x.r <= 3
            order by random()
            ",
        )
        .await
        .unwrap();
    let rows = client.query(&stmt, &[&id]).await.unwrap();
    let topics: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let text = row.get::<_, String>(0);
            let id = row.get::<_, i64>(1);
            let score = i as u32;
            assert_eq!(id as u32 as i64, id); // XXX: later maybe stringify this ID
            UserTopic {
                text,
                score,
                id: id as u32,
            }
        })
        .collect();
    UserTopicsMessage { topics }.into()
}

#[get("/registered_meetings")]
async fn get_registered_meetings(
    user: User,
    client: &State<sync::Arc<Client>>,
) -> Json<RegisteredMeetingsMessage> {
    let stmt = client
        .prepare(
            "
        select meeting from meeting_participants
        where email = $1
    ",
        )
        .await
        .unwrap();
    let rows = client.query(&stmt, &[&user.email()]).await.unwrap();
    let meetings: Vec<_> = rows
        .iter()
        .map(|row| {
            let id = row.get::<_, i64>(0);
            assert_eq!(id as u32 as i64, id); // XXX: later maybe stringify this ID
            id as u32
        })
        .collect();
    RegisteredMeetingsMessage { meetings }.into()
}

#[get("/meetings")]
async fn get_meetings(user: User, client: &State<sync::Arc<Client>>) -> Value {
    let stmt = client.prepare(GET_SCORED_MEETINGS).await.unwrap();
    let rows = client.query(&stmt, &[&user.email()]).await.unwrap();
    let meetings: Vec<_> = rows
        .iter()
        .map(|row| {
            let name = row.get::<_, String>(0);
            let id = row.get::<_, i64>(1);
            let score = row.get::<_, i32>(2);
            assert_eq!(id as u32 as i64, id); // XXX: later maybe stringify this ID
            MeetingMessage {
                meeting: Meeting {
                    name,
                    id: id as u32,
                },
                score: score as u32,
            }
        })
        .collect();
    json!({ "meetings": meetings })
}

#[get("/user_topics")]
async fn get_user_topics(user: User, client: &State<sync::Arc<Client>>) -> Json<UserTopicsMessage> {
    let stmt = client
        .prepare(
            "
            select topic, id, score from user_topics where email = $1
        ",
        )
        .await
        .unwrap();
    let rows = client.query(&stmt, &[&user.email()]).await.unwrap();
    let topics: Vec<_> = rows
        .iter()
        .map(|row| {
            let text = row.get::<_, String>(0);
            let id = row.get::<_, i64>(1);
            let score = row.get::<_, i32>(2);
            assert_eq!(id as u32 as i64, id); // XXX: later maybe stringify this ID
            UserTopic {
                text,
                score: score as u32,
                id: id as u32,
            }
        })
        .collect();
    UserTopicsMessage { topics }.into()
}

#[get("/user_id")]
async fn get_user_id(user: User) -> Value {
    json!({ "email": &(*user.email()) })
}

#[get("/show_all_users")]
async fn show_all_users(
    client: &State<sync::Arc<Client>>,
    user: Option<User>,
) -> Result<Template, Error> {
    let users: Vec<User> = client
        .query("select * from users;", &[])
        .await?
        .into_iter()
        .flat_map(TryInto::try_into)
        .collect();

    Ok(Template::render(
        "users",
        json!({"users": users, "user": user}),
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    println!("reading config file: {}", cli.config_file.display());

    let config: Config =
        toml::from_str(&fs::read_to_string(cli.config_file).context("reading config file")?)
            .context("parsing TOML config")?;
    let (client, conn) = connect(
        &format!(
            "host=localhost user={} password={}",
            config.postgres_user, config.postgres_password
        ),
        NoTls,
    )
    .await?;
    let client = sync::Arc::new(client);
    let users: Users = client.clone().into();

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("TokioPostgresError: {}", e);
        }
    });
    users.create_table().await?;
    {
        let client = client.clone();
        for sql in CREATE_DB_ASSETS {
            print!("{sql}");
            client.execute(sql, &[]).await?;
        }
    }
    let _app = rocket::build()
        .mount(
            "/",
            routes![
                index,
                add_new_meeting,
                add_new_topic,
                attend_meeting,
                delete_meeting,
                delete_topic,
                get_meeting_participants,
                get_meeting_topics,
                get_meetings,
                get_registered_meetings,
                get_user_topics,
                get_user_id,
                get_login,
                meeting_participate,
                post_signup,
                get_signup,
                logout,
                post_login,
                start_meeting,
                store_meeting_score,
                delete,
                show_all_users
            ],
        )
        .mount("/", FileServer::from(config.static_path))
        .manage(client)
        .manage(users)
        .attach(Template::fairing())
        .ignite()
        .await?
        .launch()
        .await?;

    Ok(())
}
