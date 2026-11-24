//! The ERC rule editor: the rule list, grouped and filterable, beside the
//! pin-connection matrix (#1289).
//!
//! One widget for every place that edits a rule set: the ERC dialog (the
//! project's rules, or the machine's) and Preferences ▸ Rule Profiles (a
//! library profile). It is generic over the message type, so each caller maps
//! a click to its own message and none of them forks the layout (#1608). It
//! holds no state: the list filter lives in the caller's model.

mod pin_matrix;
mod severity_list;

use std::collections::HashMap;

use iced::widget::{container, row};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_erc::pin_matrix::pair_cell;
use signex_erc::project_rules::ResolvedRules;
use signex_erc::{PinAxis, RuleGroup, RuleKind, Severity};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_widgets::{raw, text};

use crate::styles::{Radii, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;

/// Width of the matrix pane: 410 px at the 12 px body.
const MATRIX_PANE_W: f32 = TS.body * 410.0 / 12.0;

/// The shipped profile's rules, resolved once: a constant that every frame
/// of the ERC dialog and of Rule Profiles compares with.
pub(crate) fn shipped_rules() -> &'static ResolvedRules {
    static SHIPPED: std::sync::LazyLock<ResolvedRules> = std::sync::LazyLock::new(|| {
        signex_erc::project_rules::resolve(
            &signex_erc::project_rules::profile::builtin_profile().erc,
        )
    });
    &SHIPPED
}

/// What the editor shows and how a click reaches the caller.
pub(crate) struct RuleEditor<'a, M> {
    pub i18n: &'a I18n,
    pub tokens: &'a signex_types::theme::ThemeTokens,
    /// Per-rule severity overrides; a rule not listed shows its default.
    pub severity: &'a HashMap<RuleKind, Severity>,
    /// Pin-matrix cells that differ from the factory matrix.
    pub pin_matrix: &'a HashMap<(u8, u8), Severity>,
    /// The shipped profile: the baseline the changed markers compare with.
    pub shipped: &'a ResolvedRules,
    /// `false` draws the same grid with every cell inert (the shipped
    /// profile).
    pub editable: bool,
    /// List only the rules that differ from `shipped`.
    pub changed_only: bool,
    /// Text of the "Filter rules" field; the list keeps the rules whose name
    /// holds it.
    pub filter: &'a str,
    pub on_severity: fn(RuleKind, Severity) -> M,
    pub on_pin_cell: fn(u8, u8) -> M,
    /// The "Changed only" chip was pressed; carries the new state.
    pub on_changed_only: fn(bool) -> M,
    pub on_filter: fn(String) -> M,
    /// The Rule Profiles page (mockup row 148): a rule's name opens it alone,
    /// and a Rule / Severity table head replaces the section heading.
    pub on_open_rule: Option<fn(RuleKind) -> M>,
}

/// The two-column editor body: the rule list, then the matrix pane.
pub(crate) fn rule_editor<M: Clone + 'static>(editor: RuleEditor<'_, M>) -> Element<'static, M> {
    row![
        container(severity_pane(&editor)).width(Length::Fill),
        matrix_pane(&editor)
    ]
    .align_y(iced::Alignment::Start)
    .boxed()
}

/// The rule list on its own, for a surface that shows one half at a time.
pub(crate) fn severity_pane<M: Clone + 'static>(editor: &RuleEditor<'_, M>) -> Element<'static, M> {
    severity_list::view(editor)
}

/// One rule's name and severity control.
pub(crate) fn one_rule_line<M: Clone + 'static>(
    editor: &RuleEditor<'_, M>,
    rule: RuleKind,
) -> Element<'static, M> {
    severity_list::one_rule(editor, rule)
}

/// The framed pin-connection matrix.
pub(crate) fn matrix_pane<M: Clone + 'static>(editor: &RuleEditor<'_, M>) -> Element<'static, M> {
    let tokens = editor.tokens;
    let chrome = crate::styles::ti(tokens.toolbar_bg);
    let line = crate::styles::ti(tokens.border);
    container(pin_matrix::view(editor))
        .width(MATRIX_PANE_W)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(chrome)),
            border: Border {
                width: crate::styles::HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// An uppercase section label (SEVERITY PER RULE, PINS): the section size
/// and the muted section ink, in capitals for the message language. It keeps
/// one line; a narrow parent clips it rather than wrapping it into a neighbour.
pub(crate) fn section_heading<M: 'static>(
    i18n: &I18n,
    label: MessageId,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'static, M> {
    let words = t!(i18n, label).into_string();
    // raw: catalog text already rendered, then upper-cased for the locale.
    text(raw(i18n.upper_for_display(&words)))
        .size(TS.section)
        .wrapping(iced::widget::text::Wrapping::None)
        .style(crate::styles::section_label(tokens))
        .boxed()
}

/// The severity a rule runs at under `overrides`: the override, else the
/// rule's default; an on/off-only rule's "on" is always its default.
pub(crate) fn effective_severity(
    overrides: &HashMap<RuleKind, Severity>,
    rule: RuleKind,
) -> Severity {
    overrides.get(&rule).copied().map_or_else(
        || rule.default_severity(),
        |chosen| rule.normalize_override(chosen),
    )
}

/// One row of the rule list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuleRow {
    pub rule: RuleKind,
    pub severity: Severity,
    /// Differs from the shipped profile.
    pub changed: bool,
}

/// One group of the rule list, with the rows the filter keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleSection {
    pub group: RuleGroup,
    pub rows: Vec<RuleRow>,
}

/// Whether `rule` runs at another severity than in the shipped profile.
pub(crate) fn rule_differs(
    severity: &HashMap<RuleKind, Severity>,
    shipped: &ResolvedRules,
    rule: RuleKind,
) -> bool {
    effective_severity(severity, rule) != effective_severity(&shipped.severity, rule)
}

/// How many rules differ from the shipped profile.
pub(crate) fn changed_rule_count(
    severity: &HashMap<RuleKind, Severity>,
    shipped: &ResolvedRules,
) -> usize {
    RuleKind::ALL
        .into_iter()
        .filter(|&rule| rule_differs(severity, shipped, rule))
        .count()
}

/// Whether the name of `rule` holds `filter`, ignoring case; an empty filter
/// keeps every rule.
fn name_matches(rule: RuleKind, filter: &str, i18n: &I18n) -> bool {
    let needle = i18n.fold_for_search(filter.trim());
    needle.is_empty() || i18n.fold_for_search(rule.label()).contains(&needle)
}

/// The rule list: every group in order, holding the rules that pass
/// `changed_only` and `filter`. A group left empty is dropped.
pub(crate) fn rule_sections(
    severity: &HashMap<RuleKind, Severity>,
    shipped: &ResolvedRules,
    changed_only: bool,
    filter: &str,
    i18n: &I18n,
) -> Vec<RuleSection> {
    RuleGroup::ALL
        .into_iter()
        .filter_map(|group| {
            let rows: Vec<RuleRow> = group
                .rules()
                .map(|rule| RuleRow {
                    rule,
                    severity: effective_severity(severity, rule),
                    changed: rule_differs(severity, shipped, rule),
                })
                .filter(|row| !changed_only || row.changed)
                .filter(|row| name_matches(row.rule, filter, i18n))
                .collect();
            (!rows.is_empty()).then_some(RuleSection { group, rows })
        })
        .collect()
}

/// One segment of a rule's severity control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Segment {
    /// The severity a press sets.
    pub value: Severity,
    pub label: MessageId,
    pub selected: bool,
}

/// The segments of `rule`'s control: E / W / I / Off, or On / Off for an
/// on/off-only rule, whose "on" is its default severity.
pub(crate) fn severity_segments(rule: RuleKind, current: Severity) -> Vec<Segment> {
    let off = Segment {
        value: Severity::Off,
        label: ids::DIALOG_RULES_OFF,
        selected: current == Severity::Off,
    };
    if rule.is_on_off_only() {
        return vec![
            Segment {
                value: rule.default_severity(),
                label: ids::DIALOG_RULES_ON,
                selected: current != Severity::Off,
            },
            off,
        ];
    }
    let level = |value: Severity, label: MessageId| Segment {
        value,
        label,
        selected: current == value,
    };
    vec![
        level(Severity::Error, ids::DIALOG_RULES_ERROR_SHORT),
        level(Severity::Warning, ids::DIALOG_RULES_WARNING_SHORT),
        level(Severity::Info, ids::DIALOG_RULES_INFO_SHORT),
        off,
    ]
}

/// One drawn cell of the pin-connection matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MatrixCell {
    pub row: u8,
    pub col: u8,
    pub severity: Severity,
    /// Differs from the shipped profile.
    pub changed: bool,
}

/// The cells to draw: the lower triangle (column <= row), one list per axis
/// row. The matrix is symmetric, so the upper triangle would repeat it; a
/// cell overridden only on its mirror (a hand-edited file) shows that
/// override, as the check reads it (`signex_erc::pin_matrix::pair_cell`).
pub(crate) fn lower_triangle(
    pin_matrix: &HashMap<(u8, u8), Severity>,
    shipped: &ResolvedRules,
) -> Vec<Vec<MatrixCell>> {
    (0..PinAxis::ALL.len() as u8)
        .map(|row| {
            (0..=row)
                .map(|col| {
                    let severity = pair_cell(pin_matrix, row, col);
                    MatrixCell {
                        row,
                        col,
                        severity,
                        changed: severity != pair_cell(&shipped.pin_matrix, row, col),
                    }
                })
                .collect()
        })
        .collect()
}

/// How many drawn matrix cells differ from the shipped profile.
pub(crate) fn changed_cell_count(
    pin_matrix: &HashMap<(u8, u8), Severity>,
    shipped: &ResolvedRules,
) -> usize {
    lower_triangle(pin_matrix, shipped)
        .iter()
        .flatten()
        .filter(|cell| cell.changed)
        .count()
}

/// The catalog id of a group's subhead.
fn group_heading(group: RuleGroup) -> MessageId {
    match group {
        RuleGroup::Pins => ids::DIALOG_RULES_GROUP_PINS,
        RuleGroup::Nets => ids::DIALOG_RULES_GROUP_NETS,
        RuleGroup::Labels => ids::DIALOG_RULES_GROUP_LABELS,
        RuleGroup::Buses => ids::DIALOG_RULES_GROUP_BUSES,
        RuleGroup::Hierarchy => ids::DIALOG_RULES_GROUP_HIERARCHY,
        RuleGroup::Parts => ids::DIALOG_RULES_GROUP_PARTS,
    }
}

#[cfg(test)]
mod tests;
