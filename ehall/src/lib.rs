use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct JoinedMeetingsMessage {
    pub meetings: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash, PartialEq, Eq)]
pub struct Meeting {
    pub name: String,
    pub id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct MeetingMessage {
    pub meeting: Meeting,
    pub score: u32,
}

#[derive(Serialize, Deserialize)]
pub struct MeetingsMessage {
    pub meetings: Vec<MeetingMessage>,
}

#[derive(Serialize, Deserialize)]
pub struct NewMeeting<'r> {
    pub name: Cow<'r, str>,
}

#[derive(Serialize, Deserialize)]
pub struct ParticipateMeetingMessage {
    pub participate: bool,
}

#[derive(Deserialize, Serialize)]
pub struct NewTopicMessage {
    pub new_topic: String,
}

#[derive(Deserialize, Serialize)]
pub struct ScoreMessage {
    pub score: u32,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct UserIdMessage {
    pub email: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserTopic {
    pub text: String,
    pub score: u32,
    pub id: u32,
}

#[derive(Deserialize)]
pub struct UserTopicsMessage {
    pub topics: Vec<UserTopic>,
}
