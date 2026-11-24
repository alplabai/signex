//! The ERC panel's rule groups and finding rows, and the Waived group at
//! the end (#85). Row actions ("Show", "Add No-Connect", "Waive net",
//! "Waive") show on hover or selection only; "Show" is the one label for
//! selecting and framing a finding in ERC and DRC (AV-13).

use signex_i18n::{ids, t};

use super::erc_panel::{ErcGroup, ErcPanelMsg, erc_row_text};
use super::erc_waivers::{ErcWaiverStatus, violation_waiver_row, waive_action, waive_editor};
use super::validation_kit::{
    FindingRow, GroupHeader, finding_row, group_header, row_action, severity_badge, status_pill,
};
use super::*;
use crate::styles::{PillTone, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn erc_msg(msg: ErcPanelMsg) -> PanelMsg {
    PanelMsg::ErcView(msg)
}

/// A rule group: its heading, then its rows while open.
pub(super) fn group<'a>(ctx: &'a PanelContext, group: ErcGroup<'a>) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let open = !ctx.validation.erc.collapsed.contains(group.rule);
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).push(group_header(
            GroupHeader {
                open,
                badge: Some(severity_badge(
                    t!(i18n, group.severity.letter()),
                    group.severity.pill(),
                    false,
                    tokens,
                )),
                // raw: the rule name is worded by signex-erc (#2062).
                title: raw(group.rule),
                title_ink: theme_ext::text_primary(tokens),
                count: Some(group.rows.len()),
                trailing: None,
                on_toggle: erc_msg(ErcPanelMsg::GroupToggled(group.rule.to_string())),
            },
            i18n,
            tokens,
        ));
    if !open {
        return col.boxed();
    }
    for entry in group.rows {
        col = col.push(finding(ctx, entry));
        if let Some(editor) = waive_editor(entry, ctx) {
            col = col.push(editor);
        }
    }
    col.boxed()
}

/// Where a finding is, on one line: the sheet's file name and the point.
fn place_text(i18n: &signex_i18n::I18n, v: &ErcDiagnosticEntry) -> signex_i18n::Localized {
    t!(
        i18n, ids::PANELS_ERC_FINDING_PLACE,
        "sheet" => v.sheet_name.as_str(),
        "x" => format!("{:.2}", v.world_x),
        "y" => format!("{:.2}", v.shown_y)
    )
}

/// One finding: its message, sheet and point (the full path in a tooltip),
/// and its actions while hovered or selected.
fn finding<'a>(ctx: &'a PanelContext, v: &'a ErcDiagnosticEntry) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let selected = ctx.erc_focus_index == Some(v.global_index);
    let hovered = ctx.validation.erc.hovered == Some(v.global_index);
    let (_, detail) = erc_row_text(tokens, selected);
    let place = signex_widgets::tooltip(
        text(place_text(i18n, v))
            .size(TS.caption)
            .color(detail)
            .wrapping(iced::widget::text::Wrapping::None),
        container(
            text(signex_i18n::UserText::new(
                v.sheet_path.display().to_string(),
            ))
            .size(TS.caption),
        )
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens)),
        iced::widget::tooltip::Position::Top,
    )
    .gap(SP.xs)
    .boxed();
    let mut actions = Vec::new();
    if v.rule_kind == Some(signex_erc::RuleKind::UnusedPin) {
        actions.push(row_action(
            t!(i18n, ids::PANELS_ERC_ADD_NO_CONNECT),
            PanelMsg::ErcQuickFix(v.global_index),
            tokens,
        ));
    }
    actions.push(row_action(
        t!(i18n, ids::PANELS_ERC_SHOW),
        PanelMsg::FocusErcViolation(v.global_index),
        tokens,
    ));
    if let Some(net) = v.net.as_ref().filter(|n| !n.name.is_empty()) {
        actions.push(row_action(
            t!(i18n, ids::PANELS_ERC_WAIVE_NET),
            PanelMsg::ErcNetWaiverSet {
                rule_id: v.rule_id.clone(),
                net: net.name.clone(),
                waive: true,
            },
            tokens,
        ));
    }
    if let Some(waive) = waive_action(v, tokens, i18n) {
        actions.push(waive);
    }
    finding_row(
        FindingRow {
            badge: severity_badge(
                t!(i18n, v.severity.letter()),
                v.severity.pill(),
                selected,
                tokens,
            ),
            // raw: the message is worded by signex-erc (#2062).
            message: raw(v.message.as_str()),
            place,
            extra: Vec::new(),
            actions,
            selected,
            hovered,
            on_press: Some(PanelMsg::FocusErcViolation(v.global_index)),
            on_enter: Some(erc_msg(ErcPanelMsg::RowEntered(v.global_index))),
            on_exit: Some(erc_msg(ErcPanelMsg::RowLeft(v.global_index))),
        },
        tokens,
    )
}

/// A stored net waiver in the Waived group: rule and net, its status pill
/// and Restore.
fn net_waiver_row<'a>(ctx: &'a PanelContext, w: &'a ErcWaiverEntry) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (status, tone) = ErcWaiverStatus::of_net(w.hidden).pill(i18n);
    finding_row(
        FindingRow {
            badge: severity_badge(raw("–"), PillTone::Neutral, false, tokens),
            message: t!(
                i18n, ids::PANELS_ERC_NET_WAIVER_TITLE,
                "rule" => w.rule_id.as_str(), "net" => w.net.as_str()
            )
            .into(),
            place: status_pill(status, tone, tokens),
            extra: vec![row_action(
                t!(i18n, ids::PANELS_ERC_RESTORE),
                PanelMsg::ErcNetWaiverSet {
                    rule_id: w.rule_id.clone(),
                    net: w.net.clone(),
                    waive: false,
                },
                tokens,
            )],
            actions: Vec::new(),
            selected: false,
            hovered: true,
            on_press: None,
            on_enter: None,
            on_exit: None,
        },
        tokens,
    )
}

/// The Waived group at the end: every stored waiver, net and per-finding,
/// kept even when every finding is waived or the run was clean, so a
/// waiver is never invisible.
pub(super) fn waived_group<'a>(ctx: &'a PanelContext, has_findings: bool) -> Element<'a, PanelMsg> {
    let count = ctx.erc_waivers.len() + ctx.erc_violation_waivers.len();
    if count == 0 {
        return Space::new().width(0).height(0).boxed();
    }
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let open = ctx.validation.erc.is_waived_open(has_findings);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .width(Length::Fill)
        .padding(iced::Padding {
            top: SP.m,
            ..iced::Padding::ZERO
        })
        .push(group_header(
            GroupHeader {
                open,
                badge: None,
                title: t!(i18n, ids::PANELS_ERC_WAIVED_GROUP).into(),
                title_ink: crate::styles::tokens::resolve(tokens).ink3,
                count: Some(count),
                trailing: None,
                on_toggle: erc_msg(ErcPanelMsg::WaivedToggled),
            },
            i18n,
            tokens,
        ));
    if !open {
        return col.boxed();
    }
    for w in &ctx.erc_waivers {
        col = col.push(net_waiver_row(ctx, w));
    }
    for w in &ctx.erc_violation_waivers {
        col = col.push(violation_waiver_row(ctx, w));
    }
    col.boxed()
}

#[cfg(test)]
mod tests {
    use super::place_text;

    /// A finding's place is the sheet's file name and the point to two
    /// decimals, on one line: no folder, whatever folder the sheet is in.
    #[test]
    fn a_finding_location_is_the_file_name_and_the_point_on_one_line() {
        let (mut app, _boot) = crate::app::Signex::new();
        let path = std::path::PathBuf::from("w").join("sub").join("top.snxsch");
        let finding = signex_erc::Violation::new(
            signex_erc::RuleId::builtin(signex_erc::RuleKind::UnusedPin),
            signex_erc::RuleKind::UnusedPin,
            signex_erc::Severity::Warning,
            "finding",
            signex_types::schematic::Point::new(12.346, -6.0),
        );
        app.ui_state
            .erc_violations_by_path
            .insert(path, vec![finding]);
        let entries = app.build_erc_diagnostic_entries();
        let i18n = signex_i18n::I18n::en_us();
        let label = place_text(&i18n, &entries[0])
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(label, "top.snxsch · 12.35, -6.00");
        assert!(!label.contains('/') && !label.contains('\\') && !label.contains('\n'));
    }
}
