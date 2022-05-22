use std::{borrow::Cow, fs};

use anyhow::Context;
use clap::Parser;
use rocket::fs::FileServer;
use rocket::serde::{
    json::{Json, Value},
    {Deserialize, Serialize},
};
use rocket::{delete, form::*, get, post, put, response::Redirect, routes, State};
use rocket_auth::{prelude::Error, *};
use rocket_dyn_templates::Template;
use serde_json::json;
use std::*;
use std::{convert::TryInto, path::PathBuf, result::Result};
use tokio_postgres::{connect, Client, NoTls};

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

const CREATE_TABLES: [&str; 4] = [
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
        name varchar (254) not null,
        id bigserial primary key
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
];

const NEW_MEETING: &str = "
    insert into meetings (name)
    values ($1);
";

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct NewMeeting<'r> {
    name: Cow<'r, str>,
}

const NEW_TOPIC: &str = "
    insert into user_topics (email, topic)
    values ($1, $2);
";

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct NewTopic<'r> {
    new_topic: Cow<'r, str>,
}

#[post("/meetings", data = "<meeting>", format = "json")]
async fn add_new_meeting(
    client: &State<sync::Arc<Client>>,
    _user: User,
    meeting: Json<NewMeeting<'_>>,
) -> Result<Value, Error> {
    client.execute(NEW_MEETING, &[&meeting.name]).await?;
    Ok(json!({"inserted": true}))
}

#[post("/topics", data = "<topic>", format = "json")]
async fn add_new_topic(
    client: &State<sync::Arc<Client>>,
    user: User,
    topic: Json<NewTopic<'_>>,
) -> Result<Value, Error> {
    client
        .execute(NEW_TOPIC, &[&user.email(), &topic.new_topic])
        .await?;
    Ok(json!({"inserted": true}))
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

#[derive(Serialize)]
struct Meeting {
    name: String,
    id: u32,
}

#[derive(Serialize)]
struct MeetingMessage {
    meeting: Meeting,
    score: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ScoreMessage {
    score: u32,
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
async fn get_user_topics(user: User, client: &State<sync::Arc<Client>>) -> Value {
    let stmt = client
        .prepare("select topic, id from user_topics where email = $1")
        .await
        .unwrap();
    let rows = client.query(&stmt, &[&user.email()]).await.unwrap();
    let topics: Vec<_> = rows
        .iter()
        .map(|row| {
            let text = row.get::<_, String>(0);
            let id = row.get::<_, i64>(1);
            assert_eq!(id as u32 as i64, id); // XXX: later maybe stringify this ID
            (text, id as u32)
        })
        .collect();
    json!({ "topics": topics })
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
        for sql in CREATE_TABLES {
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
                delete_meeting,
                delete_topic,
                get_meetings,
                get_user_topics,
                get_user_id,
                get_login,
                post_signup,
                get_signup,
                logout,
                post_login,
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
