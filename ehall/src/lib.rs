use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// A None cohort means try again.
#[derive(Serialize, Deserialize)]
pub struct CohortMessage {
    /// The cohort that includes the user getting the message
    pub cohort: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ElectionResults {
    pub meeting: u32,
    pub topics: Option<Vec<UserTopic>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash, PartialEq, Eq)]
pub struct Meeting {
    pub name: String,
    pub id: u32,
}
#[derive(Serialize, Deserialize)]
pub struct MeetingParticipantsMessage {
    pub n_joined: u32,
    pub n_registered: u32,
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

#[derive(Deserialize, Serialize)]
pub struct NewTopicMessage {
    pub new_topic: String,
}

#[derive(Serialize, Deserialize)]
pub struct ParticipateMeetingMessage {
    pub participate: bool,
}

#[derive(Serialize, Deserialize)]
pub struct RegisteredMeetingsMessage {
    pub meetings: Vec<u32>,
}

#[derive(Deserialize, Serialize)]
pub struct ScoreMessage {
    pub score: u32,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct UserIdMessage {
    pub email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserTopic {
    pub text: String,
    pub score: u32,
    pub id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct UserTopicsMessage {
    pub topics: Vec<UserTopic>,
}

pub fn argsort<T>(a: &[T]) -> Vec<usize>
where
    T: PartialOrd,
{
    let mut indexed: Vec<_> = a.iter().enumerate().collect();
    indexed.sort_by(|(_i1, v1), (_i2, v2)| v1.partial_cmp(v2).unwrap());
    indexed.into_iter().map(|(i, _v)| i).collect()
}
