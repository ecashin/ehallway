use clap::Parser;
use rocket::fs::FileServer;
use rocket::{form::*, get, post, response::Redirect, routes, State};
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

const USER_VAL_CREATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS user_value (
    email VARCHAR (254) UNIQUE NOT NULL primary key,
    value integer DEFAULT 0
);
";

const USER_VAL_INC: &str = "
update user_value
  set value = value + 1
where email = $1;
";

use rocket::serde::json::Value;

#[get("/inc", format = "json")]
async fn increment_user_value(
    client: &State<sync::Arc<Client>>,
    user: User,
) -> Result<Value, Error> {
    client.execute(USER_VAL_CREATE_TABLE, &[]).await?;
    client.query(USER_VAL_INC, &[&user.email()]).await?;
    Ok(json!({ "metric": 3.3 }))
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
    let app = rocket::build()
        .mount(
            "/",
            routes![
                index,
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
