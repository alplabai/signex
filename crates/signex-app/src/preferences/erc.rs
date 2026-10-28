//! Electrical Rules section — per-rule severity override chips and the
//! reset-to-defaults action. Moved verbatim from the former single-file
//! `preferences` module.

use super::*;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn content_erc<'a>(
    overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) -> Element<'a, PrefMsg> {
    use signex_erc::{RuleKind, Severity};
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
            .size(TS.body)
            .style(text_muted),
    ]
    .padding([SP.xl, 20.0]);

    let mut rows_col = column![].spacing(0).padding([0, 20]);
    for rule in &RuleKind::ALL {
        let current = overrides
            .get(rule)
            .copied()
            .unwrap_or_else(|| rule.default_severity());
        let on_off_only = rule.is_on_off_only();
        let label = if on_off_only {
            format!("{} (on/off; severity per matrix cell)", rule.label())
        } else {
            rule.label().to_string()
        };
        let mut row_ui = row![
            text(label)
                .size(TS.label)
                .style(text_primary)
                .width(Length::Fill)
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        for &sev in CHOICES {
            // An on/off-only row offers its default level (shown "On") and Off.
            if on_off_only && sev != rule.default_severity() && sev != Severity::Off {
                continue;
            }
            let active = if on_off_only && sev != Severity::Off {
                current != Severity::Off
            } else {
                current == sev
            };
            let r = *rule;
            let s = sev;
            row_ui = row_ui.push(
                button(
                    text(if on_off_only && sev != Severity::Off {
                        "On"
                    } else {
                        severity_label(sev)
                    })
                    .size(TS.body)
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
                .padding([SP.s, 10.0])
                .on_press(PrefMsg::DraftErcSeverity(r, s))
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
        rows_col = rows_col.push(
            container(row_ui)
                .padding([6.0, SP.s])
                .width(Length::Fill)
                .style(|theme: &Theme| container::Style {
                    background: None,
                    border: Border {
                        width: 1.0,
                        color: theme.extended_palette().background.strong.color,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }),
        );
    }

    let reset_row = container(
        row![
            Space::new().width(Length::Fill),
            button(text("Reset to defaults").size(TS.body))
                .padding([5.0, SP.l])
                .on_press(PrefMsg::ResetErcSeverities)
                .style(primary_button_style),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.l, 20.0])
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
