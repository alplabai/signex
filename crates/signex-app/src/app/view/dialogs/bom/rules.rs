//! BOM preview — the Rules tab: one checkbox per validation rule plus the
//! live issue list grouped by severity. Pure rendering; the rules and the
//! report come from `signex-bom` (via `signex-output`), the toggles route
//! through `BomPreviewMsg::SetRule`.

use super::super::*;
use iced::widget::{Column, Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_output::{BomIssueSeverity, BomRule};

use super::super::widgets::section_header;

impl Signex {
    /// Build the Rules tab pane for the active preview.
    pub(super) fn bom_rules_pane(&self) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);

        let mut rule_list: Column<'_, Message> = Column::new().spacing(0);
        for rule in BomRule::ALL {
            let on = preview.options.rule_options.is_enabled(rule);
            rule_list = rule_list.push(rule_row(rule, on, text_c, border_c, accent_c));
        }

        let mut issues: Column<'_, Message> = Column::new().spacing(2);
        let report = &preview.validation_report;
        if report.issues.is_empty() {
            issues = issues.push(text("No issues found.").size(11).color(text_muted));
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
                    .size(11)
                    .color(severity_color(severity, text_c)),
            );
            for issue in of_severity {
                issues = issues.push(
                    column![
                        text(issue.message.clone()).size(11).color(text_c),
                        text(issue.references.join(", ")).size(10).color(text_muted),
                    ]
                    .spacing(1)
                    .padding([2, 8]),
                );
            }
            issues = issues.push(Space::new().height(6));
        }

        column![
            section_header("Validation Rules", text_muted),
            container(rule_list).padding([4, 0]),
            Space::new().height(8),
            section_header("Issues", text_muted),
            container(issues).padding([8, 12]),
        ]
        .spacing(0)
        .into()
    }
}

/// Footer-sized issue count (`2 errors, 5 warnings`), `None` when clean.
pub(super) fn issue_count_label(report: &signex_output::BomValidationReport) -> Option<String> {
    report.summary()
}

/// Colour for a severity heading and the footer count.
pub(super) fn severity_color(severity: BomIssueSeverity, neutral: Color) -> Color {
    match severity {
        BomIssueSeverity::Error => Color::from_rgb(0.90, 0.33, 0.30),
        BomIssueSeverity::Warning => Color::from_rgb(0.92, 0.70, 0.20),
        BomIssueSeverity::Info => neutral,
    }
}

fn rule_row<'a>(
    rule: BomRule,
    on: bool,
    text_c: Color,
    border_c: Color,
    accent_c: Color,
) -> Element<'a, Message> {
    let pip_bg = if on {
        accent_c
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let inner = if on {
        text("\u{2713}").size(9).color(Color::WHITE)
    } else {
        text(" ").size(9)
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
            },
            ..container::Style::default()
        });
    button(
        row![
            pip,
            Space::new().width(8),
            text(rule.label()).size(11).color(text_c),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::BomPreview(BomPreviewMsg::SetRule(rule, !on)))
    .padding([3, 12])
    .width(Length::Fill)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => {
                Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04)))
            }
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..button::Style::default()
        }
    })
    .into()
}
