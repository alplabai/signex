//! The DRC results panel of a board (#2399): the last run's violations
//! grouped by rule, filtered by rule, layer and severity, each row picked to
//! select its objects and centre the view, and waived or unwaived with a
//! note. A pure view over [`DrcPanel`]; every action is a [`DrcPanelMsg`]
//! the app runs through the same path as its catalog command.

use iced::widget::{Column, Space, column, container, pick_list, row};
use iced::{Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::pcb_layer::CopperLayer;
use signex_types::violation::Severity;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, checkbox, text, text_input, theme_ext};

use super::drc::{Rows, header, layer_label, note, row_text, status};
use super::{DrcPanel, DrcRow, PanelContext, PanelMsg};
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The prefix of a written rule's id; its name follows.
const USER_RULE_PREFIX: &str = "user::";

/// What the panel shows of a report. View state: never board data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrcFilter {
    /// One rule id, or every rule.
    pub rule: Option<String>,
    /// One copper layer, or every layer.
    pub layer: Option<CopperLayer>,
    /// One severity, or every severity.
    pub severity: Option<Severity>,
    /// Waived violations are listed too (with their note).
    pub show_waived: bool,
}

impl DrcFilter {
    /// Whether `row` passes the filter.
    pub fn shows(&self, row: &DrcRow) -> bool {
        (self.show_waived || row.waiver.is_none())
            && self.rule.as_ref().is_none_or(|rule| *rule == row.rule)
            && self.layer.is_none_or(|layer| row.layers.contains(&layer))
            && self.severity.is_none_or(|s| row.severity == Some(s))
    }
}

/// The note of a waiver being written, for the violation `key`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrcWaiveDraft {
    pub key: String,
    pub note: String,
}

/// One entry of a filter dropdown; `None` is "all".
#[derive(Debug, Clone, PartialEq)]
pub enum DrcFilterChoice {
    Rule(Option<String>),
    Layer(Option<CopperLayer>),
    Severity(Option<Severity>),
}

/// What the user did in the DRC results panel. Violations are named by
/// their waiver key, the same in every run.
#[derive(Debug, Clone, PartialEq)]
pub enum DrcPanelMsg {
    /// A row was clicked: select its objects and centre the view on it.
    RowPicked(String),
    /// A filter dropdown was set.
    FilterPicked(DrcFilterChoice),
    /// Show waived was ticked or cleared.
    WaivedShownToggled(bool),
    /// Waive was pressed on a row: its note field opens.
    WaiveStarted(String),
    /// The note was edited.
    NoteEdited(String),
    /// The note's Waive (or Enter): the waiver is stored with the note.
    WaiveConfirmed,
    /// The note's Cancel: nothing is stored.
    WaiveCancelled,
    /// Unwaive was pressed on a waived row.
    Unwaived(String),
}

fn drc_msg(msg: DrcPanelMsg) -> PanelMsg {
    PanelMsg::Drc(msg)
}

/// The rows grouped by rule id, groups in the order their first row comes
/// (the report's order: most severe kind first).
pub(crate) fn groups<'r>(rows: &[&'r DrcRow]) -> Vec<(&'r str, Vec<&'r DrcRow>)> {
    let mut out: Vec<(&'r str, Vec<&'r DrcRow>)> = Vec::new();
    for row in rows {
        match out.iter_mut().find(|(rule, _)| *rule == row.rule) {
            Some((_, members)) => members.push(*row),
            None => out.push((row.rule.as_str(), vec![*row])),
        }
    }
    out
}

/// A rule as the panel names it: a written rule by its name, a built-in one
/// by what it checks.
fn rule_label(i18n: &I18n, row: &DrcRow) -> String {
    match row.rule.strip_prefix(USER_RULE_PREFIX) {
        Some(name) => name.to_string(),
        None => row_text(i18n, row).0.into_string(),
    }
}

fn severity_label(i18n: &I18n, severity: Option<Severity>) -> Localized {
    match severity {
        Some(Severity::Error) => t!(i18n, ids::DRC_PANEL_SEVERITY_ERROR),
        Some(Severity::Warning) => t!(i18n, ids::DRC_PANEL_SEVERITY_WARNING),
        Some(Severity::Info) => t!(i18n, ids::DRC_PANEL_SEVERITY_INFO),
        None => t!(i18n, ids::DRC_PANEL_SEVERITY_OFF),
    }
}

fn severity_color(ctx: &PanelContext, severity: Option<Severity>) -> Color {
    match severity {
        Some(Severity::Error) => theme_ext::error_color(&ctx.tokens),
        Some(Severity::Warning) => theme_ext::warning_color(&ctx.tokens),
        Some(Severity::Info) | None => theme_ext::text_secondary(&ctx.tokens),
    }
}

/// The text of one dropdown entry.
fn choice_label(i18n: &I18n, drc: &DrcPanel, choice: &DrcFilterChoice) -> String {
    match choice {
        DrcFilterChoice::Rule(None) => t!(i18n, ids::DRC_PANEL_FILTER_ALL_RULES).into_string(),
        DrcFilterChoice::Rule(Some(rule)) => drc
            .rows
            .iter()
            .find(|row| row.rule == *rule)
            .map_or_else(|| rule.clone(), |row| rule_label(i18n, row)),
        DrcFilterChoice::Layer(None) => t!(i18n, ids::DRC_PANEL_FILTER_ALL_LAYERS).into_string(),
        DrcFilterChoice::Layer(layer) => layer_label(i18n, *layer)
            .map(Localized::into_string)
            .unwrap_or_default(),
        DrcFilterChoice::Severity(None) => {
            t!(i18n, ids::DRC_PANEL_FILTER_ALL_SEVERITIES).into_string()
        }
        DrcFilterChoice::Severity(severity) => severity_label(i18n, *severity).into_string(),
    }
}

/// The three filter dropdowns and Show waived.
fn filters<'a>(ctx: &'a PanelContext, drc: &'a DrcPanel) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let mut rules = vec![DrcFilterChoice::Rule(None)];
    let mut layers = vec![DrcFilterChoice::Layer(None)];
    for row in &drc.rows {
        let rule = DrcFilterChoice::Rule(Some(row.rule.clone()));
        if !rules.contains(&rule) {
            rules.push(rule);
        }
        for layer in &row.layers {
            let layer = DrcFilterChoice::Layer(Some(*layer));
            if !layers.contains(&layer) {
                layers.push(layer);
            }
        }
    }
    let severities = vec![
        DrcFilterChoice::Severity(None),
        DrcFilterChoice::Severity(Some(Severity::Error)),
        DrcFilterChoice::Severity(Some(Severity::Warning)),
        DrcFilterChoice::Severity(Some(Severity::Info)),
    ];
    let label = move |choice: &DrcFilterChoice| choice_label(i18n, drc, choice);
    let dropdown = |selected: DrcFilterChoice, options: Vec<DrcFilterChoice>| {
        pick_list(Some(selected), options, label)
            .graphite()
            .on_select(|choice: DrcFilterChoice| drc_msg(DrcPanelMsg::FilterPicked(choice)))
            .width(Length::Fill)
            .boxed()
    };
    let filter = &drc.filter;
    let waived = checkbox(filter.show_waived)
        .size(TS.body)
        .labelled(t!(i18n, ids::DRC_PANEL_SHOW_WAIVED))
        .text_size(TS.body)
        .style(super::form_look::checkbox_style)
        .on_toggle(|on| drc_msg(DrcPanelMsg::WaivedShownToggled(on)));
    column![
        row![
            dropdown(DrcFilterChoice::Rule(filter.rule.clone()), rules),
            dropdown(DrcFilterChoice::Layer(filter.layer), layers),
            dropdown(DrcFilterChoice::Severity(filter.severity), severities),
        ]
        .spacing(SP.xs),
        waived,
    ]
    .spacing(SP.xs)
    .width(Length::Fill)
    .into()
}

/// A group's heading: the rule and its count, and the Rule link, greyed
/// with its reason while boards have no Rules table.
fn group_header<'a>(ctx: &'a PanelContext, first: &DrcRow, count: usize) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let title = t!(
        i18n, ids::DRC_PANEL_GROUP,
        "rule" => rule_label(i18n, first), "count" => count
    );
    let link = dialog_button(
        t!(i18n, ids::DRC_PANEL_RULE_LINK),
        DialogButtonKind::Secondary,
        None::<PanelMsg>,
        &ctx.tokens,
    );
    let link = super::graphite::hint_tooltip(
        link,
        t!(i18n, ids::DRC_PANEL_RULE_LINK_UNAVAILABLE),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    row![
        text(title)
            .size(TS.label)
            .color(severity_color(ctx, first.severity)),
        Space::new().width(Length::Fill).height(Length::Shrink),
        link,
    ]
    .align_y(iced::Alignment::Center)
    .padding([SP.xs, 0.0])
    .into()
}

/// The clickable part of a row: severity, what broke and the nets; then
/// the objects, the place and the layer.
fn row_body<'a>(ctx: &'a PanelContext, violation: &DrcRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let (kind, detail, place) = row_text(i18n, violation);
    let first = row![
        text(severity_label(i18n, violation.severity))
            .size(TS.caption)
            .color(severity_color(ctx, violation.severity)),
        text(kind).size(TS.body).color(primary),
        text(detail).size(TS.body).color(primary),
    ]
    .spacing(SP.s + SP.xs)
    .align_y(iced::Alignment::Center);
    let mut second = row![
        text(t!(i18n, ids::DRC_PANEL_OBJECTS, "count" => violation.objects))
            .size(TS.caption)
            .color(muted),
        text(place).size(TS.caption).color(muted),
    ]
    .spacing(SP.s + SP.xs);
    if let Some(layer) = layer_label(i18n, violation.layer) {
        second = second.push(text(layer).size(TS.caption).color(muted));
    }
    let mut lines = column![first, second].spacing(SP.xs);
    if let Some(waiver) = &violation.waiver {
        let said = if waiver.is_empty() {
            t!(i18n, ids::DRC_PANEL_WAIVED_NO_NOTE)
        } else {
            t!(i18n, ids::DRC_PANEL_WAIVED_NOTE, "note" => waiver.clone())
        };
        lines = lines.push(text(said).size(TS.caption).color(muted));
    }
    signex_widgets::button(lines)
        .width(Length::Fill)
        .padding([SP.xs, SP.s + SP.xs])
        .on_press(drc_msg(DrcPanelMsg::RowPicked(violation.key.clone())))
        .style(crate::styles::menu_item(&ctx.tokens))
        .boxed()
}

/// The note field of the waiver being written, with Waive and Cancel.
fn waive_form<'a>(ctx: &'a PanelContext, draft: &'a DrcWaiveDraft) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let field = text_input(t!(i18n, ids::DRC_PANEL_WAIVE_NOTE), draft.note.clone())
        .on_input(|note| drc_msg(DrcPanelMsg::NoteEdited(note)))
        .on_submit(drc_msg(DrcPanelMsg::WaiveConfirmed))
        .size(TS.body)
        .padding(SP.s)
        .style(super::form_look::text_input_style)
        .width(Length::Fill);
    row![
        field,
        dialog_button(
            t!(i18n, ids::DRC_PANEL_WAIVE_CONFIRM),
            DialogButtonKind::Primary,
            Some(drc_msg(DrcPanelMsg::WaiveConfirmed)),
            &ctx.tokens,
        ),
        dialog_button(
            t!(i18n, ids::DRC_PANEL_WAIVE_CANCEL),
            DialogButtonKind::Secondary,
            Some(drc_msg(DrcPanelMsg::WaiveCancelled)),
            &ctx.tokens,
        ),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .into()
}

/// One violation: its body, Waive or Unwaive, and the note field while a
/// waiver of it is being written.
fn violation<'a>(
    ctx: &'a PanelContext,
    drc: &'a DrcPanel,
    item: &'a DrcRow,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let (label, msg) = match item.waiver {
        Some(_) => (
            t!(i18n, ids::DRC_PANEL_UNWAIVE),
            DrcPanelMsg::Unwaived(item.key.clone()),
        ),
        None => (
            t!(i18n, ids::DRC_PANEL_WAIVE),
            DrcPanelMsg::WaiveStarted(item.key.clone()),
        ),
    };
    let action = dialog_button(
        label,
        DialogButtonKind::Secondary,
        Some(drc_msg(msg)),
        &ctx.tokens,
    );
    let line = row![row_body(ctx, item), action]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center);
    let mut col = Column::new().push(line).spacing(SP.xs);
    if let Some(draft) = drc.waive_draft.as_ref().filter(|d| d.key == item.key) {
        col = col.push(waive_form(ctx, draft));
    }
    col.into()
}

/// The panel for the board in view.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let Some(drc) = ctx.pcb_drc.as_ref() else {
        return container(note(t!(i18n, ids::DRC_PANEL_NO_BOARD), muted))
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .into();
    };
    let col: Rows<'a> = Column::new()
        .spacing(SP.xs)
        .padding([SP.xs, SP.l])
        .width(Length::Fill);
    let mut col = status(ctx, drc, header(ctx, drc, col));
    if drc.running || !drc.ran {
        return col.into();
    }
    col = col.push(filters(ctx, drc));
    let waived = drc.rows.iter().filter(|r| r.waiver.is_some()).count();
    if waived > 0 {
        col = col.push(note(
            t!(i18n, ids::DRC_PANEL_WAIVED_COUNT, "count" => waived),
            muted,
        ));
    }
    let shown: Vec<&DrcRow> = drc.rows.iter().filter(|r| drc.filter.shows(r)).collect();
    if shown.is_empty() && !drc.rows.is_empty() {
        col = col.push(note(t!(i18n, ids::DRC_PANEL_NONE_SHOWN), muted));
    }
    for (_, members) in groups(&shown) {
        col = col.push(group_header(ctx, members[0], members.len()));
        for item in members {
            col = col.push(violation(ctx, drc, item));
        }
    }
    if drc.hidden > 0 {
        col = col.push(note(t!(i18n, ids::DRC_MORE, "count" => drc.hidden), muted));
    }
    col.into()
}

#[cfg(test)]
#[path = "drc_results_tests.rs"]
mod tests;
