use std::{borrow::Cow, boxed, collections::HashSet};

use anyhow::{anyhow, Error, Result};
use gloo_console::console_dbg;
use gloo_net::http;
use gloo_timers::callback::Interval;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use ehall::{
    ElectionResults, Meeting, MeetingsMessage, NewMeeting, NewTopicMessage,
    ParticipateMeetingMessage, RegisteredMeetingsMessage, ScoreMessage, UserIdMessage, UserTopic,
    UserTopicsMessage,
};
use svg::add_icon;

mod ranking;
mod svg;

const CHECK_ELECTION_MS: u32 = 1_000;

enum Msg {
    AddMeeting,
    AddTopic,
    AddedMeeting,
    AddedTopic,
    AttendingMeeting(boxed::Box<u32>),
    AttendMeeting(u32),
    CheckElection,
    CheckMeetings,
    DeleteMeeting(u32),
    DeleteUserTopic(u32),
    DidFinishVoting,
    DidStoreMeetingScore,
    DidStoreMeetingTopicScore(boxed::Box<u32>),
    DidStoreUserTopicScore,
    CommitVote,
    FetchMeetingTopics(u32),
    FetchUserTopics,
    LeaveMeeting,
    LeftMeeting(boxed::Box<u32>),
    LogError(Error),
    MeetingRegisteredChanged,
    MeetingToggleRegistered(u32),
    Noop,
    SetElectionResults(ElectionResults),
    SetRegisteredMeetings(Vec<u32>),
    SetMeetings(Vec<ScoredMeeting>),
    SetMeetingTopics(Vec<UserTopic>),
    SetTab(Tab),
    SetUserId(String),
    SetUserTopics(Vec<UserTopic>), // set in Model
    StartMeeting,
    StoreMeetingScore((u32, u32)), // (id, score) - store to database
    StoreMeetingTopicScore((u32, u32)), // (id, score)
    StoreUserTopicScore((u32, u32)), // (id, score)
    UpdateNewMeetingText(String),
    UpdateNewTopicText(String),
}

#[derive(Clone)]
struct ScoredMeeting {
    meeting: Meeting,
    score: u32,
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

impl Tab {
    fn needs_meeting_poll(&self) -> bool {
        match self {
            Tab::MeetingManagement => true,
            Tab::MeetingPrep => true,
            Tab::TopicManagment => false,
        }
    }
}

struct Model {
    attending_meeting: Option<u32>, // the meeting the user is currently attending
    election_results: Option<ElectionResults>,
    registered_meetings: HashSet<u32>,
    meeting_topics: Option<Vec<UserTopic>>,
    meetings: Vec<ScoredMeeting>,
    new_meeting_text: String,
    new_topic_text: String,
    user_id: UserIdState,
    user_topics: Vec<UserTopic>,
    active_tab: Tab,
    meeting_poll: Option<Interval>,
    vote_poll: Option<Interval>,
}

// These are populated by the back-end in template rendering.
const LOGIN_JS_OBJECT: &str = "elc_global";
const LOGIN_JS_ATTRIBUTE: &str = "user_email";

fn no_user() -> bool {
    let elc_global = gloo_utils::window().get(LOGIN_JS_OBJECT);
    if let Some(info) = elc_global {
        !info.has_own_property(&wasm_bindgen::JsValue::from(LOGIN_JS_ATTRIBUTE))
    } else {
        true
    }
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

async fn fetch_meetings() -> Result<Vec<ScoredMeeting>> {
    let resp: std::result::Result<MeetingsMessage, gloo_net::Error> =
        http::Request::get("/meetings").send().await?.json().await;
    match resp {
        Ok(msg) => {
            let mut mtgs: Vec<_> = msg
                .meetings
                .into_iter()
                .map(|mm| ScoredMeeting {
                    meeting: mm.meeting,
                    score: mm.score,
                })
                .collect();
            mtgs.sort_by(
                |ScoredMeeting { score: a, .. }, ScoredMeeting { score: b, .. }| {
                    a.partial_cmp(b).unwrap()
                },
            );
            let mut canonically_scored_meetings: Vec<_> = vec![];
            for (canonical_score, ScoredMeeting { meeting, score }) in mtgs.into_iter().enumerate()
            {
                let cscore = canonical_score as u32;
                if score != cscore {
                    store_meeting_score(boxed::Box::new(meeting.id), boxed::Box::new(cscore))
                        .await
                        .unwrap();
                }
                canonically_scored_meetings.push(ScoredMeeting {
                    meeting,
                    score: cscore,
                });
            }
            Ok(canonically_scored_meetings)
        }
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

async fn start_meeting(meeting_id: boxed::Box<u32>) -> Result<()> {
    let url = format!("/meeting/{}/start", meeting_id);
    gloo_net::http::Request::put(&url).send().await?;
    Ok(())
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

async fn leave_meeting(meeting_id: boxed::Box<u32>) -> Result<http::Response> {
    let url = format!("/meeting/{}/attendees", *meeting_id);
    Ok(gloo_net::http::Request::delete(&url).send().await?)
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
    fn meeting_people(&self) -> Option<(usize, usize)> {
        if let Some(attending_meeting) = self.attending_meeting {
            self.meetings
                .iter()
                .filter(|sm| sm.meeting.id == attending_meeting)
                .map(|sm| {
                    (
                        sm.meeting.n_registered as usize,
                        sm.meeting.n_joined as usize,
                    )
                })
                .next()
        } else {
            None
        }
    }

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

    fn meeting_election_results_html(&self, _ctx: &Context<Self>) -> Html {
        let ElectionResults {
            meeting_name,
            meeting_url,
            status,
            topics,
            users,
            ..
        } = self.election_results.as_ref().unwrap();
        let topics_html: Vec<_> = if topics.is_none() {
            vec![]
        } else {
            topics
                .as_ref()
                .unwrap()
                .iter()
                .map(|t| {
                    html! {
                        <div class="row">
                            {t.text.clone()}
                        </div>
                    }
                })
                .collect()
        };
        let users_html: Vec<_> = if let Some(users) = users {
            users
                .iter()
                .map(|u| {
                    html! {
                        <div class="row">
                            {u.clone()}
                        </div>
                    }
                })
                .collect()
        } else {
            vec![]
        };
        html! {
            <>
                <h2>{ meeting_name }</h2>
                <p>{ status }</p>
                <a href={meeting_url.clone()}>{meeting_url}</a>
                <h3>{"Your Group"}</h3>
                <div class="container">
                    {users_html}
                </div>
                <h3>{"Your Topics"}</h3>
                <div class="container">
                    {topics_html}
                </div>
            </>
        }
    }

    fn meeting_attendance_html(&self, ctx: &Context<Self>) -> Html {
        if let Some(meeting_id) = self.attending_meeting {
            let meeting_name = &self
                .meetings
                .iter()
                .find_map(|m| {
                    if m.meeting.id == meeting_id {
                        Some(m)
                    } else {
                        None
                    }
                })
                .unwrap()
                .meeting
                .name;
            let join_info_html = if let Some((n_registered, n_joined)) = self.meeting_people() {
                html! {
                    <div class="container">
                        <div class="row">
                            <div class="col">
                                <h3>{format!("{n_joined} of {n_registered} registered participants have joined")}</h3>
                            </div>
                        </div>
                        <div class="row">
                            <div class="col">
                                <button
                                    type="button"
                                    class="btn btn-success"
                                    onclick={ctx.link().callback(move |_| Msg::StartMeeting)}
                                >{"Start Meeting Now"}</button>
                            </div>
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
            let status_html = if let Some(results) = &self.election_results {
                html! {
                    <p>{ results.status.clone() }</p>
                }
            } else {
                html! {}
            };
            html! {
                <div class="container">
                    <div class="row">
                        <h2>{ format!("Attending meeting: {}", meeting_name) }</h2>
                        {join_info_html}
                        {status_html}
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
        let mut meetings = self.meetings.clone();
        meetings.sort_by(
            |ScoredMeeting { score: a_score, .. }, ScoredMeeting { score: b_score, .. }| {
                a_score.partial_cmp(b_score).unwrap()
            },
        );
        let meetings_html = {
            let ids = meetings.iter().map(|i| i.meeting.id).collect::<Vec<u32>>();
            html! {
                <ranking::Ranking
                    ids={ids.clone()}
                    labels={meetings.iter().map(|i| i.meeting.name.clone()).collect::<Vec<String>>()}
                    scores={meetings.iter().map(|i| i.score).collect::<Vec<u32>>()}
                    registered_counts={Some(meetings.iter().map(|i| i.meeting.n_registered).collect::<Vec<u32>>())}
                    joined_counts={Some(meetings.iter().map(|i| i.meeting.n_joined).collect::<Vec<u32>>())}
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
                <hr/>
                <div class="container">
                    {meetings_html}
                </div>
            </div>
        }
    }

    fn tabs_html(&self, ctx: &Context<Self>) -> Html {
        let link_class = |tag| {
            if self.active_tab == tag {
                "nav-link active"
            } else {
                "nav-link"
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
                <li class="nav-item">
                    <a class={ link_class(Tab::TopicManagment) }
                    aria-current={ac(Tab::TopicManagment)}
                    href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::TopicManagment))}>{ "Topics" }</a>
                </li>
                <li class="nav-item">
                    <a class={ link_class(Tab::MeetingManagement) }
                    aria-current={ac(Tab::MeetingManagement)}
                    href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::MeetingManagement))}>{ "Meetings" }</a>
                </li>
                <li class="nav-item">
                    <a class={ link_class(Tab::MeetingPrep) }
                    aria-current={ac(Tab::MeetingPrep)}
                    href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::MeetingPrep))}>{ "Meet" }</a>
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
            new_meeting_text: "".to_owned(),
            new_topic_text: "".to_owned(),
            user_id: UserIdState::New,
            user_topics: vec![],
            active_tab: Tab::TopicManagment,
            meeting_poll: None,
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
                                if msg.meeting_id == m_id {
                                    Msg::SetElectionResults(msg)
                                } else {
                                    let e = anyhow!("election status response: {:?}", &msg);
                                    Msg::LogError(e)
                                }
                            }
                            Err(e) => Msg::LogError(e),
                        }
                    });
                    true
                }
            }
            Msg::CheckMeetings => {
                match self.active_tab {
                    Tab::MeetingManagement | Tab::MeetingPrep => {
                        ctx.link().send_future(async {
                            match fetch_meetings().await {
                                Ok(meetings) => Msg::SetMeetings(meetings),
                                Err(e) => Msg::LogError(e),
                            }
                        });
                    }
                    _ => self.meeting_poll = None,
                }
                true
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
                    Interval::new(CHECK_ELECTION_MS, move || {
                        link.send_message(Msg::CheckElection)
                    })
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
                if let Some(meeting_to_leave) = self.attending_meeting {
                    let meeting = Box::new(meeting_to_leave);
                    ctx.link().send_future(async {
                        match leave_meeting(meeting.clone()).await {
                            Ok(_) => Msg::LeftMeeting(meeting),
                            Err(e) => Msg::LogError(e),
                        }
                    });
                }
                true
            }
            Msg::LeftMeeting(meeting) => {
                if self.attending_meeting.is_some() && self.attending_meeting.unwrap() == *meeting {
                    self.attending_meeting = None;
                    self.election_results = None;
                    self.vote_poll = None;
                    self.active_tab = Tab::MeetingManagement;
                }
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
                    if results.meeting_id == meeting {
                        if results.topics.is_some() {
                            self.vote_poll = None;
                        }
                        self.election_results = Some(results);
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
            Msg::SetTab(tab) => {
                let prev_tab = self.active_tab.clone();
                self.active_tab = tab.clone();
                if let Some(meeting_id) = self.attending_meeting {
                    if tab == Tab::MeetingPrep && tab != prev_tab {
                        ctx.link().send_message(Msg::CheckMeetings);
                        ctx.link().send_message(Msg::FetchMeetingTopics(meeting_id));
                    }
                }
                if tab.needs_meeting_poll() && !prev_tab.needs_meeting_poll() {
                    let handle = {
                        let link = ctx.link().clone();
                        Interval::new(CHECK_ELECTION_MS, move || {
                            link.send_message(Msg::CheckMeetings)
                        })
                    };
                    self.meeting_poll = Some(handle);
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
            Msg::StartMeeting => {
                if let Some(meeting_id) = self.attending_meeting {
                    let meeting_id = boxed::Box::new(meeting_id);
                    ctx.link().send_future(async {
                        let m_id = *meeting_id;
                        match start_meeting(meeting_id).await {
                            Ok(()) => Msg::FetchMeetingTopics(m_id),
                            Err(e) => Msg::LogError(e),
                        }
                    });
                }
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
        if no_user() {
            return html! {};
        }
        let onkeypress = ctx
            .link()
            .batch_callback(move |e: KeyboardEvent| (e.key() == "Enter").then(|| Msg::AddTopic));
        let new_topic = if let UserIdState::Fetched(_uid) = &self.user_id {
            html! {
                <div class="container">
                    <div class="row">
                        <div class="col text-end">{ "Add new topic:" }</div>
                        <div class="col">
                            <input
                                id="new-topic" type="text" value={self.new_topic_text.clone()}
                                { onkeypress }
                                oninput={ctx.link().callback(|e: InputEvent| {
                                        let input = e.target_unchecked_into::<HtmlInputElement>();
                                        Msg::UpdateNewTopicText(input.value())
                                })}
                            />
                        </div>
                        <div class="col text-start">
                            <button
                                type={"button"} class={"btn"}
                                onclick={ctx.link().callback(|_| Msg::AddTopic)}>{ add_icon() }</button>
                        </div>
                    </div>
                    <hr/>
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
                        Tab::MeetingPrep => {
                            if self.election_results.is_none() || self.election_results.as_ref().unwrap().topics.is_none() {
                                self.meeting_attendance_html(ctx)
                            } else {
                                self.meeting_election_results_html(ctx)
                            }
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
