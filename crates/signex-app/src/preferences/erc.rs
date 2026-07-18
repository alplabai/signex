//! Electrical Rules section — per-rule severity override chips and the
//! reset-to-defaults action. Moved verbatim from the former single-file
//! `preferences` module.

use super::*;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub(super) fn content_erc<'a>(
    overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) -> Element<'a, PrefMsg> {
    use signex_erc::{RuleKind, Severity};
    const RULES: &[RuleKind] = &[
        RuleKind::UnusedPin,
        RuleKind::DuplicateRefDesignator,
        RuleKind::HierPortDisconnected,
        RuleKind::DanglingWire,
        RuleKind::NetLabelConflict,
        RuleKind::OrphanLabel,
        RuleKind::BusBitWidthMismatch,
        RuleKind::BadHierSheetPin,
        RuleKind::MissingPowerFlag,
        RuleKind::PowerPortShort,
        RuleKind::SymbolOutsideSheet,
    ];
    const CHOICES: &[Severity] = &[
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Off,
    ];

    let header = column![
        text("Electrical Rules Severity").size(15).style(text_primary),
        Space::new().height(4),
        text("Per-rule severity override. Errors show red, Warnings yellow, Info blue; Off silences the rule entirely.")
            .size(11)
            .style(text_muted),
    ]
    .padding([16, 20]);

    let mut rows_col = column![].spacing(0).padding([0, 20]);
    for rule in RULES {
        let current = overrides
            .get(rule)
            .copied()
            .unwrap_or_else(|| rule.default_severity());
        let default_sev = rule.default_severity();
        let mut row_ui = row![
            text(rule.label())
                .size(12)
                .style(text_primary)
                .width(Length::Fill)
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        for &sev in CHOICES {
            let active = current == sev;
            let r = *rule;
            let s = sev;
            let d = default_sev;
            row_ui = row_ui.push(
                button(
                    text(severity_label(sev))
                        .size(11)
                        .style(move |theme: &Theme| text::Style {
                            // Active chips sit on the saturated (theme-neutral)
                            // severity fill, so white reads on both palettes;
                            // inactive chips use the muted secondary token.
                            color: Some(if active {
                                Color::from_rgb(1.0, 1.0, 1.0)
                            } else {
                                theme.extended_palette().secondary.base.color
                            }),
                        }),
                )
                .padding([4, 10])
                .on_press(PrefMsg::DraftErcSeverity(r, if s == d { d } else { s }))
                .style(move |theme: &Theme, status: button::Status| {
                    let palette = theme.extended_palette();
                    let bg = if active {
                        severity_bg(sev)
                    } else if matches!(status, button::Status::Hovered) {
                        palette.background.strong.color
                    } else {
                        palette.background.weak.color
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: if active {
                                severity_bg(sev)
                            } else {
                                palette.background.strong.color
                            },
                        },
                        ..button::Style::default()
                    }
                }),
            );
        }
        rows_col = rows_col.push(container(row_ui).padding([6, 4]).width(Length::Fill).style(
            |theme: &Theme| container::Style {
                background: None,
                border: Border {
                    width: 1.0,
                    color: theme.extended_palette().background.strong.color,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            },
        ));
    }

    let reset_row = container(
        row![
            Space::new().width(Length::Fill),
            button(text("Reset to defaults").size(11))
                .padding([5, 12])
                .on_press(PrefMsg::ResetErcSeverities)
                .style(primary_button_style),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([12, 20])
    .width(Length::Fill);

    column![header, rows_col, reset_row].spacing(0).into()
}

fn severity_label(sev: signex_erc::Severity) -> &'static str {
    match sev {
        signex_erc::Severity::Error => "Error",
        signex_erc::Severity::Warning => "Warning",
        signex_erc::Severity::Info => "Info",
        signex_erc::Severity::Off => "Off",
    }
}

fn severity_bg(sev: signex_erc::Severity) -> Color {
    match sev {
        signex_erc::Severity::Error => Color::from_rgb(0.58, 0.20, 0.22),
        signex_erc::Severity::Warning => Color::from_rgb(0.55, 0.45, 0.12),
        signex_erc::Severity::Info => Color::from_rgb(0.20, 0.36, 0.58),
        signex_erc::Severity::Off => Color::from_rgb(0.28, 0.28, 0.32),
    }
}
