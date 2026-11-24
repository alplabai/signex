//! The DRC panel's rule groups and violation rows (#88): a group heading
//! "HV_08 (3)" with its Rule link (none on Short and Unconnected), then
//! two-line rows (the detail; the
//! layer, position and object count) whose Show / Waive / Unwaive show on
//! hover or selection, and on the row whose waiver form is open.

use signex_i18n::{ids, t};
use signex_types::violation::Severity;

use super::drc_results::{DrcPanelMsg, drc_msg, rule_label};
use super::validation_kit::{
    FindingRow, GroupHeader, finding_row, group_header, link, link_ink, row_action, severity_badge,
    show_action,
};
use super::waiver_form::{ReasonFormEvents, ReasonFormView, reason_form};
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

/// Whether a group of `kind` has a rule to open. Shorts and unconnected
/// items are always checked and no rule sets them, so their groups have
/// no Rule link (mockup row 88).
pub(super) fn has_rule_link(kind: signex_drc::ViolationKind) -> bool {
    !matches!(
        kind,
        signex_drc::ViolationKind::Short | signex_drc::ViolationKind::Unconnected
    )
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
    let rule_link = has_rule_link(first.kind).then(|| {
        link(
            t!(i18n, ids::DRC_PANEL_RULE_LINK),
            drc_msg(DrcPanelMsg::RuleOpened(first.rule.clone())),
            link_ink(tokens),
        )
    });
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).push(group_header(
            GroupHeader {
                open,
                badge: Some(severity_badge_of(ctx, first.severity, false)),
                title: title.into(),
                title_ink: theme_ext::text_primary(tokens),
                count: None,
                trailing: rule_link,
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

/// The row is the selected one, or its waiver form is open (mockup row 88:
/// the open row stays tinted and keeps Show and Waive above the form).
fn is_open_row(drc: &DrcPanel, key: &str) -> bool {
    drc.view.selected.as_deref() == Some(key)
        || drc.waive_draft.as_ref().is_some_and(|d| d.key == key)
}

/// One violation row with its actions while hovered, selected or waived.
fn violation<'a>(
    ctx: &'a PanelContext,
    drc: &'a DrcPanel,
    item: &'a DrcRow,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let selected = is_open_row(drc, &item.key);
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
    let mut actions = vec![show_action(
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

/// The Reason form under the row being waived, if it is this one. Waive
/// waits for the git identity, which names the waiver's author.
fn waive_form<'a>(
    ctx: &'a PanelContext,
    drc: &'a DrcPanel,
    item: &DrcRow,
) -> Option<Element<'a, PanelMsg>> {
    let draft = drc.waive_draft.as_ref().filter(|d| d.key == item.key)?;
    let error = draft
        .blank
        .then(|| t!(ctx.i18n, ids::PANELS_WAIVER_NEEDS_REASON).into());
    let form = reason_form(
        ReasonFormView {
            reason: &draft.reason,
            placeholder: t!(ctx.i18n, ids::DRC_PANEL_WAIVE_PLACEHOLDER),
            error,
        },
        ReasonFormEvents {
            reason: |action| drc_msg(DrcPanelMsg::ReasonActed(action)),
            confirm: (!draft.awaits_identity).then(|| drc_msg(DrcPanelMsg::WaiveConfirmed)),
            cancel: drc_msg(DrcPanelMsg::WaiveCancelled),
        },
        &ctx.i18n,
        &ctx.tokens,
    );
    Some(container(form).padding([SP.xs, SP.m]).boxed())
}
