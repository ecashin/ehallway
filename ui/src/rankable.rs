use yew::{html, Callback, Component, Context, Html, Properties};

use crate::svg::{down_arrow, up_arrow, x_icon};

#[derive(Clone, Debug, PartialEq, Properties)]
pub struct Props {
    pub label: String,
    pub on_down: Callback<()>,
    pub on_up: Callback<()>,
    pub on_delete: Option<Callback<()>>,
}

pub enum Msg {
    Delete,
    Down,
    Up,
}

pub struct Rankable {}

impl Component for Rankable {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }
    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Delete => {
                ctx.props().on_delete.as_ref().unwrap().emit(());
                true
            }
            Msg::Down => {
                ctx.props().on_down.emit(());
                true
            }
            Msg::Up => {
                ctx.props().on_up.emit(());
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let Props {
            label, on_delete, ..
        } = ctx.props();

        let delete_html = if on_delete.is_some() {
            html! {
                <div class="col">
                    <button
                    onclick={ctx.link().callback(move |_| Msg::Delete)}
                    type={"button"}
                    class={"btn"}
                    >{ x_icon() }</button>
                </div>
            }
        } else {
            html! {}
        };
        html! {
            <div class={"row"}>
                <div class="col">
                    {label}
                </div>
                <div class="col">
                    <button
                    onclick={ctx.link().callback(move |_| Msg::Up)}
                    type={"button"}
                    class={"btn"}
                    >{ up_arrow() }</button>
                    <button
                    onclick={ctx.link().callback(move |_| Msg::Down)}
                    type={"button"}
                    class={"btn"}
                    >{ down_arrow() }</button>
                </div>
                {delete_html}
            </div>
        }
    }
}
