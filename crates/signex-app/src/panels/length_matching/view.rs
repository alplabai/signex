//! The two views of Length Matching (mockup row 107). The tab: a toolbar
//! with the live status, the filter and Show as panel, a New group row,
//! then every group with its members' bars against the band, and under the
//! group picked for editing its fields (name, target, tolerance, topology,
//! Add net, Remove, Delete group). The panel: Open as tab, the filter and
//! the same groups, each folding. Both are pure functions of the panel
//! context.

use iced::widget::{Column, Space, column, container, pick_list, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::pcb_length::LengthTopology;
use signex_widgets::compact_group::{
    CompactGroup, CompactGroupStyle, GroupHeader, Member, StatusPill, StatusTone,
};
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle, Footer, Toolbar};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, LengthFormat, PickListPlaceholder as _, button, text, text_input};
use uuid::Uuid;

use super::model::topology_message;
use super::{
    LengthField, LengthFilter, LengthGroupRow, LengthMatchingFacts, LengthMatchingMsg,
    LengthMemberRow, msg,
};
use crate::panels::paired::PairedHeader;
use crate::panels::{PanelContext, PanelMsg, empty_state_card, form_look};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};
use signex_engine::pcb::length_match::MemberStatus;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of a typed length, in em of the body text.
const FIELD_EM: f32 = 6.0;

/// The tab's content, or its empty state without a board.
pub fn tab_view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let Some(facts) = ctx.length_matching.as_ref() else {
        return empty_state_card(
            &ctx.tokens,
            t!(ctx.i18n, ids::LENGTH_MATCHING_NO_BOARD_TITLE),
            t!(ctx.i18n, ids::LENGTH_MATCHING_NO_BOARD),
            None,
        );
    };
    let mut body: Column<Element<'_, PanelMsg>> = column![]
        .spacing(SP.s)
        .padding([SP.s, SP.l])
        .width(Length::Fill);
    if !facts.read_only {
        body = body.push(new_group_row(ctx));
    }
    body = body.push(note(ctx, ids::LENGTH_MATCHING_MEASURED));
    if facts.groups.is_empty() {
        body = body.push(empty(ctx));
    } else {
        let ui = &ctx.length_matching_ui;
        let picked = ui.is_for(&facts.board) && ui.current.is_some();
        if !picked && !facts.read_only {
            body = body.push(note(ctx, ids::LENGTH_MATCHING_PICK_GROUP));
        }
        body = body.push(groups(ctx, facts, true));
    }
    let shell_style = DocTabShellStyle::from_tokens(&ctx.tokens, TS.body);
    DocTabShell::new(scrollable(body).height(Length::Fill).boxed(), shell_style)
        .toolbar(toolbar(ctx, facts))
        .footer(
            Footer::new(t!(ctx.i18n, ids::LENGTH_MATCHING_FOOTER))
                .note(t!(ctx.i18n, ids::LENGTH_MATCHING_FOOTER_NOTE)),
        )
        .view()
}

/// The compact dock panel: without a board its empty state alone, else
/// Open as tab over the filter and the groups.
pub fn panel_view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let Some(facts) = ctx.length_matching.as_ref() else {
        return container(empty(ctx))
            .padding(SP.m)
            .width(Length::Fill)
            .boxed();
    };
    let header = PairedHeader {
        title: ids::PANELS_PAIRED_LENGTH_MATCHING,
        open_tab: ids::PANELS_PAIRED_OPEN_AS_TAB,
        on_open_tab: msg(LengthMatchingMsg::TabOpened),
    }
    .view(&ctx.tokens, &ctx.i18n);
    if facts.groups.is_empty() {
        let card = container(empty(ctx)).padding(SP.m).width(Length::Fill);
        return column![header, card].width(Length::Fill).boxed();
    }
    let list = scrollable(groups(ctx, facts, false))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(form_look::scrollbar_style);
    let filter = container(filter_list(ctx)).padding([SP.s, SP.m]);
    column![header, filter, list]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn empty<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    empty_state_card(
        &ctx.tokens,
        t!(ctx.i18n, ids::PANELS_LENGTH_MATCHING_EMPTY_TITLE),
        t!(ctx.i18n, ids::PANELS_LENGTH_MATCHING_EMPTY_BODY),
        None,
    )
}

/// The live status, the filter and Show as panel.
fn toolbar<'a>(ctx: &'a PanelContext, facts: &'a LengthMatchingFacts) -> Toolbar<'a, PanelMsg> {
    let summary = t!(
        ctx.i18n,
        ids::LENGTH_MATCHING_SUMMARY,
        "groups" => facts.groups.len(),
        "members" => facts.member_count()
    );
    let out = facts.out_of_band();
    let band = if out > 0 {
        t!(ctx.i18n, ids::LENGTH_MATCHING_OUT_COUNT, "count" => out)
    } else {
        t!(ctx.i18n, ids::LENGTH_MATCHING_ALL_IN_BAND)
    };
    Toolbar::new()
        .push(filter_list(ctx))
        .separator()
        .push(action(
            ctx,
            ids::LENGTH_MATCHING_SHOW_AS_PANEL,
            Some(msg(LengthMatchingMsg::PanelShown)),
        ))
        .push_end(
            row![caption(ctx, summary.into()), caption(ctx, band.into())]
                .spacing(SP.m)
                .boxed(),
        )
}

/// One entry of a list: its value and what it reads.
#[derive(Debug, Clone, PartialEq)]
struct Choice<T> {
    value: T,
    label: String,
}

/// All, or Out of band.
fn filter_list<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let options: Vec<Choice<LengthFilter>> = LengthFilter::ALL
        .into_iter()
        .map(|value| Choice {
            value,
            label: t!(ctx.i18n, value.message()).into_string(),
        })
        .collect();
    let current = ctx.length_matching_ui.filter;
    let picked = options.iter().find(|c| c.value == current).cloned();
    let list = pick_list(picked, options, |c: &Choice<LengthFilter>| c.label.clone())
        .graphite()
        .text_size(TS.dense)
        .padding([SP.xs, SP.s])
        .on_select(|c: Choice<LengthFilter>| msg(LengthMatchingMsg::FilterPicked(c.value)));
    labelled(ctx, ids::LENGTH_MATCHING_FILTER, list.boxed())
}

/// The groups the filter lets through, open in the tab, folding in the
/// panel; the tab puts the picked group's fields under it.
fn groups<'a>(
    ctx: &'a PanelContext,
    facts: &'a LengthMatchingFacts,
    is_tab: bool,
) -> Element<'a, PanelMsg> {
    let ui = &ctx.length_matching_ui;
    let ours = ui.is_for(&facts.board);
    let filter = if ours { ui.filter } else { LengthFilter::All };
    let shown: Vec<&LengthGroupRow> = facts.groups.iter().filter(|g| filter.shows(g)).collect();
    if shown.is_empty() {
        return note(ctx, ids::LENGTH_MATCHING_NO_MATCH);
    }
    let style = CompactGroupStyle::from_tokens(&ctx.tokens, TS.body);
    let format = LengthFormat::new(&ctx.i18n, Unit::Mm);
    let mut list: Column<Element<'a, PanelMsg>> = column![].width(Length::Fill);
    for group in shown {
        let is_open = is_tab || !(ours && ui.closed.contains(&group.uuid));
        let is_current = ours && ui.current == Some(group.uuid);
        let selected = ui.selected.as_deref().filter(|_| is_current);
        let uuid = group.uuid;
        let on_toggle = move |g: Uuid| {
            if is_tab {
                msg(LengthMatchingMsg::GroupPicked(g))
            } else {
                msg(LengthMatchingMsg::GroupToggled(g))
            }
        };
        let on_select =
            move |net: String| msg(LengthMatchingMsg::MemberClicked { group: uuid, net });
        let members = group
            .members
            .iter()
            .map(|m| member(&ctx.i18n, group, m, selected))
            .collect();
        let view = CompactGroup::new(
            uuid,
            header(&ctx.i18n, group),
            members,
            is_open,
            on_toggle,
            on_select,
        )
        .view(&format, style);
        list = list.push(view);
        if is_tab && is_current && !facts.read_only {
            list = list.push(editor(ctx, facts, group));
        }
    }
    list.boxed()
}

/// A group's header: name, member count, band, topology and rule, and the
/// out-of-band pill.
fn header<'a>(i18n: &I18n, group: &LengthGroupRow) -> GroupHeader<'a> {
    let format = LengthFormat::new(i18n, Unit::Mm);
    let band = t!(
        i18n,
        ids::LENGTH_MATCHING_BAND,
        "target" => format.number_rounded(group.target_mm, 2),
        "tolerance" => format.number_rounded(group.tolerance_mm, 2)
    );
    let out = group.out_of_band();
    let status = if out > 0 {
        StatusPill {
            label: t!(i18n, ids::LENGTH_MATCHING_OUT_COUNT, "count" => out).into(),
            tone: StatusTone::Err,
        }
    } else {
        StatusPill {
            label: t!(i18n, ids::LENGTH_MATCHING_ALL_IN_BAND).into(),
            tone: StatusTone::Ok,
        }
    };
    GroupHeader {
        name: UserText::new(group.name.clone()).into(),
        captions: vec![
            t!(i18n, ids::LENGTH_MATCHING_MEMBERS, "count" => group.members.len()).into(),
            band.into(),
            t!(i18n, topology_message(group.topology)).into(),
            t!(i18n, ids::LENGTH_MATCHING_RULE, "rule" => group.rule.clone()).into(),
        ],
        status: Some(status),
    }
}

/// A member row: its net, the pair or the status that has no bar, and the
/// bar against the band. A member with no length draws at zero.
fn member<'a>(
    i18n: &I18n,
    group: &LengthGroupRow,
    row: &LengthMemberRow,
    selected: Option<&str>,
) -> Member<'a, String> {
    let status = match row.status {
        MemberStatus::Unrouted => Some(ids::LENGTH_MATCHING_STATUS_UNROUTED),
        MemberStatus::Missing => Some(ids::LENGTH_MATCHING_STATUS_MISSING),
        MemberStatus::Over => Some(ids::LENGTH_MATCHING_STATUS_OVER),
        MemberStatus::Short => Some(ids::LENGTH_MATCHING_STATUS_SHORT),
        MemberStatus::InBand => None,
    };
    let caption: Option<Label<'a>> = match (status, &row.pair) {
        (Some(id), _) => Some(t!(i18n, id).into()),
        (None, Some(pair)) => {
            Some(t!(i18n, ids::LENGTH_MATCHING_PAIR, "pair" => pair.clone()).into())
        }
        (None, None) => None,
    };
    Member {
        key: row.net.clone(),
        name: UserText::new(row.net.clone()).into(),
        caption,
        length: row.length_mm.unwrap_or(0.0),
        target: group.target_mm,
        tolerance: group.tolerance_mm,
        is_selected: selected == Some(row.net.as_str()),
    }
}

/// The New group row: name, target, tolerance and Add Group.
fn new_group_row<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let ui = &ctx.length_matching_ui;
    let filled = [
        LengthField::NewName,
        LengthField::NewTarget,
        LengthField::NewTolerance,
    ]
    .iter()
    .all(|field| ui.drafts.get(field).is_some_and(|t| !t.trim().is_empty()));
    row![
        caption(ctx, t!(ctx.i18n, ids::LENGTH_MATCHING_NEW_GROUP).into()),
        field(
            ctx,
            ids::LENGTH_MATCHING_NAME,
            LengthField::NewName,
            String::new()
        ),
        field(
            ctx,
            ids::LENGTH_MATCHING_TARGET,
            LengthField::NewTarget,
            String::new()
        ),
        field(
            ctx,
            ids::LENGTH_MATCHING_TOLERANCE,
            LengthField::NewTolerance,
            String::new()
        ),
        action(
            ctx,
            ids::LENGTH_MATCHING_ADD_GROUP,
            filled.then(|| msg(LengthMatchingMsg::GroupAdded)),
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The picked group's fields under its rows.
fn editor<'a>(
    ctx: &'a PanelContext,
    facts: &'a LengthMatchingFacts,
    group: &'a LengthGroupRow,
) -> Element<'a, PanelMsg> {
    let ui = &ctx.length_matching_ui;
    let format = LengthFormat::new(&ctx.i18n, Unit::Mm);
    let uuid = group.uuid;
    let fields = row![
        field(
            ctx,
            ids::LENGTH_MATCHING_NAME,
            LengthField::Name,
            group.name.clone()
        ),
        field(
            ctx,
            ids::LENGTH_MATCHING_TARGET,
            LengthField::Target,
            format.number(group.target_mm),
        ),
        field(
            ctx,
            ids::LENGTH_MATCHING_TOLERANCE,
            LengthField::Tolerance,
            format.number(group.tolerance_mm),
        ),
        topology_list(ctx, uuid, group.topology),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let outside: Vec<String> = facts
        .nets
        .iter()
        .filter(|net| !group.members.iter().any(|m| m.net == **net))
        .cloned()
        .collect();
    let add = pick_list(None::<String>, outside, |net: &String| net.clone())
        .graphite()
        .text_size(TS.dense)
        .padding([SP.xs, SP.s])
        .placeholder_label(t!(ctx.i18n, ids::LENGTH_MATCHING_ADD_NET))
        .on_select(move |net: String| msg(LengthMatchingMsg::NetAdded { group: uuid, net }));
    let remove = ui.selected.clone().map(|net| {
        let label = t!(ctx.i18n, ids::LENGTH_MATCHING_REMOVE_NET, "net" => net.clone());
        ghost(
            label.into(),
            Some(msg(LengthMatchingMsg::MemberRemoved { group: uuid, net })),
        )
    });
    let mut actions = row![add.boxed()].spacing(SP.m).align_y(Alignment::Center);
    if let Some(remove) = remove {
        actions = actions.push(remove);
    }
    actions = actions
        .push(Space::new().width(Length::Fill).boxed())
        .push(action(
            ctx,
            ids::LENGTH_MATCHING_DELETE_GROUP,
            Some(msg(LengthMatchingMsg::GroupDeleted(uuid))),
        ));
    let title = t!(ctx.i18n, ids::LENGTH_MATCHING_EDIT, "group" => group.name.clone());
    container(
        column![caption(ctx, title.into()), fields, actions]
            .spacing(SP.s)
            .width(Length::Fill),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// Star, Chain, Fly-by or T.
fn topology_list<'a>(
    ctx: &PanelContext,
    group: Uuid,
    current: LengthTopology,
) -> Element<'a, PanelMsg> {
    let options: Vec<Choice<LengthTopology>> = LengthTopology::ALL
        .into_iter()
        .map(|value| Choice {
            value,
            label: t!(ctx.i18n, topology_message(value)).into_string(),
        })
        .collect();
    let picked = options.iter().find(|c| c.value == current).cloned();
    let list = pick_list(picked, options, |c: &Choice<LengthTopology>| {
        c.label.clone()
    })
    .graphite()
    .text_size(TS.dense)
    .padding([SP.xs, SP.s])
    .on_select(move |c: Choice<LengthTopology>| {
        msg(LengthMatchingMsg::TopologyPicked {
            group,
            topology: c.value,
        })
    });
    labelled(ctx, ids::LENGTH_MATCHING_TOPOLOGY, list.boxed())
}

/// A typed field with its label; Enter applies it.
fn field<'a>(
    ctx: &PanelContext,
    label: MessageId,
    which: LengthField,
    stored: String,
) -> Element<'a, PanelMsg> {
    let shown = ctx.length_matching_ui.shown(which, || stored);
    let input = text_input(UserText::new(String::new()), shown)
        .size(TS.dense)
        .padding(SP.xs)
        .style(form_look::text_input_style)
        .width(Length::Fixed(TS.body * FIELD_EM))
        .on_input(move |text| msg(LengthMatchingMsg::FieldTyped { field: which, text }))
        .on_submit(msg(LengthMatchingMsg::FieldSubmitted(which)));
    labelled(ctx, label, input.boxed())
}

fn labelled<'a>(
    ctx: &PanelContext,
    label: MessageId,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        text(t!(ctx.i18n, label))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
        value,
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// A ghost button labelled from the catalog; inert without a message.
fn action<'a>(
    ctx: &PanelContext,
    label: MessageId,
    on_press: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    ghost(t!(ctx.i18n, label).into(), on_press)
}

fn ghost<'a>(label: Label<'a>, on_press: Option<PanelMsg>) -> Element<'a, PanelMsg> {
    button(text(label).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press_maybe(on_press)
        .boxed()
}

fn caption<'a>(ctx: &PanelContext, label: Label<'a>) -> Element<'a, PanelMsg> {
    text(label)
        .size(TS.dense)
        .color(ti(ctx.tokens.text))
        .boxed()
}

/// A muted line of the page.
fn note<'a>(ctx: &PanelContext, id: MessageId) -> Element<'a, PanelMsg> {
    text(t!(ctx.i18n, id))
        .size(TS.dense)
        .color(resolve(&ctx.tokens).ink3)
        .boxed()
}
