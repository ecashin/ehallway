#[macro_use]
extern crate rocket;

use rocket_auth::User;

#[get("/user")]
fn user_info(user: User) -> String {
    format!("{:?}", user)
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![user_info])
}
