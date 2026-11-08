//! BOM preview — the Rules tab: one checkbox per validation rule plus the
//! live issue list grouped by severity. Pure rendering; the rules and the
//! report come from `signex-bom` (via `signex-output`), the toggles route
//! through `BomPreviewMsg::SetRule`.

use super::super::*;
use iced::widget::{Column, Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_output::{BomIssueSeverity, BomRule};
use signex_types::theme::ThemeTokens;

use super::super::widgets::section_header;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    /// Build the Rules tab pane for the active preview.
    pub(super) fn bom_rules_pane(&self) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);

        let mut rule_list: Column<Element<'_, Message>> = Column::new().spacing(0);
        for rule in BomRule::ALL {
            let on = preview.options.rule_options.is_enabled(rule);
            rule_list = rule_list.push(rule_row(rule, on, text_c, border_c, accent_c, tokens));
        }

        let mut issues: Column<Element<'_, Message>> = Column::new().spacing(SP.xs);
        let report = &preview.validation_report;
        if report.issues.is_empty() {
            issues = issues.push(
                text("No issues found.")
                    .size(TS.body)
                    .color(text_muted)
                    .boxed(),
            );
        }
        for (severity, heading) in [
            (BomIssueSeverity::Error, "Errors"),
            (BomIssueSeverity::Warning, "Warnings"),
            (BomIssueSeverity::Info, "Notes"),
        ] {
            let of_severity: Vec<_> = report
                .issues
                .iter()
                .filter(|issue| issue.severity == severity)
                .collect();
            if of_severity.is_empty() {
                continue;
            }
            issues = issues.push(
                text(format!("{heading} ({})", of_severity.len()))
                    .size(TS.body)
                    .color(severity_color(severity, text_c, tokens))
                    .boxed(),
            );
            for issue in of_severity {
                issues = issues.push(
                    column![
                        text(issue.message.clone()).size(TS.body).color(text_c),
                        text(issue.references.join(", "))
                            .size(TS.caption)
                            .color(text_muted),
                    ]
                    .spacing(1)
                    .padding([SP.xs, SP.m])
                    .boxed(),
                );
            }
            issues = issues.push(Space::new().height(6).boxed());
        }

        column![
            section_header("Validation Rules", text_muted),
            container(rule_list).padding([SP.s, 0.0]),
            Space::new().height(8),
            section_header("Issues", text_muted),
            container(issues).padding([SP.m, SP.l]),
        ]
        .spacing(0)
        .boxed()
    }
}

/// Footer-sized issue count (`2 errors, 5 warnings`), `None` when clean.
pub(super) fn issue_count_label(report: &signex_output::BomValidationReport) -> Option<String> {
    report.summary()
}

/// Colour for a severity heading and the footer count.
pub(super) fn severity_color(
    severity: BomIssueSeverity,
    neutral: Color,
    tokens: &ThemeTokens,
) -> Color {
    match severity {
        BomIssueSeverity::Error => signex_widgets::theme_ext::error_color(tokens),
        BomIssueSeverity::Warning => signex_widgets::theme_ext::warning_color(tokens),
        BomIssueSeverity::Info => neutral,
    }
}

fn rule_row<'a>(
    rule: BomRule,
    on: bool,
    text_c: Color,
    border_c: Color,
    accent_c: Color,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let hover_c = crate::styles::tokens::hover_overlay(tokens);
    let pip_bg = if on {
        accent_c
    } else {
        crate::styles::tokens::resolve(tokens).field
    };
    let inner = if on {
        text("\u{2713}").size(TS.caption).color(Color::WHITE)
    } else {
        text(" ").size(TS.caption)
    };
    let pip = container(inner)
        .width(14)
        .height(14)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(pip_bg)),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
                ..Border::default()
            },
            ..container::Style::default()
        });
    button(
        row![
            pip,
            Space::new().width(8),
            text(rule.label()).size(TS.body).color(text_c),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::BomPreview(BomPreviewMsg::SetRule(rule, !on)))
    .padding([3.0, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(hover_c)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..button::Style::default()
        }
    })
    .boxed()
}
