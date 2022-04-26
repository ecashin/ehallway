#[macro_use]
extern crate rocket;

use anyhow::Result;
use rocket::State;
use rocket_auth::{User, Users};
use tokio_postgres::NoTls;

#[get("/user")]
fn user_info(user: User) -> String {
    "user info".to_owned()
}

#[get("/users")]
fn see_users(users: &State<Users>) -> String {
    "see users done".to_owned()
}

#[rocket::main]
async fn main() -> Result<()> {
    println!("here");
    let (db_client, conn) = tokio_postgres::connect("postgres://vhallway:vhallway@127.0.0.1/", NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("connection error: {}", e);
        }
    });
    println!("there");
    let rows = db_client
        .query("SELECT $1::TEXT", &[&"hello world"])
        .await?;
    let value: &str = rows[0].get(0);
    assert_eq!(value, "hello world");
    println!("everywhere");

    let users: Users = db_client.into();
    users.create_table().await?;

    rocket::build().mount("/", routes![see_users, user_info]).manage(db_client).manage(users).launch().await.expect("launching");
    Ok(())
}
