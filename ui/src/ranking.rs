use yew::{html, Callback, Component, Context, Html, Properties};

use crate::svg::{down_arrow, up_arrow, x_icon};

#[derive(Clone, Debug, PartialEq, Properties)]
pub struct Props {
    pub ids: Vec<u32>,
    pub labels: Vec<String>,
    pub scores: Vec<u32>,
    pub store_score: Callback<(u32, u32)>,
    pub delete: Option<Callback<u32>>,
    pub is_registered: Option<Vec<bool>>,
    pub attend_meeting: Option<Callback<u32>>,
    pub register_toggle: Option<Callback<u32>>,
}

pub enum Msg {
    AttendMeeting(u32),
    Delete(u32),
    Down(u32),
    RegisterToggle(u32),
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
            Msg::AttendMeeting(id) => {
                if ctx.props().attend_meeting.is_some() {
                    ctx.props().attend_meeting.as_ref().unwrap().emit(id);
                    true
                } else {
                    false
                }
            }
            Msg::Delete(id) => {
                if ctx.props().delete.is_some() {
                    ctx.props().delete.as_ref().unwrap().emit(id);
                    true
                } else {
                    false
                }
            }
            Msg::Down(id) => {
                let scores = &ctx.props().scores;
                let ids = &ctx.props().ids;
                let order = argsort(scores);
                if let Some(pos) = ids.iter().position(|&i| i == id) {
                    if order[pos] == 0 {
                        false
                    } else {
                        let i_below = order.iter().position(|&i| i == order[pos] - 1).unwrap();
                        ctx.props()
                            .store_score
                            .emit((ids[i_below], scores[pos] as u32));
                        ctx.props().store_score.emit((id, (scores[i_below]) as u32));
                        true
                    }
                } else {
                    false
                }
            }
            Msg::RegisterToggle(id) => {
                if ctx.props().register_toggle.is_some() {
                    ctx.props().register_toggle.as_ref().unwrap().emit(id);
                    true
                } else {
                    false
                }
            }
            Msg::Up(id) => {
                let scores = &ctx.props().scores;
                let ids = &ctx.props().ids;
                let order = argsort(scores);
                if let Some(pos) = ids.iter().position(|&i| i == id) {
                    if order[pos] == ids.len() - 1 {
                        false
                    } else {
                        let i_above = order.iter().position(|&i| i == order[pos] + 1).unwrap();
                        ctx.props()
                            .store_score
                            .emit((ids[i_above], scores[pos] as u32));
                        ctx.props().store_score.emit((id, (scores[i_above]) as u32));
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
            is_registered,
            attend_meeting,
            register_toggle,
            ..
        } = ctx.props();
        let order = argsort(scores);
        let mut items: Vec<_> = vec![];

        for i in order.into_iter().rev() {
            let id = ids[i];
            let attend_meeting_html = if attend_meeting.is_some() {
                let is_reg = is_registered.as_ref().unwrap()[i];
                html! {
                    <div class="col">
                        <button
                            onclick={ctx.link().callback(move |_| Msg::AttendMeeting(id))}
                            disabled={!is_reg}
                            type={"button"}
                            class={"btn btn-secondary"}
                        >{"join now"}</button>
                    </div>
                }
            } else {
                html! {}
            };
            let register_toggle_html = if register_toggle.is_some() {
                let is_reg = is_registered.as_ref().unwrap()[i];
                let register_id = format!("register{id}");
                let register_class = if is_reg {
                    "btn btn-primary"
                } else {
                    "btn btn-secondary"
                };
                html! {
                    <div class="col">
                        <input
                            id={register_id.clone()}
                            class="btn-check"
                            type={"checkbox"}
                            checked={ is_reg }
                            autocomplete={"off"}
                            onclick={ctx.link().callback(move |_| Msg::RegisterToggle(id))}
                        />
                        <label
                            class={register_class}
                            for={register_id}>{"register"}
                        </label>
                    </div>
                }
            } else {
                html! {}
            };
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
                    {attend_meeting_html}
                    {register_toggle_html}
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
