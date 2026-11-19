//! The class-to-class matrix view. Pure: the matrix's view state and the
//! caller's [`PairValues`] in, `Element<MatrixMessage>` out. Written values
//! are full ink, fallbacks muted, the mirror half inert, and the pinned cell
//! carries the accent ring.

use iced::alignment::Vertical;
use iced::widget::{Column, Row, Space, container, mouse_area};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget};
use signex_i18n::UserText;

use super::cell::{frame, length_input, reset_glyph, strong_font};
use super::matrix::{ClassMatrix, ClassPair, MatrixMessage, PairClearance, PairValues};
use super::style::GridStyle;
use crate::theme_ext::HAIRLINE;
use crate::{Label, LengthFormat, text, tooltip};

/// Widget id of the matrix's inline editor.
pub const MATRIX_EDITOR_ID: &str = "constraint-matrix-cell-editor";

/// The matrix's own words, from the caller's message catalog.
#[derive(Debug, Clone)]
pub struct MatrixText<'a> {
    /// Tooltip of the clear glyph ("Clear: use the fallback").
    pub clear: Label<'a>,
    /// Tooltip of a fallback value ("No value of its own: the larger class
    /// clearance").
    pub fallback: Label<'a>,
}

fn name_cell<'a>(name: &'a str, width: f32, style: GridStyle) -> Element<'a, MatrixMessage> {
    let m = style.metrics();
    container(
        text(UserText::new(name))
            .size(style.text_size)
            .color(style.ink)
            .font(strong_font())
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
    container(Space::new())
        .width(m.cell_width)
        .height(m.row_height)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(style.inert_fill)),
            border: Border {
                color: style.line,
                width: HAIRLINE,
                ..Border::default()
            },
            ..container::Style::default()
        })
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
        let placeholder: Label<'a> = matrix.fallback(values, pair).map_or_else(
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
        let mut value = text(format.label(clearance.mm()))
            .size(style.text_size)
            .color(if is_written {
                style.ink
            } else {
                style.muted_ink
            })
            .wrapping(iced::widget::text::Wrapping::None);
        if is_written {
            value = value.font(strong_font());
        }
        let pressable = mouse_area(
            container(value)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(MatrixMessage::CellPressed(pair))
        .interaction(iced::mouse::Interaction::Text);
        if is_written {
            pressable.boxed()
        } else {
            tooltip(
                pressable,
                text(words.fallback.clone()).size(style.text_size),
                tooltip::Position::Bottom,
            )
            .boxed()
        }
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
