//! One value cell: its shown value, inline editor, reset glyph and source
//! tooltip. Pure: reads the grid, emits [`GridMessage`]s.

use std::sync::OnceLock;

use iced::alignment::Vertical;
use iced::widget::{Row, Space, column, container, mouse_area, pick_list, svg};
use iced::{Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, LocalNumber, UserText};

use super::model::{CellRef, CellValue, ColumnKind, ConstraintGrid, GridColumn, ValueSource};
use super::style::{GridMetrics, GridStyle, GridText};
use super::update::GridMessage;
use crate::{Label, button, checkbox, text, text_input, tooltip};

/// Widget id of the inline length editor; focus it with
/// `iced::widget::operation::focus(EDITOR_ID)` after a cell press.
pub const EDITOR_ID: &str = "constraint-grid-cell-editor";

const RESET_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"><path d="M3 4.5A3.6 3.6 0 1 1 2.6 7.4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/><path d="M2.4 2.2V4.8H5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

fn reset_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(RESET_ICON))
        .clone()
}

/// The semibold face for a value set on its own row.
pub(super) fn strong_font() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

/// A length as the locale writes it, rounded to 0.1 µm.
pub(super) fn length_label(i18n: &I18n, mm: f64) -> Label<'static> {
    let rounded = (mm * 1e4).round() / 1e4;
    UserText::new(i18n.number(LocalNumber(rounded))).into()
}

fn value_label<'a>(column: &'a GridColumn, value: CellValue, i18n: &I18n) -> Label<'a> {
    match (&column.kind, value) {
        (ColumnKind::Choice(options), CellValue::Choice(i)) => options
            .get(i)
            .cloned()
            .unwrap_or_else(|| UserText::new(String::new()).into()),
        (_, CellValue::Length(mm)) => length_label(i18n, mm),
        _ => UserText::new(String::new()).into(),
    }
}

/// The tooltip naming where the cell's value comes from.
fn source_tip<'a>(
    grid: &'a ConstraintGrid,
    source: ValueSource,
    words: &GridText<'a>,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let size = style.text_size;
    let body: Element<'a, GridMessage> = match source {
        ValueSource::Own => text(words.set_here.clone()).size(size).boxed(),
        ValueSource::Default => text(words.built_in.clone()).size(size).boxed(),
        ValueSource::Inherited(from) => {
            let name = grid.row(from).map_or("", |r| r.name.as_str());
            column![
                text(words.inherited_from.clone())
                    .size(size)
                    .color(style.muted_ink),
                text(UserText::new(name)).size(size).font(strong_font()),
            ]
            .boxed()
        }
    };
    container(body)
        .padding(style.metrics().pad)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(style.header_fill)),
            text_color: Some(style.ink),
            border: Border {
                color: style.line,
                width: 1.0,
                radius: (style.text_size * 0.3).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

pub(super) fn frame(is_selected: bool, style: GridStyle) -> impl Fn(&Theme) -> container::Style {
    move |_: &Theme| container::Style {
        background: is_selected.then_some(Background::Color(style.accent_soft)),
        border: Border {
            color: if is_selected {
                style.accent
            } else {
                style.line
            },
            width: if is_selected { 1.5 } else { 0.5 },
            radius: 0.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The ↺ glyph that drops a written value, with its tooltip.
pub(super) fn reset_glyph<'a, M: Clone + 'a>(
    on_press: M,
    tip: Label<'a>,
    m: GridMetrics,
    style: GridStyle,
) -> Element<'a, M> {
    let icon = svg(reset_handle())
        .width(m.glyph)
        .height(m.glyph)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(style.muted_ink),
            ..svg::Style::default()
        });
    let press = button(icon)
        .padding(0)
        .style(|_: &Theme, _| button::Style::default())
        .on_press(on_press);
    tooltip(
        press,
        text(tip).size(style.text_size),
        tooltip::Position::Bottom,
    )
    .boxed()
}

fn length_editor<'a>(
    grid: &'a ConstraintGrid,
    cell: CellRef,
    i18n: &I18n,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let edit = grid.edit();
    let typed = edit.map_or("", |e| e.text.as_str());
    let is_invalid = edit.is_some_and(|e| e.is_invalid);
    // The placeholder is what the row inherits once its own value is gone.
    let placeholder: Label<'a> = match grid.inherited(cell).map(|r| r.value) {
        Some(CellValue::Length(mm)) => length_label(i18n, mm),
        _ => UserText::new(String::new()).into(),
    };
    length_input(
        EDITOR_ID,
        placeholder,
        typed,
        is_invalid,
        GridMessage::EditTextChanged,
        GridMessage::EditSubmitted,
        style,
    )
}

/// The inline length editor, shared by the grid and the matrix.
pub(super) fn length_input<'a, M: Clone + 'a>(
    id: &'static str,
    placeholder: Label<'a>,
    typed: &'a str,
    is_invalid: bool,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: M,
    style: GridStyle,
) -> Element<'a, M> {
    let m = style.metrics();
    text_input(placeholder, typed)
        .id(id)
        .on_input(on_input)
        .on_submit(on_submit)
        .size(style.text_size)
        .padding(Padding::from([0.0, m.pad]))
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(style.field_fill),
            border: Border {
                color: if is_invalid {
                    style.error
                } else {
                    style.accent
                },
                width: 1.0,
                radius: 0.0.into(),
                ..Border::default()
            },
            placeholder: style.muted_ink,
            value: style.ink,
            selection: style.accent_soft,
        })
        .boxed()
}

fn choice_editor<'a>(
    column: &'a GridColumn,
    cell: CellRef,
    current: Option<usize>,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let ColumnKind::Choice(options) = &column.kind else {
        return Space::new().boxed();
    };
    let indices: Vec<usize> = (0..options.len()).collect();
    pick_list(current, indices, move |i: &usize| {
        options
            .get(*i)
            .map_or_else(String::new, |l| l.as_str().to_owned())
    })
    .on_select(move |i| GridMessage::ChoicePicked(cell, i))
    .on_close(GridMessage::EditCancelled)
    .text_size(style.text_size)
    .width(Length::Fill)
    .boxed()
}

/// A flag cell's checkbox, inked strong or muted like a value.
fn flag_box<'a>(cell: CellRef, on: bool, ink: Color, style: GridStyle) -> Element<'a, GridMessage> {
    checkbox(on)
        .size(style.text_size)
        .on_toggle(move |v| GridMessage::FlagToggled(cell, v))
        .style(move |_: &Theme, _| iced::widget::checkbox::Style {
            background: Background::Color(if on { ink } else { Color::TRANSPARENT }),
            icon_color: style.field_fill,
            border: Border {
                color: ink,
                width: 1.0,
                radius: (style.text_size * 0.2).into(),
                ..Border::default()
            },
            text_color: None,
        })
        .boxed()
}

/// One value cell of the grid.
pub(super) fn view<'a>(
    grid: &'a ConstraintGrid,
    column: &'a GridColumn,
    cell: CellRef,
    i18n: &I18n,
    words: &GridText<'a>,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let m = style.metrics();
    let Some(resolved) = grid.resolve(cell) else {
        return Space::new()
            .width(m.cell_width)
            .height(m.row_height)
            .boxed();
    };
    let is_selected = grid.selected() == Some(cell);
    let is_editing = grid.edit().is_some_and(|e| e.cell == cell);
    let ink = if resolved.is_own() {
        style.ink
    } else {
        style.muted_ink
    };

    let content: Element<'a, GridMessage> = match (&column.kind, resolved.value) {
        (ColumnKind::Length, _) if is_editing => length_editor(grid, cell, i18n, style),
        (ColumnKind::Choice(_), CellValue::Choice(i)) if is_editing => {
            choice_editor(column, cell, Some(i), style)
        }
        (ColumnKind::Flag, CellValue::Flag(on)) => flag_box(cell, on, ink, style),
        (_, value) => {
            let mut label = text(value_label(column, value, i18n))
                .size(style.text_size)
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None);
            if resolved.is_own() {
                label = label.font(strong_font());
            }
            mouse_area(
                container(label)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_y(Vertical::Center),
            )
            .on_press(GridMessage::CellPressed(cell))
            .interaction(iced::mouse::Interaction::Text)
            .boxed()
        }
    };

    let mut line: Row<Element<'a, GridMessage>> = Row::new()
        .spacing(m.pad / 2.0)
        .align_y(Vertical::Center)
        .height(Length::Fill)
        .push(content);
    if resolved.is_own() && !is_editing {
        line = line.push(reset_glyph(
            GridMessage::CellReset(cell),
            words.reset.clone(),
            m,
            style,
        ));
    }
    let framed = container(line)
        .padding(Padding::from([0.0, m.pad]))
        .width(m.cell_width)
        .height(m.row_height)
        .style(frame(is_selected, style));
    tooltip(
        framed,
        source_tip(grid, resolved.source, words, style),
        tooltip::Position::Bottom,
    )
    .gap(m.pad / 2.0)
    .boxed()
}
