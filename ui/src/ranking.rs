use yew::{html, Callback, Component, Context, Html, Properties};

use ehall::argsort;

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

        for (list_item_offset, i) in order.into_iter().rev().enumerate() {
            let id = ids[i];
            let attend_meeting_html =
                if attend_meeting.is_some() && is_registered.as_ref().unwrap()[i] {
                    html! {
                        <td>
                            <button
                                onclick={ctx.link().callback(move |_| Msg::AttendMeeting(id))}
                                type={"button"}
                                class={"btn btn-secondary"}
                            >{"join now"}</button>
                        </td>
                    }
                } else {
                    html! { <td></td> }
                };
            let register_toggle_html = if register_toggle.is_some() {
                let is_reg = is_registered.as_ref().unwrap()[i];
                let register_id = format!("register{id}");
                html! {
                    <td>
                        <div class="form-check">
                            <input
                                id={register_id.clone()}
                                class="form-check-input"
                                type={"checkbox"}
                                value=""
                                checked={ is_reg }
                                autocomplete={"off"}
                                onclick={ctx.link().callback(move |_| Msg::RegisterToggle(id))}
                            />
                            <label
                                class="form-check-label"
                                for={register_id}>{"register"}
                            </label>
                        </div>
                    </td>
                }
            } else {
                html! { <td></td> }
            };
            let delete_html = if delete.is_some() {
                html! {
                    <td>
                        <button
                        onclick={ctx.link().callback(move |_| Msg::Delete(id))}
                        type={"button"}
                        class={"btn"}
                        >{ x_icon() }</button>
                    </td>
                }
            } else {
                html! { <td></td> }
            };
            let up_button = if list_item_offset == 0 {
                html! {}
            } else {
                html! {
                    <button
                    onclick={ctx.link().callback(move |_| Msg::Up(id))}
                    type={"button"}
                    class={"btn"}
                    >{ up_arrow() }</button>
                }
            };
            let down_button = if list_item_offset == scores.len() - 1 {
                html! {}
            } else {
                html! {
                    <button
                    onclick={ctx.link().callback(move |_| Msg::Down(id))}
                    type={"button"}
                    class={"btn"}
                    >{ down_arrow() }</button>
                }
            };
            items.push(html! {
                <tr>
                    {attend_meeting_html}
                    {register_toggle_html}
                    <td>
                        {labels[i].clone()}
                    </td>
                    <td>
                        {up_button}
                    </td>
                    <td>
                        {down_button}
                    </td>
                    {delete_html}
                </tr>
            });
        }
        html! {
            <table class="table table-striped">
                <tbody>
                    {items}
                </tbody>
            </table>
        }
    }
}
