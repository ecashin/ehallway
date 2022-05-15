use serde::Deserialize;
use wasm_bindgen::prelude::JsValue;
use yew::prelude::*;

mod js;

enum Msg {
    AddOne,
    AddTopic,
    SetUserId(String),
    SetUserValue(i32),
}

struct Model {
    user_id: Option<String>,
    value: Option<i32>,
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

async fn fetch_user_value() -> i32 {
    let msg: UserValueMessage = reqwasm::http::Request::get("https://localhost/user_value")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    msg.metric
}

async fn fetch_user_id() -> String {
    let msg: UserIdMessage = reqwasm::http::Request::get("https://localhost/user_id")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    msg.email
}

#[derive(Clone, Deserialize, PartialEq)]
struct UserValueMessage {
    metric: i32,
}

#[derive(Clone, Deserialize, PartialEq)]
struct UserIdMessage {
    email: String,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link()
            .send_future(async { Msg::SetUserId(fetch_user_id().await) });
        ctx.link()
            .send_future(async { Msg::SetUserValue(fetch_user_value().await) });
        Self {
            user_id: None,
            value: None,
            debug: "none".to_owned(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
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
            Msg::SetUserId(email) => {
                let msg = format!("got email: {}", &email);
                js::console_log(JsValue::from(msg));
                self.user_id = Some(email);
                true
            }
            Msg::SetUserValue(val) => {
                self.value = Some(val);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let user_value = if let Some(value) = self.value {
            html! {
                <div>
                    <p>{ value }</p>
                    <button onclick={ctx.link().callback(|_| Msg::AddOne)}>{ "+1" }</button>
                </div>
            }
        } else {
            html! {}
        };
        let new_topic = if let Some(_uid) = &self.user_id {
            html! {
                <div>
                    <input id="new-topic" type="text"/>
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
