//! Diagnostic panels -- ERC and Messages.

use super::*;
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Flattened ERC diagnostic row for the ERC panel.
///
/// The app flattens per-sheet ERC caches into this list so the panel can
/// present one navigable table across the entire project.
#[derive(Debug, Clone)]
pub struct ErcDiagnosticEntry {
    pub global_index: usize,
    pub sheet_name: String,
    pub sheet_path: std::path::PathBuf,
    pub severity: ErcSeverityLite,
    pub rule_label: String,
    /// Built-in rule kind (`None` for a DSL rule) — drives the Quick Fix chip's label and
    /// per-rule action (UnusedPin → place a NoConnect; others →
    /// zoom + select on the canvas). Carrying it here means the panel
    /// view can decide both the label and the dispatch with no
    /// extra lookup against `erc_violations_by_path`.
    pub rule_kind: Option<signex_erc::RuleKind>,
    pub message: String,
    pub world_x: f64,
    pub world_y: f64,
    pub select: Option<signex_types::schematic::SelectedItem>,
    /// False when the point is not in `sheet_path`'s own coordinates (a child
    /// sheet that was not part of the run): shown, but never panned to.
    pub locatable: bool,
    /// For a net-level finding: the net to highlight and fit on click.
    pub net: Option<signex_erc::NetFinding>,
    /// Stable rule id (`builtin::..` / `user::..`), the waiver key's rule half.
    pub rule_id: String,
}

/// One stored net waiver, shown under the ERC results with a Restore button.
#[derive(Debug, Clone)]
pub struct ErcWaiverEntry {
    pub rule_id: String,
    pub net: String,
    /// Findings the newest run hid for it; 0 when it matched nothing (the
    /// net was renamed or no longer fails the rule).
    pub hidden: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcSeverityLite {
    Error,
    Warning,
    Info,
}

pub fn view_erc<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(SP.xs).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("ERC", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(
                text("Run ERC (F8)")
                    .size(TS.body)
                    .color(theme_ext::text_primary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::RunErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(6).height(Length::Shrink),
            iced::widget::button(
                text("Clear")
                    .size(TS.body)
                    .color(theme_ext::text_secondary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::ClearErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.erc_running {
        col = col.push(
            text("Running…")
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    }
    if ctx.erc_diagnostics.is_empty() && !ctx.erc_running {
        col = col.push(
            text("No ERC diagnostics yet")
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        col = col.push(
            text("Run ERC to populate project-wide diagnostics")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        return col.push(waived_section(ctx)).into();
    }

    let errors = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Error)
        .count();
    let warnings = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Warning)
        .count();
    let infos = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Info)
        .count();
    let focus_label = if let Some(i) = ctx.erc_focus_index {
        format!("{}/{}", i + 1, ctx.erc_diagnostics.len())
    } else {
        format!("0/{}", ctx.erc_diagnostics.len())
    };

    col = col.push(
        row![
            text("ERC Diagnostic Results")
                .size(TS.caption)
                .color(theme_ext::text_primary(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{errors} errors"))
                .size(TS.caption)
                .color(theme_ext::error_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{warnings} warnings"))
                .size(TS.caption)
                .color(theme_ext::warning_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{infos} info"))
                .size(TS.caption)
                .color(theme_ext::accent(&ctx.tokens)),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(text("<").size(TS.body))
                .padding([1, 6])
                .on_press(PanelMsg::FocusPrevErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            text(focus_label)
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            iced::widget::button(text(">").size(TS.body))
                .padding([1, 6])
                .on_press(PanelMsg::FocusNextErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );

    for v in &ctx.erc_diagnostics {
        let sev_color = match v.severity {
            ErcSeverityLite::Error => theme_ext::error_color(&ctx.tokens),
            ErcSeverityLite::Warning => theme_ext::warning_color(&ctx.tokens),
            ErcSeverityLite::Info => theme_ext::accent(&ctx.tokens),
        };
        let sev_label = match v.severity {
            ErcSeverityLite::Error => "E",
            ErcSeverityLite::Warning => "W",
            ErcSeverityLite::Info => "I",
        };
        let is_focused = ctx.erc_focus_index == Some(v.global_index);
        let row_bg = if is_focused {
            Some(Background::Color(theme_ext::selection_color(&ctx.tokens)))
        } else {
            None
        };
        // Quick Fix chip label per rule kind (UX_IMPROVEMENTS_OVER_ALTIUM
        // §4.4). Only `UnusedPin` has a true mutating fix today —
        // place a NoConnect at the dangling pin. Every other rule's
        // chip is a fast "zoom + select" alias for the row click,
        // so the user has a one-click path to the offending item
        // even when the row's text is long enough that the click
        // target's centre lands far from the cursor.
        let quick_fix_label = match v.rule_kind {
            Some(signex_erc::RuleKind::UnusedPin) => "Add No-Connect",
            _ => "Show on Canvas",
        };
        col = col.push(
            row![
                iced::widget::button(
                    row![
                        text(sev_label).size(9).color(sev_color),
                        Space::new().width(4).height(Length::Shrink),
                        text(v.rule_label.clone())
                            .size(9)
                            .color(theme_ext::text_primary(&ctx.tokens)),
                        Space::new().width(6).height(Length::Shrink),
                        text(v.message.clone())
                            .size(9)
                            .color(theme_ext::text_secondary(&ctx.tokens)),
                        Space::new().width(6).height(Length::Shrink),
                        text(format!(
                            "{}  ({:.2}, {:.2})  {}",
                            v.sheet_name,
                            v.world_x,
                            v.world_y,
                            v.sheet_path.display()
                        ))
                        .size(9)
                        .color(theme_ext::text_secondary(&ctx.tokens)),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::Fill),
                )
                .width(Length::Fill)
                .padding([SP.xs, 6.0])
                .on_press(PanelMsg::FocusErcViolation(v.global_index))
                .style(
                    move |_theme: &Theme, status: iced::widget::button::Status| {
                        let base = crate::styles::menu_item(&ctx.tokens)(_theme, status);
                        iced::widget::button::Style {
                            background: row_bg.or(base.background),
                            ..base
                        }
                    },
                ),
                Space::new().width(6).height(Length::Shrink),
                iced::widget::button(
                    text(quick_fix_label)
                        .size(9)
                        .color(theme_ext::text_primary(&ctx.tokens)),
                )
                .padding([SP.xs, SP.m])
                .on_press(PanelMsg::ErcQuickFix(v.global_index))
                .style(crate::styles::menu_item(&ctx.tokens)),
                waive_chip(v, &ctx.tokens),
            ]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        );
    }

    col.push(waived_section(ctx)).into()
}

/// "Waive net" chip for a net-level finding on a named net; nothing for any
/// other row (a pin No-ERC marker is the waiver for those).
fn waive_chip<'a>(
    v: &ErcDiagnosticEntry,
    tokens: &'a signex_types::theme::ThemeTokens,
) -> Element<'a, PanelMsg> {
    let Some(net) = v.net.as_ref().filter(|n| !n.name.is_empty()) else {
        return Space::new().width(0).height(0).into();
    };
    row![
        Space::new().width(6).height(Length::Shrink),
        iced::widget::button(
            text("Waive net")
                .size(9)
                .color(theme_ext::text_primary(tokens))
        )
        .padding([SP.xs, SP.m])
        .on_press(PanelMsg::ErcNetWaiverSet {
            rule_id: v.rule_id.clone(),
            net: net.name.clone(),
            waive: true,
        })
        .style(crate::styles::menu_item(tokens)),
    ]
    .into()
}

/// The project's net waivers, dimmed, each with a Restore button. Shown even
/// when every finding is waived, so a waiver is never invisible.
fn waived_section<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(SP.xs).width(Length::Fill);
    if ctx.erc_waivers.is_empty() {
        return col.into();
    }
    col = col.push(separator(&ctx.tokens));
    col = col.push(
        text(format!("{} waived", ctx.erc_waivers.len()))
            .size(TS.caption)
            .color(muted),
    );
    for w in &ctx.erc_waivers {
        let note = match w.hidden {
            0 => "matches no finding".to_string(),
            1 => "1 finding hidden".to_string(),
            n => format!("{n} findings hidden"),
        };
        col = col.push(
            row![
                text(format!("{}  net {}  ({note})", w.rule_id, w.net))
                    .size(9)
                    .color(muted),
                Space::new().width(Length::Fill).height(Length::Shrink),
                iced::widget::button(
                    text("Restore")
                        .size(9)
                        .color(theme_ext::text_primary(&ctx.tokens)),
                )
                .padding([SP.xs, SP.m])
                .on_press(PanelMsg::ErcNetWaiverSet {
                    rule_id: w.rule_id.clone(),
                    net: w.net.clone(),
                    waive: false,
                })
                .style(crate::styles::menu_item(&ctx.tokens)),
            ]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        );
    }
    col.into()
}

// ─── Messages Panel ───────────────────────────────────────────

pub fn view_messages<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(SP.xs).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("Messages", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            text(format!("level {}", ctx.diagnostics_level))
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.diagnostics.is_empty() {
        col = col.push(
            text("No runtime messages yet")
                .size(TS.caption)
                .color(theme_ext::success_color(&ctx.tokens)),
        );
        col = col.push(
            text("Set RUST_LOG=debug or SIGNEX_LOG=debug for verbose output")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    } else {
        // Table header — mirrors Altium's Messages panel with fixed
        // columns for id, level, source tag, and the message body.
        let muted = theme_ext::text_secondary(&ctx.tokens);
        let primary = theme_ext::text_primary(&ctx.tokens);
        let border = theme_ext::border_color(&ctx.tokens);
        let header_bg = theme_ext::to_color(&ctx.tokens.hover);
        let row_bg = theme_ext::to_color(&ctx.tokens.panel_bg);
        // Alt-row tint: pull a darker variant from the panel background
        // by shifting alpha. iced::Color lets us blend cheaply.
        let alt_bg = Color {
            a: row_bg.a,
            r: (row_bg.r - 0.02).max(0.0),
            g: (row_bg.g - 0.02).max(0.0),
            b: (row_bg.b - 0.02).max(0.0),
        };

        let th = |label: &str| -> Element<'a, PanelMsg> {
            container(text(label.to_string()).size(TS.body).color(muted))
                .padding([SP.s, SP.m])
                .into()
        };
        // Header row — background fill, no border (separator line
        // below sits in its own element so the table reads as a grid
        // of horizontal rules instead of individually framed boxes).
        col = col.push(
            container(
                row![
                    container(th("#")).width(Length::Fixed(48.0)),
                    container(th("Level")).width(Length::Fixed(64.0)),
                    container(th("Source")).width(Length::Fixed(180.0)),
                    container(th("Message")).width(Length::Fill),
                ]
                .align_y(iced::Alignment::Center),
            )
            .style(move |_theme: &Theme| iced::widget::container::Style {
                background: Some(Background::Color(header_bg)),
                ..iced::widget::container::Style::default()
            }),
        );
        let separator = |bg: Color| -> Element<'a, PanelMsg> {
            container(Space::new())
                .height(Length::Fixed(1.0))
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(bg)),
                    ..iced::widget::container::Style::default()
                })
                .into()
        };
        col = col.push(separator(border));
        for (i, entry) in ctx.diagnostics.iter().rev().enumerate() {
            let level_color = match entry.level {
                crate::diagnostics::DiagnosticLevel::Error => theme_ext::error_color(&ctx.tokens),
                crate::diagnostics::DiagnosticLevel::Warning => {
                    theme_ext::warning_color(&ctx.tokens)
                }
                crate::diagnostics::DiagnosticLevel::Info => theme_ext::accent(&ctx.tokens),
                crate::diagnostics::DiagnosticLevel::Debug
                | crate::diagnostics::DiagnosticLevel::Trace => {
                    theme_ext::text_secondary(&ctx.tokens)
                }
            };
            let bg = if i % 2 == 0 { row_bg } else { alt_bg };
            let cell = |label: String, color: Color, size: f32| -> Element<'a, PanelMsg> {
                container(text(label).size(size).color(color))
                    .padding([SP.s, SP.m])
                    .into()
            };
            col = col.push(
                container(
                    row![
                        container(cell(format!("#{}", entry.id), muted, 11.0))
                            .width(Length::Fixed(48.0)),
                        container(cell(entry.level.label().to_string(), level_color, 11.0,))
                            .width(Length::Fixed(64.0)),
                        container(cell(entry.code.as_str().to_string(), muted, 11.0))
                            .width(Length::Fixed(180.0)),
                        container(cell(entry.message.as_str().to_string(), primary, 12.0))
                            .width(Length::Fill),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                // No border on the row — the explicit separator line
                // drawn below is the grid. Boxes around each entry
                // (the previous look) were too heavy for a log table.
                .style(move |_theme: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(bg)),
                    ..iced::widget::container::Style::default()
                }),
            );
            // One-pixel horizontal rule between rows — no right / left
            // borders so the table reads as a stack of records with
            // shared separators, matching Altium's Messages panel.
            col = col.push(separator(border));
        }
    }

    container(col).width(Length::Fill).into()
}
