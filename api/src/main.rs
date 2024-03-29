use std::collections::HashMap;
use std::{convert::TryInto, path::PathBuf, result::Result};
use std::{fs, sync};

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
use sha2::Digest;
use tokio::time;
use tokio_postgres::{connect, Client, NoTls};

use ehall::{
    CohortMessage, ElectionResults, Meeting, MeetingMessage, NewMeeting, NewTopicMessage,
    ParticipateMeetingMessage, RegisteredMeetingsMessage, ScoreMessage, UserTopic,
    UserTopicsMessage, COHORT_QUORUM,
};

mod chance;
mod cull;

const N_MEETING_TOPIC_WINNERS: usize = 2;
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

const CREATE_DB_ASSETS: [&str; 14] = [
    "
    CREATE or replace FUNCTION n_cohort_peers(uid varchar, mtg bigint) RETURNS table (n bigint) AS $$
    << outerblock >>
    DECLARE
        cgrp bigint;
    BEGIN
        select count(id) as cohort_group into strict cgrp
        from cohort_groups
        where meeting = mtg;
        if not found then
            return query (select 0);
        end if;
    RETURN query (
        select cgrp
    );
    END;
    $$ LANGUAGE plpgsql;
    ",
    "
    CREATE or replace FUNCTION epeers(uid varchar, mtg bigint) RETURNS table (email varchar) AS $$
    << outerblock >>
    DECLARE
        cgrp bigint;
        cht bigint;
    BEGIN
        select id as cohort_group into strict cgrp
        from cohort_groups
        where meeting = mtg;
        select cohort into strict cht
        from cohort_members
        where cohort_group = cgrp and cohort_members.email = uid;
    RETURN query (
        select cohort_members.email
            from cohort_members
        where cohort_group = cgrp and cohort = cht
    );
    END;
    $$ LANGUAGE plpgsql;
    ",
    "
    -- id is not a primary key, so that it's not an error to *try*
    -- to create a cohort_group for a meeting that already has one.
    create table if not exists cohort_groups (
        id bigserial,
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
        cohort bigint not null,
        email varchar (254) not null
    )
    ",
    "
    create table if not exists meeting_topics (
        email varchar (254) not null,
        meeting bigint not null,
        topic bigint not null,
        score integer default 0
    )
    ",
    "
    create unique index if not exists meeting_topics_idx
    on meeting_topics (meeting, email, topic);
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
        email varchar (254) not null,
        voted bool default false
    );
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
    create unique index if not exists user_mtg_attendee_idx
    on meeting_attendees (meeting, email);
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
    let cohorts = chance::cohorts(emails.len(), COHORT_QUORUM).unwrap();
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

async fn n_cohort_peers(client: &Client, meeting_id: i64, email: &str) -> i64 {
    let sql = "select n_cohort_peers($1, $2)";
    let stmt = client.prepare(sql).await.unwrap();
    let rows = client.query(&stmt, &[&email, &meeting_id]).await.unwrap();
    rows[0].get::<_, i64>(0)
}

async fn cohort_for_user(client: &Client, meeting_id: i64, email: &str) -> Option<Vec<String>> {
    if n_cohort_peers(client, meeting_id, email).await == 0 {
        println!("{} has no cohort peers", email);
        None
    } else {
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
}

async fn elected_topics(
    client: &State<sync::Arc<Client>>,
    email: &str,
    meeting_id: i64,
) -> Vec<UserTopic> {
    let sql = "
    select m.email, topic, score, text from
    (
        (select email, topic, score from meeting_topics
            where meeting = $1 and email in (select epeers($2, $1))) as m
        join
        (select topic as text, email, id from user_topics
            where email in (select epeers('Aa345678@foo.com', 16))) u
        on m.topic = u.id
    )
    order by email, topic
    ";
    let stmt = client.prepare(sql).await.unwrap();
    let rows = client.query(&stmt, &[&meeting_id, &email]).await.unwrap();
    let mut scores: HashMap<_, Vec<_>> = HashMap::new();
    for row in rows.into_iter() {
        let email: String = row.get::<_, String>(0);
        let topic: i64 = row.get::<_, i64>(1);
        let score: i32 = row.get::<_, i32>(2);
        let text: String = row.get::<_, String>(3);
        scores
            .entry(email)
            .or_insert_with(Vec::new)
            .push((topic, score, text));
    }
    let mut rankings: Vec<_> = vec![];
    let mut topics: Vec<_> = vec![];
    let mut topic_texts: Vec<String> = vec![];
    for (_email, user_scores) in scores.iter_mut() {
        let user_topics: Vec<_> = user_scores.iter().map(|(topic, _, _)| *topic).collect();
        if topics.is_empty() {
            topics.extend(user_topics);
            topic_texts.extend(
                user_scores
                    .iter()
                    .map(|(_, _, text)| text.clone())
                    .collect::<Vec<String>>(),
            );
        } else {
            // SQL did order by email, topic, so we expect these to be in the same
            // order for every `_email`.
            assert_eq!(user_topics, topics);
        }
        rankings.push(cull::Ranking {
            scores: user_scores
                .iter()
                .map(|(_topic, score, _text)| *score as usize)
                .collect(),
        });
    }
    let result = cull::borda_count(&rankings).unwrap();
    let mut topics: Vec<_> = result
        .into_iter()
        .enumerate()
        .map(|(i, bscore)| UserTopic {
            text: topic_texts[i].clone(),
            id: topics[i] as u32,
            score: bscore as u32,
        })
        .collect();
    topics.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    topics[..N_MEETING_TOPIC_WINNERS].to_vec()
}

#[get("/meeting/<id>/election_results")]
async fn get_election_results(
    client: &State<sync::Arc<Client>>,
    user: User,
    id: u32,
) -> Json<ElectionResults> {
    let cohort = cohort_for_user(client, id as i64, user.email()).await;
    let (topics, cohort, status) = if let Some(mut cohort) = cohort {
        let sql = "
            select email, voted from meeting_attendees
            where meeting = $1 and email in (select epeers($2, $1))
        ";
        let id = id as i64;
        let stmt = client.prepare(sql).await.unwrap();
        let rows = client.query(&stmt, &[&id, &user.email()]).await.unwrap();
        let mut emails: Vec<_> = rows.iter().map(|row| row.get::<_, String>(0)).collect();
        let voted: Vec<_> = rows.iter().map(|row| row.get::<_, bool>(1)).collect();
        if voted.len() != cohort.len() || !voted.iter().all(|v| *v) {
            (None, None, "Cohort voting not finished".to_owned())
        } else {
            cohort.sort();
            emails.sort();
            if cohort != emails {
                (None, None, "Unexpected cohort email mismatch".to_owned())
            } else {
                (
                    Some(elected_topics(client, user.email(), id).await),
                    Some(cohort),
                    "Vote finished".to_owned(),
                )
            }
        }
    } else {
        dbg!("empty cohort for user");
        (None, None, "Empty cohort for user".to_owned())
    };
    let name = meeting_name(client, id).await;
    let url = meeting_url(id, &name, &topics, &cohort);
    ElectionResults {
        meeting_id: id,
        meeting_name: name,
        topics,
        users: cohort,
        meeting_url: url,
        status,
    }
    .into()
}

fn meeting_url(
    meeting_id: u32,
    meeting_name: &str,
    topics: &Option<Vec<UserTopic>>,
    cohort: &Option<Vec<String>>,
) -> String {
    if topics.is_none() || cohort.is_none() {
        return "".to_owned();
    }
    let mut hasher = sha2::Sha256::new();
    hasher.update(format!("{meeting_id}:{meeting_name}:{topics:?}").as_bytes());
    hasher.update(format!(":{cohort:?}").as_bytes());
    format!("https://meet.jit.si/ehallway/{:x}", hasher.finalize())
}

async fn meeting_name(client: &State<sync::Arc<Client>>, meeting_id: u32) -> String {
    let id = meeting_id as i64;
    let sql = "
        select name from meetings where id = $1
    ";
    let stmt = client.prepare(sql).await.unwrap();
    let rows = client.query(&stmt, &[&id]).await.unwrap();
    rows.get(0).unwrap().get::<_, String>(0)
}

#[put("/meeting/<id>/start")]
async fn start_meeting(
    client: &State<sync::Arc<Client>>,
    user: User,
    id: u32,
) -> Json<CohortMessage> {
    let id = id as i64;
    let sql = "
        insert into cohort_groups
        (meeting)
        values
        ($1)
        on conflict (meeting) do nothing
        returning id
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
async fn meeting_register(
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

#[delete("/meeting/<id>/attendees")]
async fn leave_meeting(user: User, client: &State<sync::Arc<Client>>, id: u32) -> Value {
    let identifier = id as i64;
    let sql = "
        delete from meeting_attendees
        where meeting = $1 and email = $2
    ";
    client
        .execute(sql, &[&identifier, &user.email()])
        .await
        .unwrap();
    let sql = "
        delete from meeting_topics
        where meeting = $1 and email = $2
    ";
    client
        .execute(sql, &[&identifier, &user.email()])
        .await
        .unwrap();
    json!({ "left": id })
}

#[post("/meeting/<id>/attendees")]
async fn attend_meeting(user: User, client: &State<sync::Arc<Client>>, id: u32) -> Value {
    let identifier = id as i64;
    let stmt = client
        .prepare(
            "
            insert into meeting_attendees
            (meeting, email)
            values
            ($1, $2)
            on conflict (meeting, email) do nothing
            returning meeting
        ",
        )
        .await
        .unwrap();
    let rows = client
        .query(&stmt, &[&identifier, &user.email()])
        .await
        .unwrap();
    if rows.len() == 1 {
        println!("inserted meeting attendees");
        let sql = "
        insert into meeting_topics
        (email, meeting, topic, score)
        (
            select $2 as email, $1 as meeting, id as topic, (row_number() over (order by random()) - 1) as score
            from
                (select row_number()
                    over (partition by email order by score desc)
                as r, t.* from user_topics t
                    where t.email in
                        (select distinct email from meeting_attendees
                            where meeting = $1)
                ) x
            where x.r <= 3
            order by random()
        ) on conflict (email, meeting, topic) do nothing
        ";
        client
            .execute(sql, &[&identifier, &user.email()])
            .await
            .unwrap();
    } else {
        println!("inserted no meeting attendees with {} rows", rows.len());
    }
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

#[put("/meeting/<meeting_id>/vote")]
async fn vote_for_meeting_topics(
    user: User,
    client: &State<sync::Arc<Client>>,
    meeting_id: u32,
) -> Value {
    let m_id = meeting_id as i64;
    let sql = "
        update meeting_attendees
        set voted = true
        where meeting = $1 and email = $2
    ";
    client.execute(sql, &[&m_id, &user.email()]).await.unwrap();
    json!({ "voted": meeting_id })
}

#[put(
    "/meeting/<meeting_id>/topic/<topic_id>/score",
    format = "json",
    data = "<score_msg>"
)]
async fn store_meeting_topic_score(
    user: User,
    client: &State<sync::Arc<Client>>,
    meeting_id: u32,
    topic_id: u32,
    score_msg: Json<ScoreMessage>,
) -> Value {
    let m_id = meeting_id as i64;
    let t_id = topic_id as i64;
    let score = score_msg.score as i32;
    client
        .execute(
            "insert into meeting_topics
                (meeting, email, topic, score)
                values
                ($1, $2, $3, $4)
            on conflict (meeting, email, topic) do update
                set score = excluded.score
            ",
            &[&m_id, &user.email(), &t_id, &score],
        )
        .await
        .unwrap();
    json!({ "stored": score })
}

#[put("/topic/<topic_id>/score", format = "json", data = "<score_msg>")]
async fn store_user_topic_score(
    user: User,
    client: &State<sync::Arc<Client>>,
    topic_id: u32,
    score_msg: Json<ScoreMessage>,
) -> Value {
    let t_id = topic_id as i64;
    let score = score_msg.score as i32;
    client
        .execute(
            "update user_topics
             set score = $3
             where email = $1 and id = $2
            ",
            &[&user.email(), &t_id, &score],
        )
        .await
        .unwrap();
    json!({ "stored": score })
}

const GET_SCORED_MEETINGS: &str = "
    select
        meetings.name,
        meetings.id,
        coalesce(meeting_scores.score,0) as score,
        coalesce(r.n_registered,0) as n_registered,
        coalesce(a.n_attending,0) as n_attending
    from meetings
    left outer join meeting_scores on meetings.id = meeting_scores.meeting
    left join (
        select meeting, count(email) as n_registered
        from meeting_participants
        group by meeting
    ) r on meetings.id = r.meeting
    left join (
        select meeting, count(email) as n_attending
        from meeting_attendees
        group by meeting
    ) a on meetings.id = a.meeting;
";

async fn get_meeting_topics_vec(
    client: &State<sync::Arc<Client>>,
    email: &str,
    meeting: i64,
) -> Vec<UserTopic> {
    if n_cohort_peers(client, meeting, email).await == 0 {
        println!("XXXdebug: no cohort peers, so no topics");
        return vec![];
    }
    let sql = "
        select topic as text, m.id, m.score from user_topics u
        right join
        (select topic as id, score from meeting_topics
        where meeting = $1 and meeting_topics.topic in (
            select id from user_topics
            where email in (select epeers($2, $1))
        )) m
        on u.id = m.id;
    ";
    let stmt = client.prepare(sql).await.unwrap();
    let rows = client.query(&stmt, &[&meeting, &email]).await.unwrap();
    rows.into_iter()
        .map(|row| UserTopic {
            text: row.get::<_, String>(0),
            score: row.get::<_, i32>(2) as u32,
            id: row.get::<_, i64>(1) as u32,
        })
        .collect()
}

#[get("/meeting/<id>/topics")]
async fn get_meeting_topics(
    user: User,
    client: &State<sync::Arc<Client>>,
    id: u32,
) -> Json<UserTopicsMessage> {
    UserTopicsMessage {
        topics: get_meeting_topics_vec(client, user.email(), id as i64).await,
    }
    .into()
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
async fn get_meetings(_user: User, client: &State<sync::Arc<Client>>) -> Value {
    let stmt = client.prepare(GET_SCORED_MEETINGS).await.unwrap();
    let rows = client.query(&stmt, &[]).await.unwrap();
    let meetings: Vec<_> = rows
        .iter()
        .map(|row| {
            let name = row.get::<_, String>(0);
            let id = row.get::<_, i64>(1);
            let score = row.get::<_, i32>(2);
            let n_registered = row.get::<_, i64>(3);
            let n_attending = row.get::<_, i64>(4);
            assert_eq!(id as u32 as i64, id); // XXX: later maybe stringify this ID
            MeetingMessage {
                meeting: Meeting {
                    name,
                    id: id as u32,
                    n_registered: n_registered as u32,
                    n_joined: n_attending as u32,
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
            client.execute(sql, &[]).await?;
        }
    }
    let ignited = rocket::build()
        .mount(
            "/",
            routes![
                add_new_meeting,
                add_new_topic,
                attend_meeting,
                delete,
                delete_meeting,
                delete_topic,
                get_meeting_topics,
                get_meetings,
                get_registered_meetings,
                get_user_topics,
                get_user_id,
                get_login,
                get_election_results,
                get_signup,
                index,
                leave_meeting,
                logout,
                meeting_register,
                post_login,
                post_signup,
                start_meeting,
                store_meeting_score,
                store_meeting_topic_score,
                store_user_topic_score,
                show_all_users,
                vote_for_meeting_topics
            ],
        )
        .mount("/", FileServer::from(config.static_path))
        .manage(client)
        .manage(users)
        .attach(Template::fairing())
        .ignite()
        .await;
    match ignited {
        Ok(ignited) => {
            let _app = ignited.launch().await?;
        }
        Err(e) => {
            if let rocket::error::ErrorKind::Collisions(c) = e.kind() {
                println!("collisions:{:?}", c);
            }
            return Err(e.into());
        }
    }
    Ok(())
}
