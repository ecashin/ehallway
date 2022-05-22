use std::boxed;

use anyhow::{anyhow, Error, Result};
use gloo_net::http;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::JsValue;
use web_sys::HtmlInputElement;
use yew::prelude::*;

mod chance;
mod cull;
mod js;

enum Msg {
    AddMeeting,
    AddTopic,
    AddedMeeting,
    AddedTopic,
    DeleteMeeting(u32),
    DeleteTopic(u32),
    LogError(Error),
    Noop,
    SetMeetings(Vec<Meeting>),
    SetTab(Tab),
    SetUserId(String),
    SetUserTopics(Vec<UserTopic>),
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
    meetings: Vec<Meeting>,
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

#[derive(Deserialize)]
struct Meeting {
    name: String,
    id: u32,
    score: u32,
}

#[derive(Deserialize)]
struct UserTopic {
    text: String,
    id: u32,
}

#[derive(Deserialize)]
struct MeetingsMessage {
    meetings: Vec<Meeting>,
}

#[derive(Deserialize)]
struct UserTopicsMessage {
    topics: Vec<UserTopic>,
}

async fn fetch_meetings() -> Result<Vec<Meeting>> {
    let resp: std::result::Result<MeetingsMessage, gloo_net::Error> =
        http::Request::get("https://localhost/meetings")
            .send()
            .await?
            .json()
            .await;
    match resp {
        Ok(msg) => Ok(msg.meetings),
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
                    <button onclick={ctx.link().callback(|_| Msg::AddMeeting)}>{ "Add Meeting" }</button>
                </div>
            }
        } else {
            html! {}
        };
        let meetings: Vec<_> = self
        .meetings
        .iter()
        .map(|meeting| {
            let name = meeting.name.clone();
            let id = meeting.id;
            html! {
                <tr>
                    <td>{ name }</td>
                    <td>{ meeting.score }</td>
                    <td>
                        <button onclick={ctx.link().callback(move |_| Msg::DeleteMeeting(id))}>{"DELETE"}</button>
                    </td>
                </tr>
            }
        })
        .collect();

        html! {
            <div>
                {new_meeting}
                <table>
                    <tr>
                        <th>{ "name" }</th>
                        <th>{ "score" }</th>
                        <th></th>
                    </tr>
                    {meetings}
                </table>
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
            meetings: vec![],
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
            Msg::LogError(e) => {
                js::console_log(JsValue::from(format!("{e}")));
                true
            }
            Msg::Noop => true,
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
            Msg::SetMeetings(meetings) => {
                self.meetings = meetings;
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
                    <button onclick={ctx.link().callback(|_| Msg::AddTopic)}>{ "Add Topic" }</button>
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
                    <tr>
                        <td>{ text }</td>
                        <td>
                            <button onclick={ctx.link().callback(move |_| Msg::DeleteTopic(id))}>{"DELETE"}</button>
                        </td>
                    </tr>
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
                                    <table>{ topics }</table>
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
