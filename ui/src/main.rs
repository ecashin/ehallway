use serde::Deserialize;
use yew::prelude::*;

enum Msg {
    AddOne,
    SetUserValue(i32),
}

struct Model {
    value: Option<i32>,
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

#[derive(Clone, Deserialize, PartialEq)]
struct UserValueMessage {
    metric: i32,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self { value: None }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::AddOne => {
                ctx.link()
                    .send_future(async { Msg::SetUserValue(inc_and_fetch().await) });
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
            value
        } else {
            -1
        };
        html! {
            <div>
                <button onclick={ctx.link().callback(|_| Msg::AddOne)}>{ "+1" }</button>
                <p>{ user_value }</p>
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
