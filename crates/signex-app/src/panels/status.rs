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
#[derive(Debug, Clone, PartialEq)]
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
    /// `world_y` as the sheet shows it: from its page origin (#2034). The
    /// location label reads this; panning uses `world_y`.
    pub shown_y: f64,
    pub select: Option<signex_types::schematic::SelectedItem>,
    /// False when the point is not in `sheet_path`'s own coordinates (a child
    /// sheet that was not part of the run): shown, but never panned to.
    pub locatable: bool,
    /// For a net-level finding: the net to highlight and fit on click.
    pub net: Option<signex_erc::NetFinding>,
    /// Stable rule id (`builtin::..` / `user::..`), the waiver key's rule half.
    pub rule_id: String,
    /// What a per-finding waiver would match (#1285); `None` when the
    /// finding names no object and cannot be waived.
    pub waiver_key: Option<signex_erc::waivers::violation::WaiverKey>,
}

/// One stored net waiver, shown under the ERC results with a Restore button.
#[derive(Debug, Clone, PartialEq)]
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

/// Label and detail text colours of an ERC result row. A focused row is
/// filled with the selection colour, so its text is chosen against that.
pub(crate) fn erc_row_text(tokens: &ThemeTokens, focused: bool) -> (Color, Color) {
    let label = theme_ext::text_primary(tokens);
    let detail = theme_ext::text_secondary(tokens);
    if !focused {
        return (label, detail);
    }
    let fill = theme_ext::selection_color(tokens);
    let readable = |c| crate::styles::tokens::keep_readable_on(c, fill);
    (readable(label), readable(detail))
}

/// Colour of a row's severity letter: on a focused row it is checked
/// against the selection fill like the rest of the row's text.
pub(crate) fn erc_severity_text(tokens: &ThemeTokens, focused: bool, severity: Color) -> Color {
    if focused {
        crate::styles::tokens::keep_readable_on(severity, theme_ext::selection_color(tokens))
    } else {
        severity
    }
}

pub fn view_erc<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).padding(6).width(Length::Fill);
    col = col.push(
        row![
            // The dock title already says "ERC".
            Space::new().boxed(),
            Space::new().width(Length::Fill).height(Length::Shrink),
            signex_widgets::button(
                text(raw("Run ERC (F8)"))
                    .size(TS.body)
                    .color(theme_ext::text_primary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::RunErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(6).height(Length::Shrink),
            signex_widgets::button(
                text(raw("Clear"))
                    .size(TS.body)
                    .color(theme_ext::text_secondary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::ClearErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(separator(&ctx.tokens).boxed());

    if ctx.erc_running {
        col = col.push(
            text(raw("Running…"))
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens))
                .boxed(),
        );
    }
    if ctx.erc_results_stale {
        col = col.push(
            text(raw(STALE_RESULTS_NOTICE))
                .size(TS.caption)
                .color(theme_ext::warning_color(&ctx.tokens))
                .boxed(),
        );
    }
    if ctx.erc_diagnostics.is_empty() && !ctx.erc_running {
        col = col.push(
            text(raw(if ctx.erc_results_stale {
                "No findings left since the last run"
            } else {
                "No ERC diagnostics yet"
            }))
            .size(TS.caption)
            .color(theme_ext::text_secondary(&ctx.tokens))
            .boxed(),
        );
        if !ctx.erc_results_stale {
            col = col.push(
                text(raw("Run ERC to populate project-wide diagnostics"))
                    .size(TS.caption)
                    .color(theme_ext::text_secondary(&ctx.tokens))
                    .boxed(),
            );
        }
        return col
            .push(waived_section(ctx))
            .push(super::erc_waivers::violation_waivers_section(ctx))
            .boxed();
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
    let focus_row = ctx
        .erc_focus_index
        .and_then(|g| ctx.erc_diagnostics.iter().position(|v| v.global_index == g));
    let focus_label = if let Some(i) = focus_row {
        format!("{}/{}", i + 1, ctx.erc_diagnostics.len())
    } else {
        format!("0/{}", ctx.erc_diagnostics.len())
    };

    col = col.push(
        row![
            text(raw("ERC Diagnostic Results"))
                .size(TS.body)
                .color(theme_ext::text_primary(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(raw(format!("{errors} errors")))
                .size(TS.caption)
                .color(theme_ext::error_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(raw(format!("{warnings} warnings")))
                .size(TS.caption)
                .color(theme_ext::warning_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(raw(format!("{infos} info")))
                .size(TS.caption)
                .color(theme_ext::accent(&ctx.tokens)),
            Space::new().width(Length::Fill).height(Length::Shrink),
            signex_widgets::button(text(raw("<")).size(TS.body))
                .padding([1, 6])
                .on_press(PanelMsg::FocusPrevErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            text(raw(focus_label))
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            signex_widgets::button(text(raw(">")).size(TS.body))
                .padding([1, 6])
                .on_press(PanelMsg::FocusNextErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
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
        let (label_c, detail_c) = erc_row_text(&ctx.tokens, is_focused);
        let _sev_color = erc_severity_text(&ctx.tokens, is_focused, sev_color);
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
                signex_widgets::button(
                    row![
                        erc_severity_pill(&ctx.tokens, v.severity, sev_label),
                        Space::new().width(4).height(Length::Shrink),
                        text(raw(v.rule_label.clone())).size(TS.body).color(label_c),
                        Space::new().width(6).height(Length::Shrink),
                        text(raw(v.message.clone())).size(TS.body).color(detail_c),
                        Space::new().width(6).height(Length::Shrink),
                        signex_widgets::tooltip(
                            text(raw(location_label(&v.sheet_name, v.world_x, v.shown_y)))
                                .size(TS.body)
                                .wrapping(iced::widget::text::Wrapping::None)
                                .color(detail_c),
                            iced::widget::container(
                                text(signex_i18n::UserText::new(
                                    v.sheet_path.display().to_string()
                                ))
                                .size(TS.caption),
                            )
                            .padding([SP.xs, 6.0]),
                            iced::widget::tooltip::Position::Top,
                        )
                        .gap(4)
                        .style(crate::styles::context_menu(&ctx.tokens)),
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
                signex_widgets::button(
                    text(raw(quick_fix_label))
                        .size(TS.body)
                        .color(theme_ext::text_primary(&ctx.tokens)),
                )
                .padding([SP.xs, SP.m])
                .on_press(PanelMsg::ErcQuickFix(v.global_index))
                .style(crate::styles::menu_item(&ctx.tokens)),
                waive_chip(v, &ctx.tokens),
                super::erc_waivers::waive_chip(v, &ctx.tokens),
            ]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
            .boxed(),
        );
        if let Some(editor) = super::erc_waivers::waive_editor(v, ctx) {
            col = col.push(editor);
        }
    }

    col.push(waived_section(ctx))
        .push(super::erc_waivers::violation_waivers_section(ctx))
        .boxed()
}

/// Shown while the document has changed since the newest ERC run.
const STALE_RESULTS_NOTICE: &str =
    "Results may be out of date: the sheet changed since the last run. Run ERC again.";

/// Where a finding is, on one line: the sheet's file name and the point. The
/// full path is the row's tooltip, so a long path never wraps the row.
fn location_label(sheet_name: &str, x: f64, y: f64) -> String {
    format!("{sheet_name}  ({x:.2}, {y:.2})")
}

/// "Waive net" chip for a net-level finding on a named net; nothing for any
/// other row (a pin No-ERC marker is the waiver for those).
fn waive_chip<'a>(
    v: &ErcDiagnosticEntry,
    tokens: &'a signex_types::theme::ThemeTokens,
) -> Element<'a, PanelMsg> {
    let Some(net) = v.net.as_ref().filter(|n| !n.name.is_empty()) else {
        return Space::new().width(0).height(0).boxed();
    };
    row![
        Space::new().width(6).height(Length::Shrink),
        signex_widgets::button(
            text(raw("Waive net"))
                .size(TS.body)
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
    .boxed()
}

/// The project's net waivers, dimmed, each with a Restore button. Shown even
/// when every finding is waived, so a waiver is never invisible.
fn waived_section<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs).width(Length::Fill);
    if ctx.erc_waivers.is_empty() {
        return col.boxed();
    }
    col = col.push(separator(&ctx.tokens).boxed());
    col = col.push(
        text(raw(format!("{} waived", ctx.erc_waivers.len())))
            .size(TS.caption)
            .color(muted)
            .boxed(),
    );
    for w in &ctx.erc_waivers {
        let note = match w.hidden {
            0 => "matches no finding".to_string(),
            1 => "1 finding hidden".to_string(),
            n => format!("{n} findings hidden"),
        };
        col = col.push(
            row![
                text(raw(format!("{}  net {}  ({note})", w.rule_id, w.net)))
                    .size(TS.caption)
                    .color(muted),
                Space::new().width(Length::Fill).height(Length::Shrink),
                signex_widgets::button(
                    text(raw("Restore"))
                        .size(TS.body)
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
            .width(Length::Fill)
            .boxed(),
        );
    }
    col.boxed()
}

// ─── Messages Panel ───────────────────────────────────────────

/// Column widths of the Graphite Messages table; MESSAGE takes the rest.
const MSG_CLASS_W: f32 = 84.0;
const MSG_DOCUMENT_W: f32 = 150.0;
const MSG_SOURCE_W: f32 = 160.0;
const MSG_TIME_W: f32 = 84.0;

/// The Graphite Messages table: CLASS / DOCUMENT / SOURCE / MESSAGE / TIME
/// header, hairline rows, and a centred empty state. The log entries carry
/// no document or time, so those cells read as an honest dash.
fn view_messages_graphite<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = crate::styles::tokens::resolve(tokens);
    let (sunken, line, ink3, ink4) = (r.sunken, r.line_soft, r.ink3, r.ink4);
    let ink = theme_ext::text_primary(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let rule = move || -> Element<'a, PanelMsg> {
        container(Space::new())
            .height(Length::Fixed(1.0))
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            })
            .boxed()
    };
    let cell = |content: Element<'a, PanelMsg>, width: Length| -> Element<'a, PanelMsg> {
        container(content)
            .padding([0.0, 12.0])
            .width(width)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center)
            .clip(true)
            .boxed()
    };
    let caps = move |label: &'static str| -> Element<'a, PanelMsg> {
        text(raw(label)).size(TS.section).color(ink3).boxed()
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    col = col.push(
        container(
            row![
                cell(caps("CLASS"), Length::Fixed(MSG_CLASS_W)),
                cell(caps("DOCUMENT"), Length::Fixed(MSG_DOCUMENT_W)),
                cell(caps("SOURCE"), Length::Fixed(MSG_SOURCE_W)),
                cell(caps("MESSAGE"), Length::Fill),
                cell(caps("TIME"), Length::Fixed(MSG_TIME_W)),
            ]
            .width(Length::Fill),
        )
        .height(Length::Fixed(super::graphite::TABLE_ROW_HEIGHT))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        })
        .boxed(),
    );
    col = col.push(rule());

    if ctx.diagnostics.is_empty() {
        col = col.push(
            container(
                row![
                    text(raw("No runtime messages yet"))
                        .size(TS.body)
                        .color(ink3),
                    text(raw(
                        "Set RUST_LOG=debug or SIGNEX_LOG=debug for verbose output",
                    ))
                    .size(TS.caption)
                    .color(ink4),
                ]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center),
            )
            .padding([0.0, 12.0])
            .height(Length::Fixed(super::graphite::TABLE_ROW_HEIGHT))
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill)
            .clip(true)
            .boxed(),
        );
        col = col.push(rule());
        return container(col).width(Length::Fill).boxed();
    }

    let dash = move || -> Element<'a, PanelMsg> {
        text(raw("\u{2014}")).size(TS.body).color(ink4).boxed()
    };
    for entry in ctx.diagnostics.iter().rev() {
        col = col.push(
            container(
                row![
                    cell(level_pill(tokens, entry.level), Length::Fixed(MSG_CLASS_W)),
                    cell(dash(), Length::Fixed(MSG_DOCUMENT_W)),
                    cell(
                        text(raw(entry.code.as_str().to_string()))
                            .size(TS.body)
                            .font(mono)
                            .color(ink3)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .boxed(),
                        Length::Fixed(MSG_SOURCE_W),
                    ),
                    cell(
                        text(raw(entry.message.as_str().to_string()))
                            .size(TS.body)
                            .color(ink)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .boxed(),
                        Length::Fill,
                    ),
                    cell(dash(), Length::Fixed(MSG_TIME_W)),
                ]
                .width(Length::Fill),
            )
            .height(Length::Fixed(super::graphite::TABLE_ROW_HEIGHT))
            .boxed(),
        );
        col = col.push(rule());
    }
    container(col).width(Length::Fill).boxed()
}

pub fn view_messages<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    view_messages_graphite(ctx)
}

fn erc_severity_pill<'a>(
    tokens: &ThemeTokens,
    severity: ErcSeverityLite,
    label: &'static str,
) -> Element<'a, PanelMsg> {
    use crate::styles::PillTone;
    let tone = match severity {
        ErcSeverityLite::Error => PillTone::Err,
        ErcSeverityLite::Warning => PillTone::Warn,
        ErcSeverityLite::Info => PillTone::Info,
    };
    container(text(raw(label)).size(TS.caption))
        .padding([1.0, 7.0])
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

/// Severity pill of a Messages row (Graphite): error, warning, info tones.
fn level_pill<'a>(
    tokens: &ThemeTokens,
    level: crate::diagnostics::DiagnosticLevel,
) -> Element<'a, PanelMsg> {
    use crate::diagnostics::DiagnosticLevel as L;
    use crate::styles::PillTone;
    let tone = match level {
        L::Error => PillTone::Err,
        L::Warning => PillTone::Warn,
        L::Info => PillTone::Info,
        L::Debug | L::Trace => PillTone::Neutral,
    };
    container(text(raw(level.label().to_string())).size(TS.caption))
        .padding([1.0, 7.0])
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

#[cfg(test)]
mod location_tests {
    use super::location_label;

    #[test]
    fn a_finding_location_is_the_file_name_and_the_point_on_one_line() {
        let label = location_label("top.snxsch", 12.345, -6.0);
        assert_eq!(label, "top.snxsch  (12.35, -6.00)");
        assert!(!label.contains('/') && !label.contains('\\') && !label.contains('\n'));
    }
}
