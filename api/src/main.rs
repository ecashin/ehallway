use clap::Parser;
use rocket::fs::FileServer;
use rocket::{form::*, get, post, response::Redirect, routes, State};
use rocket::serde::json::Value;
use rocket_auth::{prelude::Error, *};
use rocket_dyn_templates::Template;
use serde_json::json;
use std::*;
use std::{convert::TryInto, path::PathBuf, result::Result};
use tokio_postgres::{connect, Client};

#[derive(Parser)]
struct Cli {
    #[clap(long, value_name = "DIRECTORY")]
    static_path: PathBuf,
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
async fn post_signup(
    auth: Auth<'_>,
    client: &State<sync::Arc<Client>>,
    form: Form<Signup>,
) -> Result<Redirect, Error> {
    auth.signup(&form).await?;
    let login: rocket_auth::Login = form.clone().into();
    for sql in [USER_VAL_SETUP] {
        client.execute(sql, &[&login.email]).await?;
    }
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

const CREATE_TABLES: [&str; 2] = [
    "
    CREATE TABLE IF NOT EXISTS user_value (
        email VARCHAR (254) UNIQUE NOT NULL primary key,
        value integer DEFAULT 0
    );
    ",
    "
    create table if not exists user_topics (
        email varchar (254) not null,
        topic varchar (254) not null,
        score integer default 0
    );
    ",
];

const USER_VAL_SETUP: &str = "
    insert into user_value (email) values ($1)
    on conflict do nothing;
";

const USER_VAL_INC: &str = "
    update user_value
        set value = value + 1
    where email = $1;
";

#[get("/inc")]
async fn increment_user_value(
    client: &State<sync::Arc<Client>>,
    user: User,
) -> Result<Value, Error> {
    client.execute(USER_VAL_INC, &[&user.email()]).await?;
    let stmt = client
        .prepare("select value from user_value where email = $1")
        .await?;
    let rows = client.query(&stmt, &[&user.email()]).await?;
    assert_eq!(rows.len(), 1);
    let count = rows[0].get::<_, i32>(0);
    Ok(json!({ "metric": count }))
}

#[get("/user_value", rank = 1)]
async fn get_user_value(user: User, client: &State<sync::Arc<Client>>) -> Value {
    let stmt = client
        .prepare("select value from user_value where email = $1")
        .await
        .unwrap();
    let rows = client.query(&stmt, &[&user.email()]).await.unwrap();
    assert_eq!(rows.len(), 1);
    let value = rows[0].get::<_, i32>(0);
    json!({ "metric": value })
}

#[get("/user_value", rank = 2)]
async fn get_user_value_nouser(_client: &State<sync::Arc<Client>>) -> Value {
    let value: Option<i32> = None;
    json!({ "metric": value })
}

#[get("/user_id", rank = 1)]
async fn get_user_id(user: User) -> Value {
    json!({ "email": user.email().clone() })
}

#[get("/user_id", rank = 2)]
async fn get_user_id_nouser() -> Value {
    let value: Option<String> = None;
    json!({ "email": value })
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
        .map(TryInto::try_into)
        .flatten()
        .collect();

    Ok(Template::render(
        "users",
        json!({"users": users, "user": user}),
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    println!("{}", cli.static_path.display());

    use tokio_postgres::NoTls;
    let (client, conn) = connect("host=localhost user=vhallway password=vhallway", NoTls).await?;
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
                get_user_value,
                get_user_value_nouser,
                get_user_id,
                get_user_id_nouser,
                increment_user_value,
                get_login,
                post_signup,
                get_signup,
                post_login,
                logout,
                delete,
                show_all_users
            ],
        )
        .mount("/", FileServer::from(cli.static_path))
        .manage(client)
        .manage(users)
        .attach(Template::fairing())
        .ignite()
        .await?
        .launch()
        .await?;

    Ok(())
}
