use std::{
    borrow::Cow,
    boxed,
    collections::{HashMap, HashSet},
};

use anyhow::{anyhow, Error, Result};
use gloo_net::http;
use wasm_bindgen::prelude::JsValue;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use ehall::{
    MeetingParticipantsMessage, MeetingsMessage, NewMeeting, NewTopicMessage,
    ParticipateMeetingMessage, RegisteredMeetingsMessage, ScoreMessage, UserIdMessage, UserTopic,
    UserTopicsMessage,
};

mod chance;
mod cull;
mod js;

fn add_icon() -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg"
            width="16" height="16" fill="currentColor"
            class="bi bi-plus-square" viewBox="0 0 16 16"
        >
            <path d="M14 1a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1h12zM2 0a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V2a2 2 0 0 0-2-2H2z"/>
            <path d="M8 4a.5.5 0 0 1 .5.5v3h3a.5.5 0 0 1 0 1h-3v3a.5.5 0 0 1-1 0v-3h-3a.5.5 0 0 1 0-1h3v-3A.5.5 0 0 1 8 4z"/>
        </svg>
    }
}

fn x_icon() -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            fill="currentColor"
            class="bi bi-x-square"
            viewBox="0 0 16 16"
        >
            <path d="M14 1a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1h12zM2 0a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V2a2 2 0 0 0-2-2H2z">
            </path>
            <path d="M4.646 4.646a.5.5 0 0 1 .708 0L8 7.293l2.646-2.647a.5.5 0 0 1 .708.708L8.707 8l2.647 2.646a.5.5 0 0 1-.708.708L8 8.707l-2.646 2.647a.5.5 0 0 1-.708-.708L7.293 8 4.646 5.354a.5.5 0 0 1 0-.708z">
            </path>
        </svg>
    }
}

// https://icons.getbootstrap.com/icons/arrow-down-square/
fn down_arrow() -> Html {
    html! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16" height="16" fill="currentColor"
            class="bi bi-arrow-down-square"
            viewBox="0 0 16 16"
        >
            <path fill-rule="evenodd" d="M15 2a1 1 0 0 0-1-1H2a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V2zM0 2a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2V2zm8.5 2.5a.5.5 0 0 0-1 0v5.793L5.354 8.146a.5.5 0 1 0-.708.708l3 3a.5.5 0 0 0 .708 0l3-3a.5.5 0 0 0-.708-.708L8.5 10.293V4.5z"/>
        </svg>
    }
}

fn up_arrow() -> Html {
    html! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            fill="currentColor"
            class="bi bi-arrow-up-square"
            viewBox="0 0 16 16"
        >
            <path fill-rule="evenodd" d="M15 2a1 1 0 0 0-1-1H2a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V2zM0 2a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2V2zm8.5 9.5a.5.5 0 0 1-1 0V5.707L5.354 7.854a.5.5 0 1 1-.708-.708l3-3a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1-.708.708L8.5 5.707V11.5z"/>
        </svg>
    }
}

enum Msg {
    AddMeeting,
    AddTopic,
    AddedMeeting,
    AddedTopic,
    AttendingMeeting(boxed::Box<u32>),
    AttendMeeting(u32),
    DeleteMeeting(u32),
    DeleteTopic(u32),
    DidStoreMeetingScore,
    LeaveMeeting,
    LogError(Error),
    MeetingDown(u32),
    MeetingRegisteredChanged,
    MeetingToggleRegistered(u32),
    MeetingUp(u32),
    Noop,
    SetNRegisteredNJoined((u32, u32)),
    SetRegisteredMeetings(Vec<u32>),
    SetMeetings(HashMap<u32, (String, u32)>),
    SetTab(Tab),
    SetUserId(String),
    SetUserTopics(HashMap<u32, UserTopic>), // set in Model
    StoreMeetingScore(u32),                 // store to database
    StoreTopicScore(u32),
    TopicDown(u32),
    TopicUp(u32),
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
    attending_meeting: Option<u32>,
    n_attending_meeting_registered: Option<u32>,
    n_attending_meeting_joined: Option<u32>,
    registered_meetings: HashSet<u32>,
    meetings: HashMap<u32, (String, u32)>,
    new_meeting_text: String,
    new_topic_text: String,
    user_id: UserIdState,
    user_topics: HashMap<u32, UserTopic>,
    active_tab: Tab,
}

async fn fetch_user_id() -> Option<String> {
    let resp = http::Request::get("https://localhost/user_id")
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

async fn fetch_meetings() -> Result<HashMap<u32, (String, u32)>> {
    let resp: std::result::Result<MeetingsMessage, gloo_net::Error> =
        http::Request::get("https://localhost/meetings")
            .send()
            .await?
            .json()
            .await;
    match resp {
        Ok(msg) => {
            let mut mtgs: Vec<_> = msg
                .meetings
                .iter()
                .map(|mm| (mm.meeting.id, (mm.meeting.name.clone(), mm.score)))
                .collect();
            mtgs.sort_by(|(_, (_, a)), (_, (_, b))| a.partial_cmp(b).unwrap());
            Ok(mtgs
                .into_iter()
                .enumerate()
                .map(|(i, mtg)| {
                    let (id, (name, _score)) = mtg;
                    (id, (name, i as u32))
                })
                .collect::<HashMap<_, _>>())
        }
        Err(e) => Err(e.into()),
    }
}

async fn fetch_n_meeting_participants(
    meeting_id: boxed::Box<u32>,
) -> Result<MeetingParticipantsMessage> {
    let url = format!("https://localhost/meeting/{meeting_id}/participant_counts");
    let resp: std::result::Result<MeetingParticipantsMessage, gloo_net::Error> =
        http::Request::get(&url).send().await?.json().await;
    match resp {
        Ok(msg) => Ok(msg),
        Err(e) => Err(e.into()),
    }
}

async fn fetch_registered_meetings() -> Result<Vec<u32>> {
    let resp: std::result::Result<RegisteredMeetingsMessage, gloo_net::Error> =
        http::Request::get("https://localhost/registered_meetings")
            .send()
            .await?
            .json()
            .await;
    match resp {
        Ok(msg) => Ok(msg.meetings),
        Err(e) => Err(e.into()),
    }
}

async fn fetch_user_topics() -> Result<HashMap<u32, UserTopic>> {
    let resp: std::result::Result<UserTopicsMessage, gloo_net::Error> =
        http::Request::get("https://localhost/user_topics")
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
            Ok(topics
                .into_iter()
                .enumerate()
                .map(|(score, UserTopic { text, id, .. })| {
                    (
                        id,
                        UserTopic {
                            id,
                            text,
                            score: score as u32,
                        },
                    )
                })
                .collect::<HashMap<_, _>>())
        }
        Err(e) => Err(e.into()),
    }
}

async fn delete_meeting(id: boxed::Box<u32>) -> Result<()> {
    let url = format!("https://localhost/meetings/{}", id);
    gloo_net::http::Request::delete(&url).send().await?;
    Ok(())
}

async fn delete_topic(id: boxed::Box<u32>) -> Result<()> {
    let url = format!("https://localhost/topics/{}", id);
    gloo_net::http::Request::delete(&url).send().await?;
    Ok(())
}

async fn store_score(
    what: &str,
    meeting_id: boxed::Box<u32>,
    score: boxed::Box<u32>,
) -> Result<()> {
    let url = format!("https://localhost/{what}/{}/score", meeting_id);
    gloo_net::http::Request::put(&url)
        .json(&ScoreMessage { score: *score })?
        .send()
        .await?;
    Ok(())
}

async fn attend_meeting(meeting_id: boxed::Box<u32>) -> Result<http::Response> {
    let url = format!("https://localhost/meeting/{}/attendees", *meeting_id);
    Ok(gloo_net::http::Request::post(&url).send().await?)
}

async fn add_new_meeting(name: String) -> Result<http::Response> {
    let new_meeting = NewMeeting {
        name: Cow::from(name),
    };
    Ok(gloo_net::http::Request::post("https://localhost/meetings")
        .json(&new_meeting)?
        .send()
        .await?)
}

async fn add_new_topic(topic_text: String) -> Result<http::Response> {
    let topic = NewTopicMessage {
        new_topic: topic_text,
    };
    Ok(gloo_net::http::Request::post("https://localhost/topics")
        .json(&topic)?
        .send()
        .await?)
}

async fn register_for_meeting(id: boxed::Box<u32>, participate: bool) -> Result<http::Response> {
    let id = *id;
    let url = format!("https://localhost/meeting/{id}/participants");
    Ok(gloo_net::http::Request::post(&url)
        .json(&ParticipateMeetingMessage { participate })?
        .send()
        .await?)
}

impl Model {
    fn fetch_user(&mut self, tag: &str, ctx: &Context<Self>) {
        self.user_id = UserIdState::Fetching;
        js::console_log(JsValue::from(format!("fetch_user in {}", tag)));
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
            let meeting_name = &self.meetings.get(&meeting_id).unwrap().0;
            let join_info_html = if let Some(n_registered) = self.n_attending_meeting_registered {
                let n_joined = self.n_attending_meeting_joined.unwrap();
                html! {
                    <h3>{format!("{n_joined} of {n_registered} registered participants have joined")}</h3>
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
            .map(|(id, (name, score))| (*id, name.clone(), *score))
            .collect();
        meetings.sort_by(|(_a_id, _a_name, a_score), (_b_id, _b_name, b_score)| {
            b_score.partial_cmp(a_score).unwrap()
        });
        let meetings: Vec<_> = meetings.into_iter()
        .map(|(meeting_id, name, _score)| {
            let is_registered = self.registered_meetings.get(&meeting_id).is_some();
            let register_id = format!("register{meeting_id}");
            let register_class = if is_registered {
                "btn btn-primary"
            } else {
                "btn btn-secondary"
            };
            html! {
                <div class="row">
                    <div class="col">{ name }</div>
                    <div class="col">
                        <div class={"container"}>
                            <div class={"row"}>
                                <div class="col">
                                    <button
                                        onclick={ctx.link().callback(move |_| Msg::AttendMeeting(meeting_id))}
                                        disabled={!is_registered}
                                        type={"button"}
                                        class={"btn btn-secondary"}
                                    >{"join now"}</button>
                                </div>
                                <div class="col">
                                    <input
                                        id={register_id.clone()}
                                        class="btn-check"
                                        type={"checkbox"}
                                        checked={ is_registered }
                                        autocomplete={"off"}
                                        onclick={ctx.link().callback(move |_| Msg::MeetingToggleRegistered(meeting_id))}
                                    />
                                    <label
                                        class={register_class}
                                        for={register_id}>{"register"}
                                    </label>
                                </div>
                                <div class="col">
                                    <button
                                    onclick={ctx.link().callback(move |_| Msg::MeetingUp(meeting_id))}
                                    type={"button"}
                                    class={"btn"}
                                    >{ up_arrow() }</button>
                                    <button
                                    onclick={ctx.link().callback(move |_| Msg::MeetingDown(meeting_id))}
                                    type={"button"}
                                    class={"btn"}
                                    >{ down_arrow() }</button>
                                </div>
                                <div class="col">
                                    <button
                                    onclick={ctx.link().callback(move |_| Msg::DeleteMeeting(meeting_id))}
                                    type={"button"}
                                    class={"btn"}
                                    >{ x_icon() }</button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            }
        })
        .collect();

        html! {
            <div>
                {new_meeting}
                <div class="container">
                    {meetings}
                </div>
            </div>
        }
    }

    fn sorted_by_score_meetings(&self) -> Vec<(u32, u32)> {
        let mut mtgs: Vec<_> = self
            .meetings
            .iter()
            .map(|(id, (_name, score))| (*id, *score))
            .collect();
        mtgs.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
        mtgs
    }

    fn sorted_by_score_topics(&self) -> Vec<(u32, u32)> {
        let mut topics: Vec<_> = self
            .user_topics
            .iter()
            .map(
                |(
                    _,
                    UserTopic {
                        id,
                        score,
                        text: _text,
                    },
                )| (*id, *score),
            )
            .collect();
        topics.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
        topics
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
            registered_meetings: HashSet::new(),
            meetings: HashMap::new(),
            n_attending_meeting_joined: None,
            n_attending_meeting_registered: None,
            new_meeting_text: "".to_owned(),
            new_topic_text: "".to_owned(),
            user_id: UserIdState::New,
            user_topics: HashMap::new(),
            active_tab: Tab::TopicManagment,
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
                ctx.link().send_future(async {
                    match fetch_user_topics().await {
                        Ok(topics) => Msg::SetUserTopics(topics),
                        Err(e) => Msg::LogError(e),
                    }
                });
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
            Msg::DeleteTopic(id) => {
                let id = boxed::Box::new(id);
                ctx.link().send_future(async {
                    match delete_topic(id).await {
                        Ok(_) => Msg::AddedTopic,
                        Err(e) => Msg::LogError(e),
                    }
                });
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
            Msg::LeaveMeeting => {
                self.attending_meeting = None;
                self.active_tab = Tab::MeetingManagement;
                true
            }
            Msg::LogError(e) => {
                js::console_log(JsValue::from(format!("{e}")));
                true
            }
            Msg::MeetingDown(down_id) => {
                let mut mtgs = self.sorted_by_score_meetings();
                if let Some(pos) = mtgs.iter().position(|(id, _score)| *id == down_id) {
                    if pos > 0 && mtgs.len() > 1 {
                        mtgs[pos].1 -= 1;
                        mtgs[pos - 1].1 += 1;
                        for (id, score) in mtgs {
                            self.meetings.entry(id).and_modify(|(_, entry_score)| {
                                let modified = *entry_score != score;
                                *entry_score = score;
                                if modified {
                                    ctx.link().send_message(Msg::StoreMeetingScore(id));
                                }
                            });
                        }
                    }
                }
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
            Msg::MeetingUp(up_id) => {
                let mut mtgs = self.sorted_by_score_meetings();
                if let Some(pos) = mtgs.iter().position(|(id, _score)| *id == up_id) {
                    if pos < mtgs.len() - 1 && mtgs.len() > 1 {
                        mtgs[pos].1 += 1;
                        mtgs[pos + 1].1 -= 1;
                        for (id, score) in mtgs {
                            self.meetings.entry(id).and_modify(|(_, entry_score)| {
                                let modified = *entry_score != score;
                                *entry_score = score;
                                if modified {
                                    ctx.link().send_message(Msg::StoreMeetingScore(id));
                                }
                            });
                        }
                    }
                }
                true
            }
            Msg::Noop => true,
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
                    }
                }
                true
            }
            Msg::SetUserId(email) => {
                let msg = format!("got email: {}", &email);
                js::console_log(JsValue::from(msg));
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
            Msg::StoreMeetingScore(meeting_id) => {
                if let Some((_, score)) = self.meetings.get(&meeting_id) {
                    let score = boxed::Box::new(*score);
                    let meeting_id = boxed::Box::new(meeting_id);
                    ctx.link().send_future(async {
                        match store_score("meeting", meeting_id, score).await {
                            Ok(_) => Msg::DidStoreMeetingScore,
                            Err(e) => Msg::LogError(e),
                        }
                    });
                } else {
                    js::console_log(JsValue::from(format!(
                        "meeting ID without score: {:?}",
                        meeting_id
                    )));
                }
                true
            }
            Msg::StoreTopicScore(id) => {
                if let Some(topic) = self.user_topics.get(&id) {
                    let score = boxed::Box::new(topic.score);
                    let id = boxed::Box::new(id);
                    ctx.link().send_future(async {
                        match store_score("topic", id, score).await {
                            Ok(_) => Msg::DidStoreMeetingScore,
                            Err(e) => Msg::LogError(e),
                        }
                    });
                } else {
                    js::console_log(JsValue::from(format!("topic ID without score: {id}",)));
                }
                true
            }
            Msg::TopicDown(down_id) => {
                let mut topics = self.sorted_by_score_topics();
                if let Some(pos) = topics.iter().position(|(id, _score)| *id == down_id) {
                    if pos > 0 && topics.len() > 1 {
                        topics[pos].1 -= 1;
                        topics[pos - 1].1 += 1;
                        for (id, score) in topics {
                            self.user_topics.entry(id).and_modify(|topic| {
                                if topic.score != score {
                                    topic.score = score;
                                    ctx.link().send_message(Msg::StoreTopicScore(id));
                                }
                            });
                        }
                    }
                }
                true
            }
            Msg::TopicUp(up_id) => {
                let mut topics = self.sorted_by_score_topics();
                if let Some(pos) = topics.iter().position(|(id, _score)| *id == up_id) {
                    if pos < topics.len() - 1 && topics.len() > 1 {
                        topics[pos].1 += 1;
                        topics[pos + 1].1 -= 1;
                        for (id, score) in topics {
                            self.user_topics.entry(id).and_modify(|topic| {
                                if topic.score != score {
                                    topic.score = score;
                                    ctx.link().send_message(Msg::StoreMeetingScore(id));
                                }
                            });
                        }
                    }
                }
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
        let mut topics: Vec<_> = self
            .user_topics
            .iter()
            .map(|(_, UserTopic { id, score, text })| (*id, text, *score))
            .collect();
        topics.sort_by(|(_, _, a_score), (_, _, b_score)| a_score.partial_cmp(b_score).unwrap());
        let topics: Vec<_> = topics
            .into_iter()
            .rev()
            .map(|(id, text, _score)| {
                html! {
                    <div class="row">
                        <div class="col">{ text }</div>
                        <div class="col">
                            <button
                                onclick={ctx.link().callback(move |_| Msg::TopicUp(id))}
                                type={"button"}
                                class={"btn"}
                            >{ up_arrow() }</button>
                            <button
                                onclick={ctx.link().callback(move |_| Msg::TopicDown(id))}
                                type={"button"}
                                class={"btn"}
                            >{ down_arrow() }</button>
                            <button
                                onclick={ctx.link().callback(move |_| Msg::DeleteTopic(id))}
                                type={"button"}
                                class={"btn"}
                            >{ x_icon() }</button>
                        </div>
                    </div>
                }
            })
            .collect();
        let main_panel = html! {
            <div>
                { self.tabs_html(ctx) }
                {
                    match self.active_tab {
                        Tab::TopicManagment => {
                            html! {
                                <div>
                                    { new_topic }
                                    <div class="container">{ topics }</div>
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
