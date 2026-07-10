//! Diagnostic panels -- ERC and Messages.

use super::*;

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
    pub rule_label: &'static str,
    /// Underlying rule kind — drives the Quick Fix chip's label and
    /// per-rule action (UnusedPin → place a NoConnect; others →
    /// zoom + select on the canvas). Carrying it here means the panel
    /// view can decide both the label and the dispatch with no
    /// extra lookup against `erc_violations_by_path`.
    pub rule_kind: signex_erc::RuleKind,
    pub message: String,
    pub world_x: f64,
    pub world_y: f64,
    pub select: Option<signex_types::schematic::SelectedItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcSeverityLite {
    Error,
    Warning,
    Info,
}

pub fn view_erc<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("ERC", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(
                text("Run ERC (F8)")
                    .size(11)
                    .color(theme_ext::text_primary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::RunErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(6).height(Length::Shrink),
            iced::widget::button(
                text("Clear")
                    .size(11)
                    .color(theme_ext::text_secondary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::ClearErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.erc_diagnostics.is_empty() {
        col = col.push(
            text("No ERC diagnostics yet")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        col = col.push(
            text("Run ERC to populate project-wide diagnostics")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        return col.into();
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
                .size(10)
                .color(theme_ext::text_primary(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{errors} errors"))
                .size(10)
                .color(theme_ext::error_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{warnings} warnings"))
                .size(10)
                .color(theme_ext::warning_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{infos} info"))
                .size(10)
                .color(theme_ext::accent(&ctx.tokens)),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(text("<").size(11))
                .padding([1, 6])
                .on_press(PanelMsg::FocusPrevErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            text(focus_label)
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            iced::widget::button(text(">").size(11))
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
            signex_erc::RuleKind::UnusedPin => "Add No-Connect",
            _ => "Show on Canvas",
        };
        col = col.push(
            row![
                iced::widget::button(
                    row![
                        text(sev_label).size(9).color(sev_color),
                        Space::new().width(4).height(Length::Shrink),
                        text(v.rule_label)
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
                .padding([2, 6])
                .on_press(PanelMsg::FocusErcViolation(v.global_index))
                .style(
                    move |_theme: &Theme, status: iced::widget::button::Status| {
                        let base = crate::styles::menu_item(&ctx.tokens)(_theme, status);
                        iced::widget::button::Style {
                            background: row_bg.clone().or(base.background),
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
                .padding([2, 8])
                .on_press(PanelMsg::ErcQuickFix(v.global_index))
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
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("Messages", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            text(format!("level {}", ctx.diagnostics_level))
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.diagnostics.is_empty() {
        col = col.push(
            text("No runtime messages yet")
                .size(10)
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
            container(text(label.to_string()).size(11).color(muted))
                .padding([4, 8])
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
                    .padding([4, 8])
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

// ─── Drawing properties editor (post-placement) ──────────────────
