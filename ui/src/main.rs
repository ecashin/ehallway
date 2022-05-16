use serde::Deserialize;
use wasm_bindgen::prelude::JsValue;
use web_sys::HtmlInputElement;
use yew::prelude::*;

mod js;

enum Msg {
    AddOne,
    AddTopic,
    Noop,
    SetUserId(String),
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
    user_id: UserIdState,
    user_value: Option<i32>,
    new_topic_text: String,
    debug: String,
}

async fn inc_and_fetch() -> i32 {
    let msg: UserValueMessage = reqwasm::http::Request::get("https://localhost/inc")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    msg.metric
}

async fn fetch_user_value() -> Option<i32> {
    let resp = reqwasm::http::Request::get("https://localhost/user_value")
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
    let resp = reqwasm::http::Request::get("https://localhost/user_id")
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
    }
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let mut model = Self {
            user_id: UserIdState::New,
            user_value: None,
            debug: "none".to_owned(),
            new_topic_text: "".to_owned(),
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
            Msg::AddTopic => {
                let text = web_sys::window()
                    .unwrap()
                    .document()
                    .unwrap()
                    .get_element_by_id("new-topic")
                    .unwrap()
                    .first_child();
                self.debug = format!("{:?}", text);
                js::console_log(JsValue::from(text));
                true
            }
            Msg::Noop => true,
            Msg::SetUserId(email) => {
                let msg = format!("got email: {}", &email);
                js::console_log(JsValue::from(msg));
                self.user_id = UserIdState::Fetched(email);
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
        html! {
            <div>
                { user_value }
                { new_topic }
                <p>{ &self.debug }</p>
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
