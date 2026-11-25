//! BOM dialog — the Rules tab: one checkbox per validation rule, then the
//! live issues grouped by severity. A click on an issue selects its line.
//! The rules and the report come from `signex-bom` (via `signex-output`).

use super::super::*;
use iced::widget::{Column, Space, column, container, row};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_i18n::{UserText, ids, t};
use signex_output::{BomIssueSeverity, BomRule, BomValidationIssue};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::sidebar::{caption, card_list, check_row, section};
use crate::app::state::BomIssueLine;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Least height of an issue row, as drawn.
const ISSUE_ROW_H: f32 = 34.0;

impl Signex {
    /// Build the Rules tab pane for the active preview.
    pub(super) fn bom_rules_pane(&self) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let report = &preview.validation_report;

        let mut pane: Column<Element<'_, Message>> =
            Column::new().push(section(i18n, ids::BOM_SECTION_RULES, None, true, tokens));
        for rule in BomRule::ALL {
            let on = preview.options.rule_options.is_enabled(rule);
            pane = pane.push(check_row(
                t!(i18n, rule_label(rule)),
                on,
                Message::BomPreview(BomPreviewMsg::SetRule(rule, !on)),
                tokens,
            ));
        }
        pane = pane.push(section(
            i18n,
            ids::BOM_SECTION_ISSUES,
            issue_pill(report, tokens),
            false,
            tokens,
        ));
        if report.issues.is_empty() {
            return pane
                .push(Space::new().height(SP.s).boxed())
                .push(caption(t!(i18n, ids::BOM_NO_ISSUES_FOUND), tokens))
                .boxed();
        }
        for (severity, heading) in [
            (BomIssueSeverity::Error, ids::BOM_ISSUES_ERRORS),
            (BomIssueSeverity::Warning, ids::BOM_ISSUES_WARNINGS),
            (BomIssueSeverity::Info, ids::BOM_ISSUES_NOTES),
        ] {
            let items: Vec<Element<'_, Message>> = report
                .issues
                .iter()
                .enumerate()
                .filter(|(_, issue)| issue.severity == severity)
                .map(|(index, issue)| {
                    let at = preview.issue_lines.get(index).copied().flatten();
                    let part = at
                        .and_then(|at| preview.table.rows.get(at.row))
                        .map(|row| {
                            if row.value.is_empty() {
                                row.name.clone()
                            } else {
                                row.value.clone()
                            }
                        })
                        .unwrap_or_default();
                    issue_row(issue, index, at, part, i18n, tokens)
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            pane = pane
                .push(Space::new().height(SP.m).boxed())
                .push(caption(t!(i18n, heading, "count" => items.len()), tokens))
                .push(Space::new().height(SP.xs).boxed())
                .push(card_list(items, tokens));
        }
        pane.push(Space::new().height(SP.s).boxed())
            .push(caption(t!(i18n, ids::BOM_ISSUES_HINT), tokens))
            .boxed()
    }
}

/// The catalog name of a validation rule.
pub(super) fn rule_label(rule: BomRule) -> signex_i18n::MessageId {
    match rule {
        BomRule::MissingFootprint => ids::BOM_RULE_MISSING_FOOTPRINT,
        BomRule::MissingMpn => ids::BOM_RULE_MISSING_MPN,
        BomRule::DuplicateDesignator => ids::BOM_RULE_DUPLICATE_DESIGNATOR,
        BomRule::EmptyOrZeroQty => ids::BOM_RULE_EMPTY_OR_ZERO_QTY,
        BomRule::UnitFieldConflict => ids::BOM_RULE_UNIT_FIELD_CONFLICT,
    }
}

/// Colour of a severity's glyph and count: error, warning, or the muted ink
/// for notes.
fn severity_ink(severity: BomIssueSeverity, tokens: &ThemeTokens) -> Color {
    match severity {
        BomIssueSeverity::Error => ti(tokens.error),
        BomIssueSeverity::Warning => ti(tokens.warning),
        BomIssueSeverity::Info => resolve(tokens).ink3,
    }
}

/// The count pill beside the Issues heading, toned by the worst severity.
fn issue_pill<'a>(
    report: &signex_output::BomValidationReport,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    if report.issues.is_empty() {
        return None;
    }
    let tone = if report.has_errors() {
        PillTone::Err
    } else if report.warning_count() > 0 {
        PillTone::Warn
    } else {
        PillTone::Neutral
    };
    // raw: a count, not language.
    Some(
        container(text(raw(report.issues.len().to_string())).size(TS.caption))
            .padding([0.0, SP.s])
            .style(crate::styles::pill(tokens, tone))
            .boxed(),
    )
}

/// One issue: its glyph, the designators and the rule, and under them the
/// line it is on. A click selects that line. A unit-field conflict also
/// names the field, which only the validator's message carries.
fn issue_row<'a>(
    issue: &BomValidationIssue,
    index: usize,
    at: Option<BomIssueLine>,
    part: String,
    i18n: &signex_i18n::I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let glyph = match issue.severity {
        BomIssueSeverity::Info => "\u{2139}",
        BomIssueSeverity::Error | BomIssueSeverity::Warning => "\u{26A0}",
    };
    let mono = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..super::super::graphite::mono()
    };
    let title = row![
        text(UserText::new(issue.references.join(", ")))
            .size(TS.body)
            .font(mono)
            .color(ink),
        text(t!(i18n, rule_label(issue.rule)))
            .size(TS.body)
            .color(ink),
    ]
    .spacing(SP.s);
    let mut lines: Column<Element<'a, Message>> = Column::new().push(title.boxed());
    if let Some(at) = at {
        lines = lines.push(
            text(t!(i18n, ids::BOM_ISSUE_LINE, "line" => at.line, "part" => part))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    if issue.rule == BomRule::UnitFieldConflict {
        // raw: the validator's own message, in English (#2061).
        lines = lines.push(
            text(raw(issue.message.clone()))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    let hover = ti(tokens.hover);
    let line = r.line_soft;
    // The zero-width strut keeps the row at least as tall as drawn.
    // raw: a status glyph, not language.
    let content = column![
        container(
            row![
                Space::new()
                    .width(0)
                    .height(ISSUE_ROW_H - crate::styles::HAIRLINE),
                text(raw(glyph))
                    .size(TS.body)
                    .color(severity_ink(issue.severity, tokens)),
                lines.width(Length::Fill),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.xs, SP.m])
        .width(Length::Fill),
        container(Space::new())
            .width(Length::Fill)
            .height(crate::styles::HAIRLINE)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            }),
    ];
    // An issue on no line (its parts are left out of the BOM) has nothing
    // to select.
    let on_press = at.map(|_| Message::BomPreview(BomPreviewMsg::IssueSelected(index)));
    let pressable = on_press.is_some();
    button(content)
        .padding(0)
        .width(Length::Fill)
        .on_press_maybe(on_press)
        .style(move |_: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: (pressable && hovered).then_some(Background::Color(hover)),
                text_color: ink,
                border: iced::Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..iced::Border::default()
                },
                ..button::Style::default()
            }
        })
        .boxed()
}
