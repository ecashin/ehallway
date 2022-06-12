use std::{borrow::Cow, boxed, collections::HashSet};

use anyhow::{anyhow, Error, Result};
use gloo_console::console_dbg;
use gloo_net::http;
use gloo_timers::callback::Interval;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use ehall::{
    ElectionResults, MeetingParticipantsMessage, MeetingsMessage, NewMeeting, NewTopicMessage,
    ParticipateMeetingMessage, RegisteredMeetingsMessage, ScoreMessage, UserIdMessage, UserTopic,
    UserTopicsMessage,
};
use svg::add_icon;

mod ranking;
mod svg;

struct Meeting {
    id: u32,
    name: String,
    score: u32,
}

enum Msg {
    AddMeeting,
    AddTopic,
    AddedMeeting,
    AddedTopic,
    AttendingMeeting(boxed::Box<u32>),
    AttendMeeting(u32),
    CheckElection,
    DeleteMeeting(u32),
    DeleteUserTopic(u32),
    DidFinishVoting,
    DidStoreMeetingScore,
    DidStoreMeetingTopicScore(boxed::Box<u32>),
    DidStoreUserTopicScore,
    CommitVote,
    FetchNMeetingParticipants(u32),
    FetchMeetingTopics(u32),
    FetchUserTopics,
    LeaveMeeting,
    LogError(Error),
    MeetingRegisteredChanged,
    MeetingToggleRegistered(u32),
    Noop,
    SetElectionResults(ElectionResults),
    SetNRegisteredNJoined((u32, u32)),
    SetRegisteredMeetings(Vec<u32>),
    SetMeetings(Vec<Meeting>),
    SetMeetingTopics(Vec<UserTopic>),
    SetTab(Tab),
    SetUserId(String),
    SetUserTopics(Vec<UserTopic>),      // set in Model
    StoreMeetingScore((u32, u32)),      // (id, score) - store to database
    StoreMeetingTopicScore((u32, u32)), // (id, score)
    StoreUserTopicScore((u32, u32)),    // (id, score)
    UpdateNewMeetingText(String),
    UpdateNewTopicText(String),
}

enum UserIdState {
    New,
    Fetching,
    Fetched(String),
}

impl UserIdState {
    fn is_new(&self) -> bool {
        matches!(self, UserIdState::New)
    }
}

#[derive(Clone, PartialEq)]
enum Tab {
    MeetingManagement,
    MeetingPrep,
    TopicManagment,
}

struct Model {
    attending_meeting: Option<u32>, // the meeting the user is currently attending
    election_results: Option<(u32, Vec<UserTopic>)>,
    n_attending_meeting_registered: Option<u32>,
    n_attending_meeting_joined: Option<u32>,
    registered_meetings: HashSet<u32>,
    meeting_topics: Option<Vec<UserTopic>>,
    meetings: Vec<Meeting>,
    new_meeting_text: String,
    new_topic_text: String,
    user_id: UserIdState,
    user_topics: Vec<UserTopic>,
    active_tab: Tab,
    vote_poll: Option<Interval>,
}

async fn fetch_user_id() -> Option<String> {
    let resp = http::Request::get("/user_id")
        .send()
        .await
        .unwrap()
        .json()
        .await;
    match resp {
        Ok(resp) => {
            let msg: UserIdMessage = resp;
            Some(msg.email)
        }
        Err(_e) => None,
    }
}

fn error_from_response(resp: http::Response) -> Error {
    let status = resp.status();
    assert_ne!(status, 200);
    anyhow!("response status {status}: {}", resp.status_text())
}

async fn fetch_meetings() -> Result<Vec<Meeting>> {
    let resp: std::result::Result<MeetingsMessage, gloo_net::Error> =
        http::Request::get("/meetings").send().await?.json().await;
    match resp {
        Ok(msg) => {
            let mut mtgs: Vec<_> = msg
                .meetings
                .iter()
                .map(|mm| Meeting {
                    id: mm.meeting.id,
                    name: mm.meeting.name.clone(),
                    score: mm.score,
                })
                .collect();
            mtgs.sort_by(|Meeting { score: a, .. }, Meeting { score: b, .. }| {
                a.partial_cmp(b).unwrap()
            });
            let mut canonically_scored_meetings: Vec<_> = vec![];
            for (canonical_score, Meeting { id, name, score }) in mtgs.iter().enumerate() {
                let cscore = canonical_score as u32;
                if *score != cscore {
                    store_meeting_score(boxed::Box::new(*id), boxed::Box::new(cscore))
                        .await
                        .unwrap();
                }
                canonically_scored_meetings.push(Meeting {
                    id: *id,
                    name: name.clone(),
                    score: cscore,
                });
            }
            Ok(canonically_scored_meetings)
        }
        Err(e) => Err(e.into()),
    }
}

async fn fetch_n_meeting_participants(
    meeting_id: boxed::Box<u32>,
) -> Result<MeetingParticipantsMessage> {
    let url = format!("/meeting/{meeting_id}/participant_counts");
    let resp: std::result::Result<MeetingParticipantsMessage, gloo_net::Error> =
        http::Request::get(&url).send().await?.json().await;
    match resp {
        Ok(msg) => Ok(msg),
        Err(e) => Err(e.into()),
    }
}

async fn fetch_registered_meetings() -> Result<Vec<u32>> {
    let resp: std::result::Result<RegisteredMeetingsMessage, gloo_net::Error> =
        http::Request::get("/registered_meetings")
            .send()
            .await?
            .json()
            .await;
    match resp {
        Ok(msg) => Ok(msg.meetings),
        Err(e) => Err(e.into()),
    }
}

async fn fetch_meeting_topics(meeting_id: boxed::Box<u32>) -> Result<Vec<UserTopic>> {
    let url = format!("/meeting/{meeting_id}/topics");
    let resp: std::result::Result<UserTopicsMessage, gloo_net::Error> =
        http::Request::get(&url).send().await?.json().await;
    match resp {
        Ok(msg) => {
            let mut topics = msg.topics;
            topics.sort_by(|a, b| {
                let UserTopic { score: a_score, .. } = a;
                let UserTopic { score: b_score, .. } = b;
                a_score.partial_cmp(b_score).unwrap()
            });
            Ok(topics
                .into_iter()
                .enumerate()
                .map(|(score, UserTopic { text, id, .. })| UserTopic {
                    id,
                    text,
                    score: score as u32,
                })
                .collect())
        }
        Err(e) => Err(e.into()),
    }
}

async fn fetch_user_topics() -> Result<Vec<UserTopic>> {
    let resp: std::result::Result<UserTopicsMessage, gloo_net::Error> =
        http::Request::get("/user_topics")
            .send()
            .await?
            .json()
            .await;
    match resp {
        Ok(msg) => {
            let mut topics = msg.topics;
            topics.sort_by(|a, b| {
                let UserTopic { score: a_score, .. } = a;
                let UserTopic { score: b_score, .. } = b;
                a_score.partial_cmp(b_score).unwrap()
            });
            let orig_scores: Vec<_> = topics.iter().map(|t| t.score).collect();
            let topics: Vec<_> = topics
                .into_iter()
                .enumerate()
                .map(|(score, UserTopic { text, id, .. })| UserTopic {
                    id,
                    text,
                    score: score as u32,
                })
                .collect();
            let canonical_scores: Vec<_> = topics.iter().map(|t| t.score).collect();
            if orig_scores != canonical_scores {
                for t in topics.iter() {
                    store_user_topic_score(boxed::Box::new(t.id), boxed::Box::new(t.score))
                        .await
                        .unwrap();
                }
            }
            Ok(topics)
        }
        Err(e) => Err(e.into()),
    }
}

async fn commit_vote(meeting_id: boxed::Box<u32>) -> Result<()> {
    let url = format!("/meeting/{}/vote", meeting_id);
    gloo_net::http::Request::put(&url).send().await?;
    Ok(())
}

async fn delete_meeting(id: boxed::Box<u32>) -> Result<()> {
    let url = format!("/meetings/{}", id);
    gloo_net::http::Request::delete(&url).send().await?;
    Ok(())
}

async fn delete_user_topic(id: boxed::Box<u32>) -> Result<()> {
    let url = format!("/topics/{}", id);
    gloo_net::http::Request::delete(&url).send().await?;
    Ok(())
}

async fn fetch_election_status(meeting_id: boxed::Box<u32>) -> Result<ElectionResults> {
    let url = format!("/meeting/{}/election_results", meeting_id);
    let resp: std::result::Result<ElectionResults, gloo_net::Error> =
        http::Request::get(&url).send().await?.json().await;
    match resp {
        Err(e) => Err(e.into()),
        Ok(msg) => Ok(msg),
    }
}

async fn store_meeting_score(meeting_id: boxed::Box<u32>, score: boxed::Box<u32>) -> Result<()> {
    let url = format!("/meeting/{}/score", meeting_id);
    gloo_net::http::Request::put(&url)
        .json(&ScoreMessage { score: *score })?
        .send()
        .await?;
    Ok(())
}

async fn store_meeting_topic_score(
    meeting_id: boxed::Box<u32>,
    topic_id: boxed::Box<u32>,
    score: boxed::Box<u32>,
) -> Result<()> {
    let url = format!("/meeting/{}/topic/{}/score", meeting_id, topic_id);
    gloo_net::http::Request::put(&url)
        .json(&ScoreMessage { score: *score })?
        .send()
        .await?;
    Ok(())
}

async fn store_user_topic_score(topic_id: boxed::Box<u32>, score: boxed::Box<u32>) -> Result<()> {
    let url = format!("/topic/{}/score", topic_id);
    gloo_net::http::Request::put(&url)
        .json(&ScoreMessage { score: *score })?
        .send()
        .await?;
    Ok(())
}

async fn attend_meeting(meeting_id: boxed::Box<u32>) -> Result<http::Response> {
    let url = format!("/meeting/{}/attendees", *meeting_id);
    Ok(gloo_net::http::Request::post(&url).send().await?)
}

async fn add_new_meeting(name: String) -> Result<http::Response> {
    let new_meeting = NewMeeting {
        name: Cow::from(name),
    };
    Ok(gloo_net::http::Request::post("/meetings")
        .json(&new_meeting)?
        .send()
        .await?)
}

async fn add_new_topic(topic_text: String) -> Result<http::Response> {
    let topic = NewTopicMessage {
        new_topic: topic_text,
    };
    Ok(gloo_net::http::Request::post("/topics")
        .json(&topic)?
        .send()
        .await?)
}

async fn register_for_meeting(id: boxed::Box<u32>, participate: bool) -> Result<http::Response> {
    let id = *id;
    let url = format!("/meeting/{id}/participants");
    Ok(gloo_net::http::Request::post(&url)
        .json(&ParticipateMeetingMessage { participate })?
        .send()
        .await?)
}

impl Model {
    fn fetch_user(&mut self, tag: &str, ctx: &Context<Self>) {
        self.user_id = UserIdState::Fetching;
        console_dbg!(format!("fetch_user in {}", tag));
        ctx.link().send_future(async {
            if let Some(uid) = fetch_user_id().await {
                Msg::SetUserId(uid)
            } else {
                Msg::Noop
            }
        });
        ctx.link().send_future(async {
            if let Ok(topics) = fetch_user_topics().await {
                Msg::SetUserTopics(topics)
            } else {
                Msg::Noop
            }
        });
        ctx.link().send_future(async {
            if let Ok(meetings) = fetch_registered_meetings().await {
                Msg::SetRegisteredMeetings(meetings)
            } else {
                Msg::Noop
            }
        });
    }

    fn meeting_attendance_html(&self, ctx: &Context<Self>) -> Html {
        if let Some(meeting_id) = self.attending_meeting {
            let meeting_name = &self
                .meetings
                .iter()
                .find_map(|m| if m.id == meeting_id { Some(m) } else { None })
                .unwrap()
                .name;
            let join_info_html = if let Some(n_registered) = self.n_attending_meeting_registered {
                let n_joined = self.n_attending_meeting_joined.unwrap();
                html! {
                    <div class="container">
                        <div class="row">
                            <div class="col">
                                <h3>{format!("{n_joined} of {n_registered} registered participants have joined")}</h3>
                            </div>
                            <div class="col">
                                <button
                                    type="button"
                                    onclick={ctx.link().callback(move |_| Msg::FetchNMeetingParticipants(meeting_id))}
                                    class="btn btn-secondary"
                                >{"refresh"}</button>
                            </div>
                        </div>
                        <div class="row">
                            <div class="col">
                                <button
                                    type="button"
                                    class="btn btn-success"
                                    onclick={ctx.link().callback(move |_| Msg::CommitVote)}
                                >{"DONE RANKING!"}</button>
                            </div>
                        </div>
                    </div>
                }
            } else {
                html! {}
            };
            let meeting_topics_html = if let Some(topics) = &self.meeting_topics {
                html! {
                    <ranking::Ranking
                        ids={topics.iter().map(|t| t.id).collect::<Vec<u32>>()}
                        labels={topics.iter().map(|t| t.text.clone()).collect::<Vec<String>>()}
                        scores={topics.iter().map(|t| t.score).collect::<Vec<u32>>()}
                        store_score={ctx.link().callback(Msg::StoreMeetingTopicScore)}
                    />
                }
            } else {
                html! {}
            };
            html! {
                <div class="container">
                    <div class="row">
                        <h2>{ format!("Attending meeting: {}", meeting_name) }</h2>
                        {join_info_html}
                        <button
                            onclick={ctx.link().callback(move |_| Msg::LeaveMeeting)}
                            type={"button"}
                            class={"btn btn-secondary"}
                        >{"leave"}</button>
                    </div>
                    <div class="row">
                        { meeting_topics_html }
                    </div>
                </div>
            }
        } else {
            html! {}
        }
    }
    fn meeting_management_html(&self, ctx: &Context<Self>) -> Html {
        let onkeypress = ctx
            .link()
            .batch_callback(move |e: KeyboardEvent| (e.key() == "Enter").then(|| Msg::AddMeeting));

        let new_meeting = if let UserIdState::Fetched(_uid) = &self.user_id {
            html! {
                <div>
                    <label>{"Add new meeting"}</label>
                    <input
                        id="new-meeting"
                        type="text"
                        value={self.new_meeting_text.clone()}
                        { onkeypress }
                        oninput={ctx.link().callback(|e: InputEvent| {
                                let input = e.target_unchecked_into::<HtmlInputElement>();
                                Msg::UpdateNewMeetingText(input.value())
                        })}
                    />
                    <button
                        onclick={ctx.link().callback(|_| Msg::AddMeeting)}
                        type={"button"}
                        class={"btn"}
                    >{ add_icon() }</button>
                </div>
            }
        } else {
            html! {}
        };
        let mut meetings: Vec<_> = self
            .meetings
            .iter()
            .map(|Meeting { id, name, score }| (*id, name.clone(), *score))
            .collect();
        meetings.sort_by(|(_a_id, _a_name, a_score), (_b_id, _b_name, b_score)| {
            b_score.partial_cmp(a_score).unwrap()
        });
        let meetings_html = {
            let ids = meetings.iter().map(|m| m.0).collect::<Vec<u32>>();
            html! {
                <ranking::Ranking
                    ids={ids.clone()}
                    labels={meetings.iter().map(|m| m.1.clone()).collect::<Vec<String>>()}
                    scores={meetings.iter().map(|m| m.2).collect::<Vec<u32>>()}
                    store_score={ctx.link().callback(Msg::StoreMeetingScore)}
                    delete={Some(ctx.link().callback(Msg::DeleteMeeting))}
                    is_registered={Some(ids.iter().map(|id| self.registered_meetings.get(id).is_some()).collect::<Vec<bool>>())}
                    attend_meeting={Some(ctx.link().callback(Msg::AttendMeeting))}
                    register_toggle={Some(ctx.link().callback(Msg::MeetingToggleRegistered))}
                />
            }
        };
        html! {
            <div>
                {new_meeting}
                <div class="container">
                    {meetings_html}
                </div>
            </div>
        }
    }

    fn tabs_html(&self, ctx: &Context<Self>) -> Html {
        let item_class = |tag| {
            if self.active_tab == tag {
                "nav-item"
            } else {
                "nav-item active"
            }
        };
        // aria-current value
        let ac = |tag| {
            if self.active_tab == tag {
                "page"
            } else {
                "false"
            }
        };
        // https://getbootstrap.com/docs/5.0/components/navs-tabs/
        html! {
            <ul class="nav nav-tabs">
                <li class={ item_class(Tab::TopicManagment) } aria-current={ac(Tab::TopicManagment)}>
                    <a class="nav-link" href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::TopicManagment))}>{ "Topics" }</a>
                </li>
                <li class={ item_class(Tab::MeetingManagement) } aria-current={ac(Tab::MeetingManagement)}>
                    <a class="nav-link" href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::MeetingManagement))}>{ "Meetings" }</a>
                </li>
                <li class={ item_class(Tab::MeetingPrep) } aria-current={ac(Tab::MeetingPrep)}>
                    <a class="nav-link" href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::MeetingPrep))}>{ "Meet" }</a>
                </li>
            </ul>
        }
    }
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let mut model = Self {
            attending_meeting: None,
            election_results: None,
            registered_meetings: HashSet::new(),
            meeting_topics: None,
            meetings: vec![],
            n_attending_meeting_joined: None,
            n_attending_meeting_registered: None,
            new_meeting_text: "".to_owned(),
            new_topic_text: "".to_owned(),
            user_id: UserIdState::New,
            user_topics: vec![],
            active_tab: Tab::TopicManagment,
            vote_poll: None,
        };
        model.fetch_user("create", ctx);
        model
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        if self.user_id.is_new() {
            self.fetch_user("update", ctx);
        }
        match msg {
            Msg::AddedMeeting => {
                self.new_meeting_text = "".to_owned();
                ctx.link().send_future(async {
                    match fetch_meetings().await {
                        Ok(meetings) => Msg::SetMeetings(meetings),
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::AddedTopic => {
                self.new_topic_text = "".to_owned();
                ctx.link().send_message(Msg::FetchUserTopics);
                true
            }
            Msg::AddMeeting => {
                let meeting_name = self.new_meeting_text.clone();
                ctx.link().send_future(async {
                    match add_new_meeting(meeting_name).await {
                        Ok(resp) => {
                            if resp.status() == 200 {
                                Msg::AddedMeeting
                            } else {
                                Msg::LogError(error_from_response(resp))
                            }
                        }
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::AddTopic => {
                let topic_text = self.new_topic_text.clone();
                ctx.link().send_future(async {
                    match add_new_topic(topic_text).await {
                        Ok(resp) => {
                            if resp.status() == 200 {
                                Msg::AddedTopic
                            } else {
                                Msg::LogError(error_from_response(resp))
                            }
                        }
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::AttendingMeeting(id) => {
                self.attending_meeting = Some(*id);
                ctx.link().send_message(Msg::SetTab(Tab::MeetingPrep));
                true
            }
            Msg::AttendMeeting(id) => {
                let id = boxed::Box::new(id);
                ctx.link().send_future(async {
                    match attend_meeting(id.clone()).await {
                        Ok(_) => Msg::AttendingMeeting(id),
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::CheckElection => {
                if self.attending_meeting.is_none() {
                    false
                } else {
                    let meeting_id = boxed::Box::new(self.attending_meeting.unwrap());
                    ctx.link().send_future(async {
                        let m_id = *meeting_id;
                        match fetch_election_status(meeting_id).await {
                            Ok(msg) => {
                                if msg.meeting == m_id && msg.topics.is_some() {
                                    Msg::SetElectionResults(msg)
                                } else {
                                    Msg::Noop
                                }
                            }
                            Err(e) => Msg::LogError(e),
                        }
                    });
                    true
                }
            }
            Msg::CommitVote => {
                if let Some(meeting_id) = self.attending_meeting {
                    let meeting_id = boxed::Box::new(meeting_id);
                    ctx.link().send_future(async {
                        match commit_vote(meeting_id).await {
                            Ok(()) => Msg::DidFinishVoting,
                            Err(e) => Msg::LogError(e),
                        }
                    });
                    true
                } else {
                    false
                }
            }
            Msg::DeleteMeeting(id) => {
                let id = boxed::Box::new(id);
                ctx.link().send_future(async {
                    match delete_meeting(id).await {
                        Ok(_) => Msg::AddedMeeting,
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::DeleteUserTopic(id) => {
                let id = boxed::Box::new(id);
                ctx.link().send_future(async {
                    match delete_user_topic(id).await {
                        Ok(_) => Msg::AddedTopic,
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::DidFinishVoting => {
                let handle = {
                    let link = ctx.link().clone();
                    Interval::new(1000, move || link.send_message(Msg::CheckElection))
                };
                self.vote_poll = Some(handle);
                true
            }
            Msg::DidStoreMeetingScore => {
                ctx.link().send_future(async {
                    match fetch_meetings().await {
                        Ok(meetings) => Msg::SetMeetings(meetings),
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::DidStoreMeetingTopicScore(meeting_id) => {
                ctx.link()
                    .send_message(Msg::FetchMeetingTopics(*meeting_id));
                false
            }
            Msg::DidStoreUserTopicScore => {
                ctx.link().send_message(Msg::FetchUserTopics);
                false
            }
            Msg::FetchNMeetingParticipants(meeting_id) => {
                let id = boxed::Box::new(meeting_id);
                ctx.link().send_future(async {
                    match fetch_n_meeting_participants(id).await {
                        Ok(MeetingParticipantsMessage {
                            n_joined,
                            n_registered,
                        }) => Msg::SetNRegisteredNJoined((n_registered, n_joined)),
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::FetchMeetingTopics(meeting_id) => {
                let id = boxed::Box::new(meeting_id);
                ctx.link().send_future(async {
                    match fetch_meeting_topics(id).await {
                        Ok(topics) => Msg::SetMeetingTopics(topics),
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::FetchUserTopics => {
                ctx.link().send_future(async {
                    match fetch_user_topics().await {
                        Ok(topics) => Msg::SetUserTopics(topics),
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::LeaveMeeting => {
                self.attending_meeting = None;
                self.election_results = None;
                self.vote_poll = None;
                self.active_tab = Tab::MeetingManagement;
                true
            }
            Msg::LogError(e) => {
                console_dbg!(format!("{e}"));
                true
            }
            Msg::MeetingRegisteredChanged => {
                // could refresh participation info here, but worth it?
                true
            }
            Msg::MeetingToggleRegistered(id) => {
                let boxed_id = boxed::Box::<u32>::new(id);
                if self.registered_meetings.contains(&id) {
                    self.registered_meetings.remove(&id);
                    ctx.link().send_future(async {
                        register_for_meeting(boxed_id, false).await.unwrap();
                        Msg::MeetingRegisteredChanged
                    });
                } else {
                    self.registered_meetings.insert(id);
                    ctx.link().send_future(async {
                        register_for_meeting(boxed_id, true).await.unwrap();
                        Msg::MeetingRegisteredChanged
                    });
                }
                true
            }
            Msg::Noop => true,
            Msg::SetElectionResults(results) => {
                if let Some(meeting) = self.attending_meeting {
                    if results.meeting == meeting {
                        self.vote_poll = None;
                        self.election_results = Some((meeting, results.topics.unwrap()));
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Msg::SetMeetingTopics(topics) => {
                self.meeting_topics = Some(topics);
                true
            }
            Msg::SetRegisteredMeetings(meetings) => {
                self.registered_meetings = meetings.into_iter().collect();
                true
            }
            Msg::SetMeetings(meetings) => {
                self.meetings = meetings;
                true
            }
            Msg::SetNRegisteredNJoined((n_registered, n_joined)) => {
                self.n_attending_meeting_registered = Some(n_registered);
                self.n_attending_meeting_joined = Some(n_joined);
                true
            }
            Msg::SetTab(tab) => {
                let prev_tab = self.active_tab.clone();
                self.active_tab = tab.clone();
                if let Some(meeting_id) = self.attending_meeting {
                    if tab == Tab::MeetingPrep && tab != prev_tab {
                        ctx.link()
                            .send_message(Msg::FetchNMeetingParticipants(meeting_id));
                        ctx.link().send_message(Msg::FetchMeetingTopics(meeting_id));
                    }
                }
                true
            }
            Msg::SetUserId(email) => {
                console_dbg!(format!("got email: {}", &email));
                self.user_id = UserIdState::Fetched(email);
                ctx.link().send_future(async {
                    match fetch_meetings().await {
                        Ok(meetings) => Msg::SetMeetings(meetings),
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::SetUserTopics(topics) => {
                self.user_topics = topics;
                true
            }
            Msg::StoreMeetingScore((meeting_id, score)) => {
                let score = boxed::Box::new(score);
                let meeting_id = boxed::Box::new(meeting_id);
                ctx.link().send_future(async {
                    match store_meeting_score(meeting_id, score).await {
                        Ok(_) => Msg::DidStoreMeetingScore,
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::StoreMeetingTopicScore((id, score)) => {
                if self.meeting_topics.is_some() {
                    let score = boxed::Box::new(score);
                    let topic_id = boxed::Box::new(id);
                    let meeting_id = boxed::Box::new(self.attending_meeting.unwrap());
                    ctx.link().send_future(async {
                        match store_meeting_topic_score(meeting_id.clone(), topic_id, score).await {
                            Ok(_) => Msg::DidStoreMeetingTopicScore(meeting_id),
                            Err(e) => Msg::LogError(e),
                        }
                    });
                }
                true
            }
            Msg::StoreUserTopicScore((id, score)) => {
                let score = boxed::Box::new(score);
                let id = boxed::Box::new(id);
                ctx.link().send_future(async {
                    match store_user_topic_score(id, score).await {
                        Ok(_) => Msg::DidStoreUserTopicScore,
                        Err(e) => Msg::LogError(e),
                    }
                });
                true
            }
            Msg::UpdateNewMeetingText(text) => {
                self.new_meeting_text = text;
                true
            }
            Msg::UpdateNewTopicText(text) => {
                self.new_topic_text = text;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let onkeypress = ctx
            .link()
            .batch_callback(move |e: KeyboardEvent| (e.key() == "Enter").then(|| Msg::AddTopic));
        let new_topic = if let UserIdState::Fetched(_uid) = &self.user_id {
            html! {
                <div>
                    <input
                        id="new-topic" type="text" value={self.new_topic_text.clone()}
                        { onkeypress }
                        oninput={ctx.link().callback(|e: InputEvent| {
                                let input = e.target_unchecked_into::<HtmlInputElement>();
                                Msg::UpdateNewTopicText(input.value())
                        })}
                    />
                    <button
                        type={"button"} class={"btn"}
                        onclick={ctx.link().callback(|_| Msg::AddTopic)}>{ add_icon() }</button>
                </div>
            }
        } else {
            html! {}
        };
        let topics_html = html! {
            <ranking::Ranking
                ids={self.user_topics.iter().map(|t| t.id).collect::<Vec<u32>>()}
                labels={self.user_topics.iter().map(|t| t.text.clone()).collect::<Vec<String>>()}
                scores={self.user_topics.iter().map(|t| t.score).collect::<Vec<u32>>()}
                store_score={ctx.link().callback(Msg::StoreUserTopicScore)}
                delete={Some(ctx.link().callback(Msg::DeleteUserTopic))}
            />
        };
        let main_panel = html! {
            <div>
                { self.tabs_html(ctx) }
                {
                    match self.active_tab {
                        Tab::TopicManagment => {
                            html! {
                                <div>
                                    { new_topic }
                                    <div class="container">{ topics_html }</div>
                                </div>
                            }
                        }
                        Tab::MeetingManagement => {
                            self.meeting_management_html(ctx)
                        }
                        Tab::MeetingPrep => html!{
                            self.meeting_attendance_html(ctx)
                        }
                    }
                }
            </div>
        };
        if matches!(self.user_id, UserIdState::Fetched(_)) {
            html! { main_panel }
        } else {
            html! {}
        }
    }
}

fn main() {
    let app_div = gloo_utils::document()
        .get_element_by_id("vhallway")
        .unwrap();
    yew::start_app_in_element::<Model>(app_div);
}
