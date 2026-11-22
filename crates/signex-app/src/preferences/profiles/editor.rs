//! The right column of the Rule Profiles page: the selected profile's header,
//! the Severity / Matrix switch and the half it picks (mockup row 148), or
//! one rule opened alone (the maintainer's 2026-11-22 decision, #2765).

use iced::widget::{column, container, row};
use iced::{Alignment, Element, Font, Length, Widget};
use signex_erc::RuleKind;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, segmented, text};

use super::super::{PrefMsg, SP, TS, danger_button_style, text_muted, text_primary, text_warning};
use super::{ProfileEditorTab, ProfilePageMsg, ProfilesMsg, page, pane};
use crate::app::rule_profile_ui::ProfileManager;
use crate::app::view::dialogs::{
    RuleEditor, changed_cell_count, changed_rule_count, matrix_pane, one_rule_line, severity_pane,
    shipped_rules,
};
use crate::rule_profiles::{ListedProfile, ProfileKind};
use crate::styles::{PillTone, pill, tokens::resolve};

fn open_rule(rule: RuleKind) -> PrefMsg {
    page(ProfilePageMsg::RuleOpened(rule))
}

pub(super) fn view<'a>(
    manager: &'a ProfileManager,
    listed: &'a ListedProfile,
    is_saved: bool,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, PrefMsg> {
    let editable = listed.kind != ProfileKind::Builtin;
    let name = listed.profile.name.as_str();
    let resolved = signex_erc::project_rules::resolve(&listed.profile.erc);
    let editor = RuleEditor {
        i18n,
        tokens,
        severity: &resolved.severity,
        pin_matrix: &resolved.pin_matrix,
        shipped: shipped_rules(),
        editable,
        changed_only: manager.rule_list.changed_only,
        filter: &manager.rule_list.query,
        on_severity: |rule, severity| pane(ProfilesMsg::SeverityChanged(rule, severity)),
        on_pin_cell: |row, col| pane(ProfilesMsg::PinCellCycled { row, col }),
        on_changed_only: |on| pane(ProfilesMsg::ChangedOnlyToggled(on)),
        on_filter: |query| pane(ProfilesMsg::RuleFilterChanged(query)),
        on_open_rule: Some(open_rule as fn(RuleKind) -> PrefMsg),
    };
    let body: Element<'static, PrefMsg> = match (manager.open_rule, manager.editor_tab) {
        (Some(rule), _) => one_rule(&editor, rule, name, i18n),
        (None, ProfileEditorTab::Severity) => severity_pane(&editor),
        (None, ProfileEditorTab::PinMatrix) => matrix_pane(&editor),
    };
    let mut out = column![head(manager, name, editable, is_saved, tokens, i18n)]
        .spacing(SP.m)
        .width(Length::Fill);
    if editable && manager.confirm_reset.as_deref() == Some(name) {
        out = out.push(reset_confirm(name, tokens, i18n));
    }
    out.push(tabs(manager.editor_tab, tokens, i18n))
        .push(body)
        .boxed()
}

/// The profile's name and Saved pill, what editing it does, and the reset.
fn head<'a>(
    manager: &ProfileManager,
    name: &'a str,
    editable: bool,
    is_saved: bool,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let mut title = row![
        text(UserText::new(name.to_string()))
            .size(TS.label)
            .font(Font {
                weight: iced::font::Weight::Semibold,
                ..Font::DEFAULT
            })
            .style(text_primary)
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if editable && is_saved {
        title = title.push(
            container(text(t!(i18n, ids::PREFERENCES_PROFILES_SAVED)).size(TS.caption))
                .padding([0.0, SP.s])
                .style(pill(tokens, PillTone::Ok))
                .boxed(),
        );
    }
    let line = if editable {
        t!(i18n, ids::PREFERENCES_PROFILES_SAVED_LINE)
    } else {
        t!(i18n, ids::PREFERENCES_PROFILES_READ_ONLY, "name" => name)
    };
    let mut top = row![
        column![title, text(line).size(TS.caption).style(text_muted)]
            .spacing(SP.xs)
            .width(Length::Fill)
    ]
    .spacing(SP.s)
    .align_y(Alignment::Start);
    if editable {
        top = top.push(
            button(text(t!(i18n, ids::PREFERENCES_PROFILES_RESET_TO_DEFAULT)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press_maybe((!manager.busy).then(|| pane(ProfilesMsg::ResetRequested)))
                .style(crate::styles::outline_button(tokens))
                .boxed(),
        );
    }
    top.boxed()
}

/// The question before the rules go back to the shipped ones.
fn reset_confirm<'a>(name: &'a str, tokens: &'a ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    row![
        text(t!(
            i18n,
            ids::PREFERENCES_PROFILES_RESET_CONFIRM,
            "name" => name
        ))
        .size(TS.body)
        .style(text_warning)
        .width(Length::Fill),
        button(text(t!(i18n, ids::PREFERENCES_PROFILES_RESET)).size(TS.body))
            .padding([SP.xs, SP.m])
            .on_press(pane(ProfilesMsg::ResetConfirmed))
            .style(danger_button_style),
        button(text(t!(i18n, ids::PREFERENCES_PROFILES_KEEP)).size(TS.body))
            .padding([SP.xs, SP.m])
            .on_press(pane(ProfilesMsg::ResetCancelled))
            .style(crate::styles::outline_button(tokens)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// Severity per rule / Pin Connection Matrix.
fn tabs<'a>(tab: ProfileEditorTab, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    segmented(
        [
            (
                ProfileEditorTab::Severity,
                t!(i18n, ids::DIALOG_RULES_SEVERITY_HEADING).into(),
            ),
            (
                ProfileEditorTab::PinMatrix,
                t!(i18n, ids::DIALOG_RULES_MATRIX_HEADING).into(),
            ),
        ],
        tab,
        resolve(tokens).segmented(TS.body),
        |tab| page(ProfilePageMsg::EditorTabPicked(tab)),
    )
}

/// The editor opened for one rule: its profile, what differs, its control.
fn one_rule(
    editor: &RuleEditor<'_, PrefMsg>,
    rule: RuleKind,
    name: &str,
    i18n: &I18n,
) -> Element<'static, PrefMsg> {
    let title = t!(i18n, ids::PREFERENCES_PROFILES_RULE_TITLE, "name" => name);
    let summary = t!(
        i18n,
        ids::PREFERENCES_PROFILES_RULE_SUMMARY,
        "rules" => RuleKind::ALL.len(),
        "changed" => changed_rule_count(editor.severity, editor.shipped),
        "cells" => changed_cell_count(editor.pin_matrix, editor.shipped)
    );
    column![
        row![
            text(title)
                .size(TS.label)
                .style(text_primary)
                .width(Length::Fill),
            button(text(t!(i18n, ids::DIALOG_BUTTON_CLOSE)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(page(ProfilePageMsg::RuleClosed))
                .style(crate::styles::outline_button(editor.tokens)),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
        text(summary).size(TS.caption).style(text_muted),
        one_rule_line(editor, rule),
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .boxed()
}
