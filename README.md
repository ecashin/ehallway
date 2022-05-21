# eHallway

Teleworkers accidentally meeting each other.

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
** PostgreSQL Database
** Caddy Reverse Proxy
* eHallway Originals
** API Server
** UI

### Configuring Postgres

### Preparing Caddy

## System Startup

Starting at the repository's top level,
the user interface (UI) is built and run
as follows in the example below.

    cd ui && \
    trunk build && \
    sh -xe ../tpt-update.sh

Starting at the repository's top level,
the web server is built and run as shown below.

    cd api && \
    cargo run -- --static-path ../ui/dist

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
