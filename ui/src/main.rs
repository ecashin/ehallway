use std::boxed;

use anyhow::{anyhow, Error, Result};
use gloo_net::http;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::JsValue;
use web_sys::HtmlInputElement;
use yew::prelude::*;

mod js;

enum Msg {
    AddOne,
    AddTopic,
    AddedTopic,
    DeleteTopic(u32),
    LogError(Error),
    Noop,
    SetUserId(String),
    SetUserTopics(Vec<UserTopic>),
    SetUserValue(i32),
    UpdateNewTopicText(String),
}

enum UserIdState {
    New,
    Fetching,
    Fetched(String),
}

impl UserIdState {
    fn is_new(&self) -> bool {
        match self {
            UserIdState::New => true,
            _ => false,
        }
    }
}

struct Model {
    debug: String,
    new_topic_text: String,
    user_id: UserIdState,
    user_topics: Vec<UserTopic>,
    user_value: Option<i32>,
}

async fn inc_and_fetch() -> i32 {
    let msg: UserValueMessage = http::Request::get("https://localhost/inc")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    msg.metric
}

async fn fetch_user_value() -> Option<i32> {
    let resp = http::Request::get("https://localhost/user_value")
        .send()
        .await
        .unwrap()
        .json()
        .await;
    match resp {
        Ok(resp) => {
            let msg: UserValueMessage = resp;
            Some(msg.metric)
        }
        Err(_e) => None,
    }
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
struct UserTopic {
    text: String,
    id: u32,
}

#[derive(Deserialize)]
struct UserTopicsMessage {
    topics: Vec<UserTopic>,
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
struct NewTopic {
    new_topic: String,
}

async fn delete_topic(id: boxed::Box<u32>) -> Result<()> {
    let url = format!("https://localhost/topics/{}", id);
    gloo_net::http::Request::delete(&url).send().await?;
    Ok(())
}

async fn add_new_topic(topic_text: String) -> Result<http::Response> {
    let topic = NewTopic {
        new_topic: topic_text,
    };
    Ok(
        gloo_net::http::Request::post("https://localhost/add-new-topic")
            .json(&topic)?
            .send()
            .await?,
    )
}

#[derive(Clone, Deserialize, PartialEq)]
struct UserValueMessage {
    metric: i32,
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
            if let Some(val) = fetch_user_value().await {
                Msg::SetUserValue(val)
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
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let mut model = Self {
            debug: "none".to_owned(),
            new_topic_text: "".to_owned(),
            user_id: UserIdState::New,
            user_topics: vec![],
            user_value: None,
        };
        model.fetch_user("create", ctx);
        model
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        if self.user_id.is_new() {
            self.fetch_user("update", ctx);
        }
        match msg {
            Msg::AddOne => {
                ctx.link()
                    .send_future(async { Msg::SetUserValue(inc_and_fetch().await) });
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
            Msg::SetUserId(email) => {
                let msg = format!("got email: {}", &email);
                js::console_log(JsValue::from(msg));
                self.user_id = UserIdState::Fetched(email);
                true
            }
            Msg::SetUserTopics(topics) => {
                self.user_topics = topics;
                true
            }
            Msg::SetUserValue(val) => {
                self.user_value = Some(val);
                true
            }
            Msg::UpdateNewTopicText(text) => {
                self.new_topic_text = text;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let user_value = if let Some(value) = self.user_value {
            html! {
                <div>
                    <p>{ value }</p>
                    <button onclick={ctx.link().callback(|_| Msg::AddOne)}>{ "+1" }</button>
                </div>
            }
        } else {
            html! {}
        };
        let new_topic = if let UserIdState::Fetched(_uid) = &self.user_id {
            html! {
                <div>
                    <input
                        id="new-topic"
                        type="text"
                        value={self.new_topic_text.clone()}
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
        html! {
            <div>
                { user_value }
                { new_topic }
                <p>{ &self.debug }</p>
                <table>{ topics }</table>
            </div>
        }
    }
}

fn main() {
    let app_div = gloo_utils::document()
        .get_element_by_id("vhallway")
        .unwrap();
    yew::start_app_in_element::<Model>(app_div);
}
