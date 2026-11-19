//! The DRC panel's rule groups and violation rows (#88): a group heading
//! "HV_08 (3)" with its Rule link, then two-line rows (the detail; the
//! layer, position and object count) whose Show / Waive / Unwaive show on
//! hover or selection. Waive opens the waiver form ERC uses too (AV-13).

use signex_i18n::{ids, t};
use signex_types::violation::Severity;

use super::drc_results::{DrcPanelMsg, drc_msg, rule_label};
use super::validation_kit::{
    FindingRow, GroupHeader, finding_row, group_header, link, link_ink, row_action, severity_badge,
};
use super::waiver_form::{WaiverFormEvents, WaiverFormView, waiver_form};
use super::*;
use crate::styles::{PillTone, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn severity_badge_of<'a>(
    ctx: &'a PanelContext,
    severity: Option<Severity>,
    selected: bool,
) -> Element<'a, PanelMsg> {
    let (letter, tone) = match severity {
        Some(Severity::Error) => (ids::PANELS_ERC_SEVERITY_ERROR, PillTone::Err),
        Some(Severity::Warning) => (ids::PANELS_ERC_SEVERITY_WARNING, PillTone::Warn),
        Some(Severity::Info) => (ids::PANELS_ERC_SEVERITY_INFO, PillTone::Info),
        None => {
            return severity_badge(raw("–"), PillTone::Neutral, selected, &ctx.tokens);
        }
    };
    severity_badge(t!(ctx.i18n, letter), tone, selected, &ctx.tokens)
}

/// One rule's group: its heading, then its rows while open.
pub(super) fn group<'a>(
    ctx: &'a PanelContext,
    drc: &'a DrcPanel,
    members: Vec<&'a DrcRow>,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let first = members[0];
    let open = !drc.view.collapsed.contains(&first.rule);
    let title = t!(
        i18n, ids::DRC_PANEL_GROUP,
        "rule" => rule_label(i18n, first), "count" => members.len()
    );
    let rule_link = link(
        t!(i18n, ids::DRC_PANEL_RULE_LINK),
        drc_msg(DrcPanelMsg::RuleOpened(first.rule.clone())),
        link_ink(tokens),
    );
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).push(group_header(
            GroupHeader {
                open,
                badge: Some(severity_badge_of(ctx, first.severity, false)),
                title: title.into(),
                title_ink: theme_ext::text_primary(tokens),
                count: None,
                trailing: Some(rule_link),
                on_toggle: drc_msg(DrcPanelMsg::GroupToggled(first.rule.clone())),
            },
            i18n,
            tokens,
        ));
    if !open {
        return col.boxed();
    }
    for item in members {
        col = col.push(violation(ctx, drc, item));
        if let Some(form) = waive_form(ctx, drc, item) {
            col = col.push(form);
        }
    }
    col.boxed()
}

/// The second line of a row: layer, position and object count.
fn place<'a>(ctx: &'a PanelContext, item: &DrcRow, selected: bool) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let (_, detail) = super::erc_panel::erc_row_text(&ctx.tokens, selected);
    let (_, _, at) = super::drc::row_text(i18n, item);
    let objects = t!(i18n, ids::DRC_PANEL_OBJECTS, "count" => item.objects).into_string();
    let words = match super::drc::layer_label(i18n, item.layer) {
        Some(layer) => t!(
            i18n, ids::DRC_PANEL_ROW_PLACE,
            "layer" => layer.into_string(), "place" => at.into_string(), "objects" => objects
        ),
        None => t!(
            i18n, ids::DRC_PANEL_ROW_PLACE_NO_LAYER,
            "place" => at.into_string(), "objects" => objects
        ),
    };
    text(words).size(TS.caption).color(detail).boxed()
}

/// One violation row with its actions while hovered or selected.
fn violation<'a>(
    ctx: &'a PanelContext,
    drc: &'a DrcPanel,
    item: &'a DrcRow,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let selected = drc.view.selected.as_deref() == Some(item.key.as_str());
    let hovered = drc.view.hovered.as_deref() == Some(item.key.as_str());
    let (_, detail_ink) = super::erc_panel::erc_row_text(tokens, selected);
    let (_, detail, _) = super::drc::row_text(i18n, item);
    let mut extra = Vec::new();
    if let Some(note) = &item.waiver {
        let said = if note.is_empty() {
            t!(i18n, ids::DRC_PANEL_WAIVED_NO_NOTE)
        } else {
            t!(i18n, ids::DRC_PANEL_WAIVED_NOTE, "note" => note.clone())
        };
        extra.push(text(said).size(TS.caption).color(detail_ink).boxed());
    }
    let mut actions = vec![row_action(
        t!(i18n, ids::DRC_PANEL_SHOW),
        drc_msg(DrcPanelMsg::RowPicked(item.key.clone())),
        tokens,
    )];
    actions.push(match item.waiver {
        Some(_) => row_action(
            t!(i18n, ids::DRC_PANEL_UNWAIVE),
            drc_msg(DrcPanelMsg::Unwaived(item.key.clone())),
            tokens,
        ),
        None => row_action(
            t!(i18n, ids::DRC_PANEL_WAIVE),
            drc_msg(DrcPanelMsg::WaiveStarted(item.key.clone())),
            tokens,
        ),
    });
    finding_row(
        FindingRow {
            badge: severity_badge_of(ctx, item.severity, selected),
            message: detail.into(),
            place: place(ctx, item, selected),
            extra,
            actions,
            selected,
            hovered,
            on_press: Some(drc_msg(DrcPanelMsg::RowPicked(item.key.clone()))),
            on_enter: Some(drc_msg(DrcPanelMsg::RowEntered(item.key.clone()))),
            on_exit: Some(drc_msg(DrcPanelMsg::RowLeft(item.key.clone()))),
        },
        tokens,
    )
}

/// The shared waiver form under the row being waived, if it is this one.
fn waive_form<'a>(
    ctx: &'a PanelContext,
    drc: &'a DrcPanel,
    item: &DrcRow,
) -> Option<Element<'a, PanelMsg>> {
    let draft = drc.waive_draft.as_ref().filter(|d| d.key == item.key)?;
    let error = draft
        .blank
        .then(|| t!(ctx.i18n, ids::PANELS_WAIVER_NEEDS_BOTH).into());
    let form = waiver_form(
        WaiverFormView {
            reason: &draft.reason,
            author: &draft.author,
            error,
        },
        WaiverFormEvents {
            reason: |action| drc_msg(DrcPanelMsg::ReasonActed(action)),
            author: |author| drc_msg(DrcPanelMsg::AuthorEdited(author)),
            confirm: drc_msg(DrcPanelMsg::WaiveConfirmed),
            cancel: drc_msg(DrcPanelMsg::WaiveCancelled),
        },
        &ctx.i18n,
        &ctx.tokens,
    );
    Some(container(form).padding([SP.xs, SP.m]).boxed())
}
