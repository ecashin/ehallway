# eHallway

"Teleworkers accidentally meeting each other"

This implementation is incomplete but contains examples
of working web technologies.

## Technologies

This software has a web server (back end)
and a web user interface (front end),
both implemented in [Rust](https://www.rust-lang.org/).

The front end has a [React](https://reactjs.org/)-like implementation
written in Rust by using [yew v0.19](https://yew.rs/)
and compiled to [Web Assembly (wasm)](https://webassembly.org/)
and bundled with [trunk](https://trunkrs.dev/).

The back end is also written in Rust
using the [Rocket](https://rocket.rs/) framework
with [rocket_auth](https://docs.rs/rocket_auth/latest/rocket_auth/)
and [tokio_postgres](https://docs.rs/tokio-postgres/latest/tokio_postgres/)
working together to handle user-authenticated sessions.

## Concepts

The eHallway isn't finished,
but this section covers the concepts
guiding its direction.

In a physical workplace, accidental hallway meetings
can spark new collaborations and new ways of working.
The conversations aren't always expected or desired.
You might find yourself trying to figure out how to get away
while two colleagues discuss potato farming,
but these unplanned interactions can keep a community fresh.

This proof of concept system attempts to provide
some of the hallway-meeting serendipity
that teleworkers don't often experience.

To that end, each user can create potential topics of conversation.
Each user can create a named meeting.
The meeting can be anything, like, "Monday 9am Discord", e.g.
Each user can indicate the desire to attend a meeting.

When the meeting participants all check in,
cohorts of three participants are randomly chosen.
Each cohort sees nine topics, three from each participant in the cohort.
Each cohort ranks all nine topics.
The system uses the Borda Count method to select the top two scoring topics,
and the cohort members are presented with the top two topics.

## Development Status

Now eHallway is a bare-bones framework.
The parts below are in place.

* User authentication (email and password only) and authenticated sessions
* User-specific topic-list management
* Site-wide meeting management

## System Setup

The system has a few main components.

* Third-Party
    * [PostgreSQL Database](https://www.postgresql.org/)
    * [Caddy Reverse Proxy](https://caddyserver.com/)
* eHallway Originals
    * API Server
    * UI

### Configuring Postgres

The back-end server creates tables on startup
if they do not already exist.
Beforehand, create an `ehallway` user
and set a password
by following the PostgreSQL documentation.

Pass that username and password to the back end
when starting it,
via command-line arguments.

### Preparing Caddy

Caddy is run as root in order to use the regular HTTPS port.
If you use a script as shown in the example below,
you do not have to configure Caddy.

### Preparing API

Using [rustup](https://rustup.rs/) or your favorite method,
ensure that the nightly toolchain is installed,
and configure it to be used for building the back end.
(The front end does not need the nightly toolchain.)

    cd api
    rustup override set nightly

### Preparing UI

Starting at the repository's top level,
the user interface (UI) is built
as follows in the example below.

    cd ui && \
    trunk build && \
    sh -xe ../tpt-update.sh

## System Startup

Create a [TOML](https://github.com/toml-lang/toml) config file
that contains contents you edit to supply your own values.
Example config-file contents are shown below.

    static_path = "/path/to/ehallway/ui/dist"
    postgres_user = "ehallway"
    postgres_password = "mypgpassword"

Starting at the repository's top level,
the web server is built and run as shown below.

    cd api && \
    cargo run -- --config-file myconfig.toml

Starting at the repository's top level,
the reverse proxy server is started as shown below.
Please edit the command,
so that the path to your own caddy is used.

    sudo ~/opt/bin/caddy reverse-proxy --to 127.0.0.1:8000

## System Usage

To access the system, use your web browser
to visit [this link](https://localhost/).

## Contributing

Documentation uses [semantic linefeeds](https://rhodesmill.org/brandon/2012/one-sentence-per-line/).
