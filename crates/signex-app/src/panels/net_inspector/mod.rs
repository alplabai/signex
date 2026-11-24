//! Net Inspector panel (#1269): every net of the active document's project,
//! read from the cached stitched project net index (`signex_net::NetIndex`),
//! listed By Net (a net expands to its pins, labels, power ports and sheet
//! entries) or By Component (a part expands to its pins and the net of
//! each). Clicking a member or a wired pin goes to it
//! (`handlers/net_nav/inspector.rs`). A pure view over data the panel
//! context copies from the index; nothing here re-derives a net.

mod board;
mod by_component;
mod by_net;
mod model;

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_net::NetAnchor;
use signex_widgets::segmented::segmented;
use signex_widgets::{text, text_input};

pub(crate) use model::preview;
pub use model::{
    BoardNetRow, BoardNetView, ComponentPinRow, ComponentRow, MEMBER_PREVIEW, NetInspectorState,
    NetInspectorTab, NetInspectorView, NetListStatus, NetMemberRow, NetMemberRowKind, NetRow,
};

use super::{EmptyStateAction, PanelContext, PanelMsg, empty_state_card_with_actions};
use crate::styles::{Spacing, TypeScale, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What happened in the Net Inspector panel.
#[derive(Debug, Clone)]
pub enum NetInspectorMsg {
    FilterChanged(String),
    /// The no-match card's Clear filter was pressed.
    FilterCleared,
    /// The By Net / By Component control picked a tab.
    TabSelected(NetInspectorTab),
    /// A net row was clicked: expand or fold it.
    NetToggled(String),
    /// A part row was clicked: expand or fold it. Carries the row's key.
    ComponentToggled(String),
    /// The "+ N more" row of an expanded net or part (by net name or part
    /// key, in the current tab) was clicked: list every member.
    MoreMembersShown(String),
    /// A member row or a wired pin row was clicked: go to it and highlight
    /// its net. `net` is the name it was listed under; `anchor` finds it
    /// again in a rebuilt index.
    MemberClicked {
        net: String,
        anchor: NetAnchor,
    },
    /// The Unavailable card's Open Messages was pressed.
    OpenMessagesPressed,
    /// A board net row's Select (#2403): select the net on the board.
    BoardNetSelected(String),
    /// A board net row's Highlight: draw the net bright, or no longer.
    BoardNetHighlighted(String),
    /// The pointer entered a schematic net row (mockup row 58).
    NetRowEntered(String),
    /// The pointer left a schematic net row.
    NetRowLeft(String),
    /// A schematic net row's Highlight, shown on hover: draw the net bright
    /// on every sheet, or no longer.
    NetHighlighted(String),
}

fn msg(m: NetInspectorMsg) -> PanelMsg {
    PanelMsg::NetInspector(m)
}

/// Render the panel.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let view = &ctx.net_inspector;
    if let Some(board) = &view.board {
        return board::view(ctx, board);
    }
    let card = |title: Localized, body: Localized, actions: Vec<EmptyStateAction<PanelMsg>>| {
        container(empty_state_card_with_actions(
            &ctx.tokens,
            title,
            body,
            None,
            actions,
        ))
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
    };
    match view.status {
        NetListStatus::NoProject => card(
            t!(i18n, ids::PANELS_NET_INSPECTOR_NO_PROJECT_TITLE),
            t!(i18n, ids::PANELS_NET_INSPECTOR_NO_PROJECT),
            Vec::new(),
        ),
        NetListStatus::Reading => card(
            t!(i18n, ids::PANELS_NET_INSPECTOR_READING_TITLE),
            t!(i18n, ids::PANELS_NET_INSPECTOR_READING),
            Vec::new(),
        ),
        NetListStatus::Unavailable => card(
            t!(i18n, ids::PANELS_NET_INSPECTOR_UNAVAILABLE_TITLE),
            t!(i18n, ids::PANELS_NET_INSPECTOR_UNAVAILABLE),
            vec![EmptyStateAction {
                label: t!(i18n, ids::PANELS_NET_INSPECTOR_OPEN_MESSAGES),
                on_press: msg(NetInspectorMsg::OpenMessagesPressed),
                primary: false,
            }],
        ),
        NetListStatus::Ready => ready(ctx, view),
    }
}

/// The tab strip, the filter, the list of the tab and the footer.
fn ready<'a>(ctx: &'a PanelContext, view: &'a NetInspectorView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let (list, shown, summary) = match view.tab {
        NetInspectorTab::ByNet => (
            by_net::list(ctx, view),
            view.nets.len(),
            t!(
                i18n,
                ids::PANELS_NET_INSPECTOR_SUMMARY,
                "shown" => view.nets.len(),
                "total" => view.total
            ),
        ),
        NetInspectorTab::ByComponent => (
            by_component::list(ctx, view),
            view.components.len(),
            t!(
                i18n,
                ids::PANELS_NET_INSPECTOR_PARTS_SUMMARY,
                "shown" => view.components.len(),
                "total" => view.component_total
            ),
        ),
    };
    let placeholder = match view.tab {
        NetInspectorTab::ByNet => ids::PANELS_NET_INSPECTOR_FILTER,
        NetInspectorTab::ByComponent => ids::PANELS_NET_INSPECTOR_FILTER_PARTS,
    };
    // The "Click a row" hint fits the net list, where every row leads somewhere.
    let hint = (view.tab == NetInspectorTab::ByNet && shown > 0)
        .then(|| t!(i18n, ids::PANELS_NET_INSPECTOR_FOOTER_HINT));
    column![
        container(tab_strip(ctx, view))
            .padding(iced::Padding {
                top: SP.m,
                right: SP.l,
                bottom: 0.0,
                left: SP.l,
            })
            .width(Length::Fill),
        filter_field(ctx, placeholder, &view.filter),
        list,
        rule(ctx),
        footer(ctx, summary, hint),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// By Net / By Component, each with the count of its list.
fn tab_strip<'a>(ctx: &'a PanelContext, view: &'a NetInspectorView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let style = r.segmented(TS.body).fill_width(true);
    let by_net: signex_widgets::Label<'a> =
        t!(i18n, ids::PANELS_NET_INSPECTOR_BY_NET, "count" => view.total).into();
    let by_component: signex_widgets::Label<'a> =
        t!(i18n, ids::PANELS_NET_INSPECTOR_BY_COMPONENT, "count" => view.component_total).into();
    segmented(
        [
            (NetInspectorTab::ByNet, by_net),
            (NetInspectorTab::ByComponent, by_component),
        ],
        view.tab,
        style,
        |tab| msg(NetInspectorMsg::TabSelected(tab)),
    )
}

/// The filter box; its placeholder names what the list holds.
pub(super) fn filter_field<'a>(
    ctx: &'a PanelContext,
    placeholder: signex_i18n::MessageId,
    filter: &'a str,
) -> Element<'a, PanelMsg> {
    container(
        text_input(t!(ctx.i18n, placeholder), filter)
            .on_input(|s| msg(NetInspectorMsg::FilterChanged(s)))
            .style(super::form_look::text_input_style)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .width(Length::Fill),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// The summary, pinned under the list, with an optional hint on the right.
pub(super) fn footer<'a>(
    ctx: &'a PanelContext,
    summary: Localized,
    hint: Option<Localized>,
) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let mut line = row![
        text(summary)
            .size(TS.caption)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None),
        Space::new().width(Length::Fill).height(Length::Shrink),
    ]
    .spacing(SP.s);
    if let Some(hint) = hint {
        line = line.push(
            text(hint)
                .size(TS.caption)
                .color(muted)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    }
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A one-pixel soft rule across the panel.
pub(super) fn rule<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let color = resolve(&ctx.tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(crate::styles::HAIRLINE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// The list in a thin-scrollbar scrollable filling the space left.
pub(super) fn scrolled<'a>(list: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

/// The no-match card under the filter, with Clear filter; or, for an empty
/// project with no filter, only the title.
pub(super) fn no_match<'a>(
    ctx: &'a PanelContext,
    title: signex_i18n::MessageId,
    body: signex_i18n::MessageId,
    total: usize,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let filter = ctx.net_inspector.filter.trim();
    let (body, actions) = if filter.is_empty() {
        (t!(i18n, ids::PANELS_NET_INSPECTOR_EMPTY_NONE), Vec::new())
    } else {
        (
            t!(i18n, body, "filter" => filter.to_string(), "total" => total),
            vec![EmptyStateAction {
                label: t!(i18n, ids::PANELS_NET_INSPECTOR_CLEAR_FILTER),
                on_press: msg(NetInspectorMsg::FilterCleared),
                primary: false,
            }],
        )
    };
    container(empty_state_card_with_actions(
        &ctx.tokens,
        t!(i18n, title),
        body,
        None,
        actions,
    ))
    .padding([0.0, SP.l])
    .width(Length::Fill)
    .boxed()
}
