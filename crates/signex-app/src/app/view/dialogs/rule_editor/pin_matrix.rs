//! The right column of the rule editor: the pin-connection matrix as its
//! lower triangle of tinted severity chips, the legend and the caption.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_erc::{PinAxis, Severity};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::{MatrixCell, RuleEditor, TS, lower_triangle, section_heading};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, Spacing, pill_colors, ti};

const SP: Spacing = Spacing::DEFAULT;

// Pin-matrix geometry at the 12 px body, kept as ratios of the body size. A
// column label wider than its cell ("PowerOut") does not wrap; it overhangs
// into the cell gap.
const CELL_W: f32 = TS.body * 46.0 / 12.0;
const CELL_H: f32 = TS.body * 28.0 / 12.0;
const CELL_GAP: f32 = TS.body * 3.0 / 12.0;
const COL_HEADER_H: f32 = TS.body * 24.0 / 12.0;
/// Row labels sit right-aligned; wide enough for "PowerOut" at caption size.
const ROW_HEADER_W: f32 = TS.body * 60.0 / 12.0;
/// Width of the accent ring on a changed cell.
const RING: f32 = TS.body * 1.5 / 12.0;
/// Legend swatch: a small severity chip.
const SWATCH_W: f32 = TS.body * 18.0 / 12.0;
const SWATCH_H: f32 = TS.body * 16.0 / 12.0;
const SWATCH_RADIUS: f32 = TS.body * 4.0 / 12.0;
const LEGEND_DOT: f32 = TS.body * 6.0 / 12.0;
const LEGEND_RING: f32 = TS.body * 12.0 / 12.0;

/// Heading, matrix, legend and caption.
pub(super) fn view<M: Clone + 'static>(editor: &RuleEditor<'_, M>) -> Element<'static, M> {
    let tokens = editor.tokens;
    let i18n = editor.i18n;
    let ink3 = resolve(tokens).ink3;
    column![
        heading(i18n, tokens, editor.editable),
        matrix(editor),
        legend(i18n, tokens),
        text(t!(i18n, ids::DIALOG_RULES_MATRIX_CAPTION))
            .size(TS.caption)
            .color(ink3),
    ]
    .spacing(SP.m)
    .padding(Padding {
        top: SP.m,
        right: SP.l,
        bottom: SP.l,
        left: SP.l,
    })
    .width(Length::Fill)
    .boxed()
}

/// The section heading, with the click hint on its right while the cells
/// take edits (mockup row 84).
fn heading<M: 'static>(i18n: &I18n, tokens: &ThemeTokens, editable: bool) -> Element<'static, M> {
    let title = section_heading(i18n, ids::DIALOG_RULES_MATRIX_HEADING, tokens);
    if !editable {
        return title;
    }
    let faint = resolve(tokens).ink4;
    row![
        title,
        Space::new().width(Length::Fill).height(Length::Shrink),
        text(t!(i18n, ids::DIALOG_RULES_MATRIX_CYCLE_HINT))
            .size(TS.caption)
            .color(faint)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The glyph of a severity: its catalog letter, or a middle dot for Off.
fn severity_glyph(i18n: &I18n, severity: Severity) -> String {
    match severity {
        Severity::Error => t!(i18n, ids::DIALOG_RULES_ERROR_SHORT).into_string(),
        Severity::Warning => t!(i18n, ids::DIALOG_RULES_WARNING_SHORT).into_string(),
        Severity::Info => t!(i18n, ids::DIALOG_RULES_INFO_SHORT).into_string(),
        Severity::Off => "\u{00B7}".to_owned(),
    }
}

/// `(fill, ink)` of a severity chip: the soft status tones, the sunken fill
/// for Off.
fn severity_tone(tokens: &ThemeTokens, severity: Severity) -> (Color, Color) {
    match severity {
        Severity::Error => pill_colors(tokens, PillTone::Err),
        Severity::Warning => pill_colors(tokens, PillTone::Warn),
        Severity::Info => pill_colors(tokens, PillTone::Info),
        Severity::Off => {
            let r = resolve(tokens);
            (r.sunken, r.ink4)
        }
    }
}

/// A tinted severity chip of `width` x `height`, with an accent ring when
/// `ring` is set. Matrix cells and legend swatches share it.
fn severity_chip<M: 'static>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    severity: Severity,
    ring: bool,
    (width, height, radius): (f32, f32, f32),
) -> Element<'static, M> {
    let (fill, ink) = severity_tone(tokens, severity);
    let accent = ti(tokens.accent);
    let weight = if severity == Severity::Off {
        iced::font::Weight::Normal
    } else {
        iced::font::Weight::Semibold
    };
    let font = Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    };
    // raw: the letter is rendered from the catalog above (or a glyph).
    container(
        text(raw(severity_glyph(i18n, severity)))
            .size(TS.caption)
            .font(font)
            .color(ink),
    )
    .width(width)
    .height(height)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: if ring { RING } else { 0.0 },
            color: accent,
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// An axis label in the caption size and the section ink, on one line.
fn axis_label<M: 'static>(
    label: &str,
    ink: Color,
    (width, height): (f32, f32),
    (align_x, align_y): (iced::alignment::Horizontal, iced::alignment::Vertical),
) -> Element<'static, M> {
    // raw: the pin-axis names come from signex-erc (#2062).
    container(
        text(raw(label.to_string()))
            .size(TS.caption)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(width)
    .height(height)
    .align_x(align_x)
    .align_y(align_y)
    .boxed()
}

/// The lower triangle, with the axis names along the top and down the left.
/// Click a cell to cycle Error -> Warning -> Info -> Off -> baseline; the
/// mirrored cell follows.
fn matrix<M: Clone + 'static>(editor: &RuleEditor<'_, M>) -> Element<'static, M> {
    let tokens = editor.tokens;
    let ink3 = resolve(tokens).ink3;

    // Six axes; `PinAxis::of` documents how the 14 pin electrical types fold
    // onto them. The baseline grid lives in `signex_erc::pin_matrix`.
    let mut header: Row<Element<'static, M>> = Row::new()
        .spacing(CELL_GAP)
        .push(Space::new().width(ROW_HEADER_W).boxed());
    for axis in PinAxis::ALL {
        header = header.push(axis_label(
            axis.label(),
            ink3,
            (CELL_W, COL_HEADER_H),
            (
                iced::alignment::Horizontal::Center,
                iced::alignment::Vertical::Bottom,
            ),
        ));
    }

    let mut body: Column<Element<'static, M>> =
        Column::new().spacing(CELL_GAP).push(header.boxed());
    for (axis, cells) in PinAxis::ALL
        .iter()
        .zip(lower_triangle(editor.pin_matrix, editor.shipped))
    {
        let mut line: Row<Element<'static, M>> = Row::new()
            .spacing(CELL_GAP)
            .align_y(Alignment::Center)
            .push(axis_label(
                axis.label(),
                ink3,
                (ROW_HEADER_W, CELL_H),
                (
                    iced::alignment::Horizontal::Right,
                    iced::alignment::Vertical::Center,
                ),
            ));
        for cell in cells {
            line = line.push(matrix_cell(editor, cell));
        }
        body = body.push(line.boxed());
    }
    body.boxed()
}

fn matrix_cell<M: Clone + 'static>(
    editor: &RuleEditor<'_, M>,
    cell: MatrixCell,
) -> Element<'static, M> {
    let MatrixCell {
        row,
        col,
        severity,
        changed,
    } = cell;
    button(severity_chip(
        editor.i18n,
        editor.tokens,
        severity,
        changed,
        (CELL_W, CELL_H, Radii::GRAPHITE.small),
    ))
    .padding(0)
    .on_press_maybe(editor.editable.then(|| (editor.on_pin_cell)(row, col)))
    .style(|_: &Theme, _| button::Style::default())
    .boxed()
}

/// One legend entry: a swatch and its word.
fn legend_entry<M: 'static>(
    i18n: &I18n,
    ink: Color,
    swatch: Element<'static, M>,
    label: MessageId,
) -> Element<'static, M> {
    row![swatch, text(t!(i18n, label)).size(TS.caption).color(ink)]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

/// E Error, W Warning, I Info, · No report, the changed-cell ring and the
/// changed-rule dot.
fn legend<M: 'static>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'static, M> {
    let ink3 = resolve(tokens).ink3;
    let accent = ti(tokens.accent);
    let swatch = |severity: Severity| {
        severity_chip(
            i18n,
            tokens,
            severity,
            false,
            (SWATCH_W, SWATCH_H, SWATCH_RADIUS),
        )
    };
    let marker = |size: f32, ring: bool| -> Element<'static, M> {
        container(Space::new())
            .width(size)
            .height(size)
            .style(move |_: &Theme| container::Style {
                background: (!ring).then_some(Background::Color(accent)),
                border: Border {
                    width: if ring { RING } else { 0.0 },
                    color: accent,
                    radius: if ring { SWATCH_RADIUS } else { size / 2.0 }.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    };
    let entries = [
        (swatch(Severity::Error), ids::DIALOG_RULES_LEGEND_ERROR),
        (swatch(Severity::Warning), ids::DIALOG_RULES_LEGEND_WARNING),
        (swatch(Severity::Info), ids::DIALOG_RULES_LEGEND_INFO),
        (swatch(Severity::Off), ids::DIALOG_RULES_LEGEND_OFF),
        (marker(LEGEND_RING, true), ids::DIALOG_RULES_LEGEND_CHANGED),
        (
            marker(LEGEND_DOT, false),
            ids::DIALOG_RULES_LEGEND_CHANGED_RULE,
        ),
    ];
    let mut line: Row<Element<'static, M>> = Row::new().spacing(SP.l);
    for (swatch, label) in entries {
        line = line.push(legend_entry(i18n, ink3, swatch, label));
    }
    line.wrap().vertical_spacing(SP.s).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The matrix is sized in proportion to its text: the row labels hold
    /// the longest axis label ("PowerOut", about 4.8 caption heights), a cell
    /// is 46 x 28 at the 12 px body, and the lengths move with the type
    /// scale, not with a fixed pixel count.
    #[test]
    fn matrix_lengths_are_proportional_to_the_type_scale() {
        const { assert!(ROW_HEADER_W >= 4.8 * TS.caption) };
        assert!((CELL_W / TS.body - 46.0 / 12.0).abs() < 1e-4);
        assert!((CELL_H / TS.body - 28.0 / 12.0).abs() < 1e-4);
        assert!((COL_HEADER_H / TS.body - 24.0 / 12.0).abs() < 1e-4);
        assert!((ROW_HEADER_W / TS.body - 60.0 / 12.0).abs() < 1e-4);
    }

    /// The pane fits the six-column triangle: row labels plus six cells and
    /// their gaps, inside the pane's side padding.
    #[test]
    fn the_triangle_fits_the_matrix_pane() {
        let grid = ROW_HEADER_W + 6.0 * (CELL_W + CELL_GAP);
        assert!(
            grid + 2.0 * SP.l <= super::super::MATRIX_PANE_W + 1e-3,
            "{grid}"
        );
    }

    /// Mockup row 84: an editable matrix says on the heading's right that a
    /// click cycles a cell, inside the pane; the inert shipped grid does not.
    #[test]
    fn the_heading_carries_the_cycle_hint_only_while_editable() {
        let (app, _boot) = crate::app::Signex::new();
        let tokens = app.document_state.panel_ctx.tokens.clone();
        let i18n = I18n::en_us();
        let inner = super::super::MATRIX_PANE_W - 2.0 * SP.l;
        let window = iced::Size::new(inner, 200.0);
        let laid_out = |editable| {
            crate::layout_probe::lay_out_texts(heading::<()>(&i18n, &tokens, editable), window)
        };
        let shown = laid_out(true);
        let texts: Vec<String> = shown
            .iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert_eq!(
            texts,
            ["PIN CONNECTION MATRIX", "Click a cell to cycle severity."]
        );
        let (title, hint) = (&shown[0].bounds, &shown[1].bounds);
        assert!(hint.x >= title.x + title.width, "{shown:#?}");
        assert!(hint.x + hint.width <= inner + 0.5, "{shown:#?}");
        assert_eq!(laid_out(false).len(), 1);
    }
}
