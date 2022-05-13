use serde::Deserialize;
use yew::prelude::*;

enum Msg {
    AddOne,
}

struct Model {
    value: Option<i32>,
}

fn inc_and_fetch() -> i32 {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let msg: UserValueMessage = runtime.block_on(async {
        reqwest::Client::new()
            .get("https://localhost/inc")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    });
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

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::AddOne => {
                self.value = Some(inc_and_fetch());
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let user_value = if let Some(value) = self.value {
            value
        } else {
            0
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
