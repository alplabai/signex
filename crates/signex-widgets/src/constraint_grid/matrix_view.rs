//! The class-to-class matrix view. Pure: the matrix's view state and the
//! caller's [`PairValues`] in, `Element<MatrixMessage>` out. Class names and
//! values are in the monospace face, values centred: a written value full
//! ink, a fallback faint
//! in parentheses with its unit. The mirror half is hatched and inert, and
//! the pinned cell carries the accent ring (mockup row 90).

use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::{self, Canvas, Path, Stroke};
use iced::widget::{Column, Row, Space, container, mouse_area};
use iced::{
    Background, Border, Color, Element, Font, Length, Padding, Point, Rectangle, Renderer, Size,
    Theme, Widget, font, mouse,
};
use signex_i18n::{MessageId, UserText, t};

use super::cell::{frame, length_input, reset_glyph, strong_font};
use super::matrix::{ClassMatrix, ClassPair, MatrixMessage, PairClearance, PairValues};
use super::style::GridStyle;
use crate::theme_ext::HAIRLINE;
use crate::{Label, LengthFormat, text, tooltip};

/// Widget id of the matrix's inline editor.
pub const MATRIX_EDITOR_ID: &str = "constraint-matrix-cell-editor";

/// Hatch period of the mirror half, in em: 6 px at the 12 px base.
const HATCH_EM: f32 = 0.5;
/// Hatch period of the legend's mirror swatch, in em: 4 px at 12 px.
const SWATCH_HATCH_EM: f32 = 1.0 / 3.0;

/// The matrix's own words, from the caller's message catalog.
#[derive(Debug, Clone)]
pub struct MatrixText<'a> {
    /// Tooltip of the clear glyph ("Clear: use the fallback").
    pub clear: Label<'a>,
    /// Tooltip of a fallback value ("No value of its own: the larger class
    /// clearance").
    pub fallback: Label<'a>,
    /// A fallback value as a cell shows it, `$value` the number and `$unit`
    /// the symbol of the format's unit: `({ $value } { $unit })`.
    pub fallback_value: MessageId,
}

/// What a cell shows for `clearance`: a written value as its number, a
/// fallback in [`MatrixText::fallback_value`] (`(0.15 mm)`) in the format's
/// unit, as the legend shows the two.
pub fn cell_text(
    clearance: PairClearance,
    format: LengthFormat<'_>,
    words: &MatrixText<'_>,
) -> Label<'static> {
    match clearance {
        PairClearance::Written(mm) => format.label(mm),
        PairClearance::Fallback(mm) => t!(
            format.i18n,
            words.fallback_value,
            "value" => format.number(mm),
            "unit" => format.unit.to_string()
        )
        .into(),
    }
}

/// Cell values are in the style's monospace face; a written one at medium
/// weight.
pub(super) fn value_font(is_written: bool, style: GridStyle) -> Font {
    let weight = if is_written {
        font::Weight::Medium
    } else {
        font::Weight::Normal
    };
    Font {
        weight,
        ..style.mono
    }
}

/// The diagonal hatch strokes over a `size` box, `step` apart: each runs
/// from lower left to upper right (`x + y` constant), clipped to the box.
pub(super) fn hatch_segments(size: Size, step: f32) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    if !step.is_finite() || step <= 0.0 || size.width <= 0.0 || size.height <= 0.0 {
        return out;
    }
    let mut c = step;
    while c < size.width + size.height {
        let start_x = (c - size.height).max(0.0);
        let end_x = c.min(size.width);
        out.push((
            Point::new(start_x, c - start_x),
            Point::new(end_x, c - end_x),
        ));
        c += step;
    }
    out
}

/// A box of diagonal hairlines in `ink`, `step` apart.
struct Hatch {
    ink: Color,
    step: f32,
}

impl<M> canvas::Program<M> for Hatch {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let path = Path::new(|b| {
            for (from, to) in hatch_segments(bounds.size(), self.step) {
                b.move_to(from);
                b.line_to(to);
            }
        });
        frame.stroke(
            &path,
            Stroke::default().with_color(self.ink).with_width(HAIRLINE),
        );
        vec![frame.into_geometry()]
    }
}

/// A `width` × `height` box hatched and framed in the grid's rule ink.
fn hatched<'a, M: 'a>(step: f32, width: f32, height: f32, style: GridStyle) -> Element<'a, M> {
    let hatch = Canvas::<Hatch, M>::new(Hatch {
        ink: style.line,
        step,
    })
    .width(width)
    .height(height);
    container(hatch)
        .width(width)
        .height(height)
        .style(move |_: &Theme| container::Style {
            border: Border {
                color: style.line,
                width: HAIRLINE,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The legend's mirror-half sample: a `side` square hatched as the mirror
/// cells are.
pub fn mirror_swatch<'a, M: 'a>(side: f32, style: GridStyle) -> Element<'a, M> {
    hatched(style.text_size * SWATCH_HATCH_EM, side, side, style)
}

/// Class names are in the style's monospace face at the heading weight.
pub(super) fn name_font(style: GridStyle) -> Font {
    Font {
        weight: strong_font().weight,
        ..style.mono
    }
}

fn name_cell<'a>(name: &'a str, width: f32, style: GridStyle) -> Element<'a, MatrixMessage> {
    let m = style.metrics();
    container(
        text(UserText::new(name))
            .size(style.text_size)
            .color(style.ink)
            .font(name_font(style))
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding(Padding::from([0.0, m.pad]))
    .width(width)
    .height(m.row_height)
    .align_y(Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(style.header_fill)),
        border: Border {
            color: style.line,
            width: HAIRLINE,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

fn inert_cell<'a>(style: GridStyle) -> Element<'a, MatrixMessage> {
    let m = style.metrics();
    hatched(
        style.text_size * HATCH_EM,
        m.cell_width,
        m.row_height,
        style,
    )
}

/// A closed cell's value, centred, pressable; a fallback carries the
/// fallback tooltip.
fn shown_value<'a>(
    clearance: PairClearance,
    pair: ClassPair,
    format: LengthFormat<'_>,
    words: &MatrixText<'a>,
    style: GridStyle,
) -> Element<'a, MatrixMessage> {
    let is_written = matches!(clearance, PairClearance::Written(_));
    let value = text(cell_text(clearance, format, words))
        .size(style.text_size)
        .font(value_font(is_written, style))
        .color(if is_written {
            style.ink
        } else {
            style.faint_ink
        })
        .wrapping(iced::widget::text::Wrapping::None);
    let pressable = mouse_area(
        container(value)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center),
    )
    .on_press(MatrixMessage::CellPressed(pair))
    .interaction(iced::mouse::Interaction::Text);
    if is_written {
        return pressable.boxed();
    }
    tooltip(
        pressable,
        text(words.fallback.clone()).size(style.text_size),
        tooltip::Position::Bottom,
    )
    .boxed()
}

fn live_cell<'a>(
    matrix: &'a ClassMatrix,
    values: &impl PairValues,
    pair: ClassPair,
    format: LengthFormat<'_>,
    words: &MatrixText<'a>,
    style: GridStyle,
) -> Element<'a, MatrixMessage> {
    let m = style.metrics();
    let Some(clearance) = matrix.clearance(values, pair) else {
        return inert_cell(style);
    };
    let is_pinned = matrix.pinned() == Some(pair);
    let edit = matrix.edit().filter(|e| e.pair == pair);
    let is_written = matches!(clearance, PairClearance::Written(_));

    let content: Element<'a, MatrixMessage> = if let Some(edit) = edit {
        let placeholder: Label<'a> = values.fallback(pair).map_or_else(
            || UserText::new(String::new()).into(),
            |mm| format.label(mm),
        );
        length_input(
            MATRIX_EDITOR_ID,
            placeholder,
            edit.text.as_str(),
            edit.is_invalid,
            MatrixMessage::EditTextChanged,
            MatrixMessage::EditSubmitted,
            style,
        )
    } else {
        shown_value(clearance, pair, format, words, style)
    };

    let mut line: Row<Element<'a, MatrixMessage>> = Row::new()
        .spacing(m.pad / 2.0)
        .align_y(Vertical::Center)
        .height(Length::Fill)
        .push(content);
    if is_pinned && is_written && edit.is_none() {
        line = line.push(reset_glyph(
            MatrixMessage::CellCleared(pair),
            words.clear.clone(),
            m,
            style,
        ));
    }
    container(line)
        .padding(Padding::from([0.0, m.pad]))
        .width(m.cell_width)
        .height(m.row_height)
        .style(frame(is_pinned, style))
        .boxed()
}

/// The class-to-class matrix: class names down and across, one clearance
/// per pair in the lower triangle. `values` is the caller's view of the
/// clearances; `format` its locale and display unit.
pub fn matrix_view<'a>(
    matrix: &'a ClassMatrix,
    values: &impl PairValues,
    format: LengthFormat<'_>,
    words: &MatrixText<'a>,
    style: GridStyle,
) -> Element<'a, MatrixMessage> {
    let m = style.metrics();
    let classes = matrix.classes();
    let name_width = m.cell_width;
    let mut header: Row<Element<'a, MatrixMessage>> =
        Row::new().push(Space::new().width(name_width).height(m.row_height).boxed());
    for class in classes {
        header = header.push(name_cell(&class.name, m.cell_width, style));
    }
    let mut table: Column<Element<'a, MatrixMessage>> = Column::new().push(header.boxed());
    for (row, class) in classes.iter().enumerate() {
        let mut line: Row<Element<'a, MatrixMessage>> =
            Row::new().push(name_cell(&class.name, name_width, style));
        for col in 0..classes.len() {
            line = line.push(if ClassPair::is_live(row, col) {
                live_cell(
                    matrix,
                    values,
                    ClassPair::new(row, col),
                    format,
                    words,
                    style,
                )
            } else {
                inert_cell(style)
            });
        }
        table = table.push(line.boxed());
    }
    table.boxed()
}
