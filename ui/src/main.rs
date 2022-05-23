use std::{boxed, collections::HashMap};

use anyhow::{anyhow, Error, Result};
use gloo_net::http;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::JsValue;
use web_sys::HtmlInputElement;
use yew::prelude::*;

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
    DeleteMeeting(u32),
    DeleteTopic(u32),
    DidStoreMeetingScore,
    LogError(Error),
    MeetingDown(u32),
    MeetingUp(u32),
    Noop,
    SetMeetings(HashMap<u32, (String, u32)>),
    SetTab(Tab),
    SetUserId(String),
    SetUserTopics(Vec<UserTopic>), // set in Model
    StoreMeetingScore(u32),        // store to database
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

#[derive(PartialEq)]
enum Tab {
    MeetingManagement,
    MeetingPrep,
    TopicManagment,
}

struct Model {
    meetings: HashMap<u32, (String, u32)>,
    new_meeting_text: String,
    new_topic_text: String,
    user_id: UserIdState,
    user_topics: Vec<UserTopic>,
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

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Eq)]
struct Meeting {
    name: String,
    id: u32,
}

#[derive(Deserialize)]
struct MeetingMessage {
    meeting: Meeting,
    score: u32,
}

#[derive(Deserialize)]
struct UserTopic {
    text: String,
    id: u32,
}

#[derive(Deserialize)]
struct MeetingsMessage {
    meetings: Vec<MeetingMessage>,
}

#[derive(Deserialize)]
struct UserTopicsMessage {
    topics: Vec<UserTopic>,
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
                    let (id, (name, score)) = mtg;
                    (id, (name, i as u32))
                })
                .collect::<HashMap<_, _>>())
        }
        Err(e) => Err(e.into()),
    }
}

async fn fetch_user_topics() -> Result<Vec<UserTopic>> {
    let resp: std::result::Result<UserTopicsMessage, gloo_net::Error> =
        http::Request::get("https://localhost/user_topics")
            .send()
            .await?
            .json()
            .await;
    match resp {
        Ok(msg) => Ok(msg.topics),
        Err(e) => Err(e.into()),
    }
}

#[derive(Serialize)]
struct NewMeeting {
    name: String,
}

#[derive(Serialize)]
struct NewTopic {
    new_topic: String,
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

#[derive(Serialize)]
struct ScoreMessage {
    score: u32,
}
async fn store_meeting_score(meeting_id: boxed::Box<u32>, score: boxed::Box<u32>) -> Result<()> {
    let url = format!("https://localhost/meeting/{}/score", meeting_id);
    gloo_net::http::Request::put(&url)
        .json(&ScoreMessage { score: *score })?
        .send()
        .await?;
    Ok(())
}

async fn add_new_meeting(name: String) -> Result<http::Response> {
    let new_meeting = NewMeeting { name };
    Ok(gloo_net::http::Request::post("https://localhost/meetings")
        .json(&new_meeting)?
        .send()
        .await?)
}

async fn add_new_topic(topic_text: String) -> Result<http::Response> {
    let topic = NewTopic {
        new_topic: topic_text,
    };
    Ok(gloo_net::http::Request::post("https://localhost/topics")
        .json(&topic)?
        .send()
        .await?)
}

#[derive(Clone, Deserialize, PartialEq)]
struct UserIdMessage {
    email: String,
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
    }

    fn meeting_management_html(&self, ctx: &Context<Self>) -> Html {
        let onkeypress = ctx
            .link()
            .batch_callback(move |e: KeyboardEvent| (e.key() == "Enter").then(|| Msg::AddMeeting));

        let new_meeting = if let UserIdState::Fetched(_uid) = &self.user_id {
            html! {
                <div>
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
            html! {
                <div class="row">
                    <div class="col">{ name }</div>
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
                        <button
                            onclick={ctx.link().callback(move |_| Msg::DeleteMeeting(meeting_id))}
                            type={"button"}
                            class={"btn"}
                        >{ x_icon() }</button>
                    </div>
                </div>
            }
        })
        .collect();

        html! {
            <div>
                {new_meeting}
                <div class="container">
                    <div class="col">
                        { "Meetings" }
                    </div>
                    <div class="col">
                    </div>
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
        mtgs.into_iter()
            .enumerate()
            .map(|(i, (id, _score))| (id, i as u32))
            .collect()
    }

    fn tabs_html(&self, ctx: &Context<Self>) -> Html {
        let item_class = |tag| {
            if self.active_tab == tag {
                "nav-item"
            } else {
                "nav-item active"
            }
        };
        html! {
            <ul class="nav nav-tabs">
                <li class={ item_class(Tab::TopicManagment) }>
                    <a class="nav-link" href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::TopicManagment))}>{ "Topics" }</a>
                </li>
                <li class={ item_class(Tab::MeetingManagement) }>
                    <a class="nav-link" href="#" onclick={ctx.link().callback(|_| Msg::SetTab(Tab::MeetingManagement))}>{ "Meetings" }</a>
                </li>
                <li class={ item_class(Tab::MeetingPrep) }>
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
            meetings: HashMap::new(),
            new_meeting_text: "".to_owned(),
            new_topic_text: "".to_owned(),
            user_id: UserIdState::New,
            user_topics: vec![],
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
            Msg::SetMeetings(meetings) => {
                self.meetings = meetings;
                true
            }
            Msg::StoreMeetingScore(meeting_id) => {
                if let Some((_, score)) = self.meetings.get(&meeting_id) {
                    let score = boxed::Box::new(*score);
                    let meeting_id = boxed::Box::new(meeting_id);
                    ctx.link().send_future(async {
                        match store_meeting_score(meeting_id, score).await {
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
            Msg::SetTab(tab) => {
                self.active_tab = tab;
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
                        id="new-topic"
                        type="text"
                        value={self.new_topic_text.clone()}
                        { onkeypress }
                        oninput={ctx.link().callback(|e: InputEvent| {
                                let input = e.target_unchecked_into::<HtmlInputElement>();
                                Msg::UpdateNewTopicText(input.value())
                        })}
                    />
                    <button onclick={ctx.link().callback(|_| Msg::AddTopic)}>{ add_icon() }</button>
                </div>
            }
        } else {
            html! {}
        };
        let topics: Vec<_> = self
            .user_topics
            .iter()
            .map(|topic| {
                let text = topic.text.clone();
                let id = topic.id;
                html! {
                    <div class="row">
                        <div class="col">{ text }</div>
                        <div class="col">
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
                        Tab::MeetingPrep => html!{}
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
