//! The ERC rule editor: a severity grid (one row per rule) beside the
//! pin-connection matrix.
//!
//! One widget for every place that edits a rule set: the ERC dialog (the
//! project's rules, or the machine's) and Preferences ▸ Rule Profiles (a
//! library profile). It is generic over the message type, so each caller maps
//! a click to its own message and none of them forks the layout (#1608).

use iced::widget::{Column, Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};

use signex_erc::{PinAxis, RuleKind, Severity};

use crate::styles::{TypeScale, chrome_px};

const TS: TypeScale = TypeScale::DEFAULT;

/// Pin-matrix geometry. The cell is wide enough for the longest axis label
/// ("PowerOut") at caption size, and every length follows the base size.
const CELL_W: f32 = chrome_px(52.0);
const CELL_H: f32 = chrome_px(30.0);
const COL_HEADER_H: f32 = chrome_px(24.0);
const ROW_HEADER_W: f32 = chrome_px(80.0);

/// What the editor shows and how a click reaches the caller.
pub(crate) struct RuleEditor<'a, M> {
    pub tokens: &'a signex_types::theme::ThemeTokens,
    /// Per-rule severity overrides; a rule not listed shows its default.
    pub severity: &'a std::collections::HashMap<RuleKind, Severity>,
    /// Pin-matrix cells that differ from the factory matrix.
    pub pin_matrix: &'a std::collections::HashMap<(u8, u8), Severity>,
    /// `false` draws the same grid with every cell inert (the shipped
    /// profile).
    pub editable: bool,
    pub on_severity: fn(RuleKind, Severity) -> M,
    pub on_pin_cell: fn(u8, u8) -> M,
}

/// The two-column editor body.
pub(crate) fn rule_editor<M: Clone + 'static>(editor: RuleEditor<'_, M>) -> Element<'static, M> {
    let RuleEditor {
        tokens,
        severity,
        pin_matrix,
        editable,
        on_severity,
        on_pin_cell,
    } = editor;
    let text_c = crate::styles::ti(tokens.text);
    let text_muted = crate::styles::ti(tokens.text_secondary);
    let border_c = crate::styles::ti(tokens.border);

    // Per-rule severity grid: one row per built-in rule x 4 severities.
    let mut rule_rows: Column<Element<'static, M>> = column![
        row![
            text("Rule")
                .size(TS.caption)
                .color(text_muted)
                .width(Length::FillPortion(3)),
            text("Severity")
                .size(TS.caption)
                .color(text_muted)
                .width(Length::FillPortion(2)),
        ]
        .padding([4.0, 8.0]),
    ]
    .spacing(0);
    for rule in &RuleKind::ALL {
        let current = severity
            .get(rule)
            .copied()
            .unwrap_or_else(|| rule.default_severity());
        let label = if rule.is_on_off_only() {
            format!("{} (on/off; severity per matrix cell)", rule.label())
        } else {
            rule.label().to_string()
        };
        rule_rows = rule_rows.push(
            row![
                text(label)
                    .size(TS.body)
                    .color(text_c)
                    .width(Length::FillPortion(3)),
                severity_segmented(
                    *rule,
                    current,
                    border_c,
                    text_muted,
                    crate::styles::tokens::resolve(tokens).field,
                    editable,
                    on_severity,
                ),
            ]
            .align_y(iced::Alignment::Center)
            .padding([2.0, 8.0])
            .boxed(),
        );
    }

    let framed = move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border_c,
            ..Border::default()
        },
        ..container::Style::default()
    };
    let rules_pane = column![
        text("Severity per rule").size(TS.body).color(text_c),
        text("Click a cell to change.")
            .size(TS.caption)
            .color(text_muted),
        Space::new().height(8),
        container(rule_rows)
            .padding(4.0)
            .width(Length::Fill)
            .style(framed),
    ]
    .spacing(2)
    .width(Length::FillPortion(3));

    let matrix_pane = column![
        text("Pin Connection Matrix").size(TS.body).color(text_c),
        text("Click a cell to cycle severity.")
            .size(TS.caption)
            .color(text_muted),
        Space::new().height(8),
        container(pin_matrix_view(tokens, pin_matrix, editable, on_pin_cell))
            .padding(8.0)
            .width(Length::Fill)
            .style(framed),
    ]
    .spacing(2)
    .width(Length::FillPortion(2));

    row![rules_pane, Space::new().width(16), matrix_pane]
        .align_y(iced::Alignment::Start)
        .boxed()
}

fn severity_segmented<M: Clone + 'static>(
    rule: RuleKind,
    current: Severity,
    border: Color,
    text_muted: Color,
    unselected: Color,
    editable: bool,
    on_severity: fn(RuleKind, Severity) -> M,
) -> Element<'static, M> {
    let seg = |label: &'static str, value: Severity, color: Color| -> Element<'static, M> {
        let selected = if rule.is_on_off_only() && value != Severity::Off {
            current != Severity::Off
        } else {
            value == current
        };
        let bg = if selected { color } else { unselected };
        let fg = if selected { Color::WHITE } else { text_muted };
        button(container(text(label.to_string()).size(TS.caption).color(fg)).padding([3.0, 8.0]))
            .on_press_maybe(editable.then(|| on_severity(rule, value)))
            .style(move |_: &Theme, _| button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                    ..Border::default()
                },
                text_color: fg,
                ..button::Style::default()
            })
            .boxed()
    };
    let off = seg("Off", Severity::Off, Color::from_rgb(0.35, 0.36, 0.42));
    if rule.is_on_off_only() {
        // Severity comes from the pin matrix per cell; the row only gates it.
        return row![
            seg(
                "On",
                rule.default_severity(),
                Color::from_rgb(0.30, 0.60, 0.95)
            ),
            off
        ]
        .spacing(2)
        .boxed();
    }
    row![
        seg("E", Severity::Error, Color::from_rgb(0.85, 0.25, 0.25)),
        seg("W", Severity::Warning, Color::from_rgb(0.95, 0.70, 0.15)),
        seg("I", Severity::Info, Color::from_rgb(0.30, 0.60, 0.95)),
        off,
    ]
    .spacing(2)
    .boxed()
}

/// Pin-connection matrix. Click a cell to cycle Error -> Warning -> Info ->
/// Off -> baseline. Currently 6x6 primary pin types; the full 12-type grid
/// lands when the rule engine learns the remaining variants (Open Collector,
/// Open Emitter, HiZ, ...).
fn pin_matrix_view<M: Clone + 'static>(
    tokens: &signex_types::theme::ThemeTokens,
    overrides: &std::collections::HashMap<(u8, u8), Severity>,
    editable: bool,
    on_pin_cell: fn(u8, u8) -> M,
) -> Element<'static, M> {
    let text_c = crate::styles::ti(tokens.text);
    let text_muted = crate::styles::ti(tokens.text_secondary);
    let border = crate::styles::ti(tokens.border);

    // Six axes; `PinAxis::of` documents how the 14 pin electrical types fold
    // onto them. The baseline grid lives in `signex_erc::pin_matrix`.
    let cell = |r: u8, c: u8, sev: Severity| -> Element<'static, M> {
        let (bg, ch) = match sev {
            Severity::Error => (Color::from_rgba(0.85, 0.25, 0.25, 0.55), "E"),
            Severity::Warning => (Color::from_rgba(0.95, 0.70, 0.15, 0.55), "W"),
            Severity::Info => (Color::from_rgba(0.30, 0.55, 0.85, 0.55), "I"),
            Severity::Off => (Color::from_rgba(0.25, 0.70, 0.30, 0.35), "\u{00B7}"),
        };
        button(
            container(text(ch.to_string()).size(TS.label).color(text_c))
                .width(CELL_W)
                .height(CELL_H)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .on_press_maybe(editable.then(|| on_pin_cell(r, c)))
        .padding(0)
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: border,
                ..Border::default()
            },
            text_color: text_c,
            ..button::Style::default()
        })
        .boxed()
    };

    let header_label = |label: &str| -> Element<'static, M> {
        container(text(label.to_string()).size(TS.caption).color(text_muted))
            .width(ROW_HEADER_W)
            .height(CELL_H)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center)
            .padding([0.0, 6.0])
            .boxed()
    };
    let col_label = |label: &str| -> Element<'static, M> {
        container(text(label.to_string()).size(TS.caption).color(text_muted))
            .width(CELL_W)
            .height(COL_HEADER_H)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .boxed()
    };

    let mut header: iced::widget::Row<Element<'static, M>> =
        row![container(Space::new().width(ROW_HEADER_W)).width(ROW_HEADER_W)].spacing(0);
    for axis in PinAxis::ALL {
        header = header.push(col_label(axis.label()));
    }

    let mut body: Column<Element<'static, M>> = column![header].spacing(0);
    for (r, row_axis) in PinAxis::ALL.iter().enumerate() {
        let mut rr: iced::widget::Row<Element<'static, M>> =
            row![header_label(row_axis.label())].spacing(0);
        for c in 0..PinAxis::ALL.len() {
            let sev = overrides
                .get(&(r as u8, c as u8))
                .copied()
                .unwrap_or_else(|| signex_erc::pin_matrix::baseline_cell(r as u8, c as u8));
            rr = rr.push(cell(r as u8, c as u8, sev));
        }
        body = body.push(rr.boxed());
    }

    container(body).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The matrix is sized in proportion to its text: a cell holds the
    /// longest axis label ("PowerOut", about 4.8 caption heights) and the
    /// lengths move with the type scale, not with a fixed pixel count.
    #[test]
    fn matrix_lengths_are_proportional_to_the_type_scale() {
        const { assert!(CELL_W >= 4.8 * TS.caption) };
        assert!((CELL_W / TS.body - 52.0 / 11.0).abs() < 1e-4);
        assert!((COL_HEADER_H / TS.body - 24.0 / 11.0).abs() < 1e-4);
        assert!((ROW_HEADER_W / TS.body - 80.0 / 11.0).abs() < 1e-4);
    }
}
