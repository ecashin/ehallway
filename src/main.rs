use anyhow::Result;
use oxhttp::model::{Response, Status};
use oxhttp::Server;
use std::time::Duration;

fn main() -> Result<()> {
    // Builds a new server that returns a 404 everywhere except for "/"
    // where it returns the body 'home'
    let mut server = Server::new(|request| {
        if request.url().path() == "/" {
            Response::builder(Status::OK).with_body("home")
        } else {
            Response::builder(Status::NOT_FOUND).build()
        }
    });
    server.set_global_timeout(Duration::from_secs(10));
    server.listen(("localhost", 8080)).unwrap();
    Ok(())
}
