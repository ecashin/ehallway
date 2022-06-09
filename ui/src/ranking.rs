use yew::{html, Callback, Component, Context, Html, Properties};

use crate::svg::{down_arrow, up_arrow, x_icon};

#[derive(Clone, Debug, PartialEq, Properties)]
pub struct Props {
    pub ids: Vec<u32>,
    pub labels: Vec<String>,
    pub scores: Vec<u32>,
    pub store_score: Callback<(u32, u32)>,
    pub delete: Option<Callback<u32>>,
}

pub enum Msg {
    Delete(u32),
    Down(u32),
    Up(u32),
}

pub fn argsort<T>(a: &[T]) -> Vec<usize>
where
    T: PartialOrd,
{
    let mut indexed: Vec<_> = a.iter().enumerate().collect();
    indexed.sort_by(|(_i1, v1), (_i2, v2)| v1.partial_cmp(v2).unwrap());
    indexed.into_iter().map(|(i, _v)| i).collect()
}

pub struct Ranking {}

impl Component for Ranking {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }
    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Delete(id) => {
                if ctx.props().delete.is_some() {
                    ctx.props().delete.as_ref().unwrap().emit(id);
                    true
                } else {
                    false
                }
            }
            Msg::Down(id) => {
                let order = argsort(&ctx.props().scores);
                if let Some(pos) = ctx.props().ids.iter().position(|&i| i == id) {
                    if order[pos] == 0 {
                        false
                    } else {
                        let below =
                            ctx.props().ids[order.iter().position(|&i| i == pos - 1).unwrap()];
                        ctx.props().store_score.emit((below, pos as u32));
                        ctx.props().store_score.emit((id, (pos - 1) as u32));
                        true
                    }
                } else {
                    false
                }
            }
            Msg::Up(id) => {
                let order = argsort(&ctx.props().scores);
                if let Some(pos) = ctx.props().ids.iter().position(|&i| i == id) {
                    if order[pos] == order.len() - 1 {
                        false
                    } else {
                        let above =
                            ctx.props().ids[order.iter().position(|&i| i == pos + 1).unwrap()];
                        ctx.props().store_score.emit((above, pos as u32));
                        ctx.props().store_score.emit((id, (pos + 1) as u32));
                        true
                    }
                } else {
                    false
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let Props {
            delete,
            ids,
            labels,
            scores,
            ..
        } = ctx.props();
        let order = argsort(scores);
        let mut items: Vec<_> = vec![];

        for i in order.into_iter().rev() {
            let id = ids[i];
            let delete_html = if delete.is_some() {
                html! {
                    <div class="col">
                        <button
                        onclick={ctx.link().callback(move |_| Msg::Delete(id))}
                        type={"button"}
                        class={"btn"}
                        >{ x_icon() }</button>
                    </div>
                }
            } else {
                html! {}
            };
            items.push(html! {
                <div class={"row"}>
                    <div class="col">
                        {labels[i].clone()}
                    </div>
                    <div class="col">
                        <button
                        onclick={ctx.link().callback(move |_| Msg::Up(id))}
                        type={"button"}
                        class={"btn"}
                        >{ up_arrow() }</button>
                        <button
                        onclick={ctx.link().callback(move |_| Msg::Down(id))}
                        type={"button"}
                        class={"btn"}
                        >{ down_arrow() }</button>
                    </div>
                    {delete_html}
                </div>
            });
        }
        html! {
            <div class="container">
                {items}
            </div>
        }
    }
}
