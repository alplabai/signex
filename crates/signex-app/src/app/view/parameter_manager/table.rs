//! The Parameter Manager table. The header row stays on top and the
//! Reference column stays on the left: each is its own scrollable without
//! a scrollbar, and `TableScrolled` keeps them at the body's offset.
//!
//! The constraint grid (`signex_widgets::constraint_grid`) is not used for
//! the cells: its values are typed lengths, choices and flags over an
//! inheriting row hierarchy, and these are free text on flat rows. Its
//! colours and metrics (`GridStyle`, `GridMetrics`) are reused, so both
//! tables look alike.

use std::path::Path;

use iced::alignment::Vertical;
use iced::widget::scrollable::{self, Scroll, Scrollbar, Source};
use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::Symbol;
use signex_widgets::constraint_grid::{GridMetrics, GridStyle};
use signex_widgets::theme_ext::HAIRLINE;
use signex_widgets::{button, raw, text, text_input, tooltip};

use crate::app::contracts::ParameterTablePane;
use crate::app::handlers::parameter_manager::{
    BODY_SCROLL_ID, HEADER_SCROLL_ID, REFERENCE_SCROLL_ID,
};
use crate::app::state::ParameterCell;
use crate::app::{Message, ParameterManagerMsg, Signex};
use crate::styles::{self, TypeScale};

use super::PartRow;

const TS: TypeScale = TypeScale::DEFAULT;

/// Average glyph advance in em, to size a column to its longest text.
const GLYPH_ADVANCE_EM: f32 = 0.62;
/// `(min, max)` width in em of the Sheet column (All sheets only).
const SHEET_EM: (f32, f32) = (7.0, 14.0);
/// Smallest parameter column, in em of the body text (140 px by default).
const PARAM_MIN_EM: f32 = 140.0 / 12.0;
/// Room for the header's remove button beside the parameter name, in em.
const REMOVE_ROOM_EM: f32 = 1.75;
/// `(min, max)` width in em of Reference, Value and Footprint.
const REFERENCE_EM: (f32, f32) = (6.0, 9.0);
const VALUE_EM: (f32, f32) = (7.0, 12.0);
const FOOTPRINT_EM: (f32, f32) = (8.0, 16.0);
/// Width of the changed-cell mark (2 px).
const CHANGED_MARK: f32 = 2.0 * HAIRLINE;

/// Column widths in px.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ColumnWidths {
    pub reference: f32,
    pub value: f32,
    pub footprint: f32,
    pub params: Vec<f32>,
}

/// Width of a text of `chars` characters at `size`, with cell padding.
fn text_width(chars: usize, size: f32, pad: f32) -> f32 {
    (chars as f32 * size * GLYPH_ADVANCE_EM + 2.0 * pad).ceil()
}

/// The column widths for a table `width` px wide. `fixed` holds the longest
/// text (header included) of Reference, Value and Footprint, `params` that
/// of each parameter column. A parameter column is as wide as its longest
/// value, so a full part number fits, and never under its minimum; the
/// parameter columns share whatever width is left.
pub(super) fn column_widths(
    size: f32,
    pad: f32,
    width: f32,
    fixed: [usize; 3],
    params: &[usize],
) -> ColumnWidths {
    let clamp = |chars: usize, (min, max): (f32, f32)| {
        text_width(chars, size, pad).clamp(min * size, max * size)
    };
    let reference = clamp(fixed[0], REFERENCE_EM);
    let value = clamp(fixed[1], VALUE_EM);
    let footprint = clamp(fixed[2], FOOTPRINT_EM);
    let mut widths: Vec<f32> = params
        .iter()
        .map(|&chars| {
            (text_width(chars, size, pad) + REMOVE_ROOM_EM * size).max(PARAM_MIN_EM * size)
        })
        .collect();
    let used = reference + value + footprint + HAIRLINE + widths.iter().sum::<f32>();
    let free = width - used;
    if free > 0.0 && !widths.is_empty() {
        let share = free / widths.len() as f32;
        widths.iter_mut().for_each(|w| *w += share);
    }
    ColumnWidths {
        reference,
        value,
        footprint,
        params: widths,
    }
}

/// A one-pixel rule in `color`, `length` long; across when `horizontal`.
pub(super) fn rule<'a>(color: Color, length: Length, horizontal: bool) -> Element<'a, Message> {
    let (w, h) = if horizontal {
        (length, Length::Fixed(HAIRLINE))
    } else {
        (Length::Fixed(HAIRLINE), length)
    };
    container(Space::new())
        .width(w)
        .height(h)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// The message of a pane's scroll, `noop` for a scroll another pane caused
/// (it would bounce back otherwise).
fn scrolled(pane: ParameterTablePane, scroll: Scroll) -> Message {
    if matches!(scroll.source, Source::Operation) {
        return Message::noop();
    }
    let offset = scroll.viewport.absolute_offset();
    Message::ParameterManager(ParameterManagerMsg::TableScrolled {
        pane,
        x: offset.x,
        y: offset.y,
    })
}

/// A cell that shows `value` in `font` and cannot be edited.
fn read_only_cell<'a>(
    value: &str,
    width: f32,
    ink: Color,
    font: Font,
    m: GridMetrics,
) -> Element<'a, Message> {
    container(
        text(UserText::new(value.to_string()))
            .size(TS.body)
            .font(font)
            .color(ink)
            .wrapping(Wrapping::None),
    )
    .padding([0.0, m.pad])
    .width(Length::Fixed(width))
    .height(Length::Fixed(m.row_height))
    .align_y(Vertical::Center)
    .clip(true)
    .boxed()
}

/// The mono face of the cell values (mockup row 70).
fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The inks of the reference, value and footprint cells: the reference in
/// full ink, the value and footprint muted (mockup row 70).
fn fixed_cell_inks(style: GridStyle) -> [Color; 3] {
    [style.ink, style.muted_ink, style.muted_ink]
}

/// The face of a reference: mono at medium weight.
fn reference_font() -> Font {
    Font {
        weight: iced::font::Weight::Medium,
        ..mono()
    }
}

/// The face of a parameter cell: mono for a value, the italic UI face of
/// `family` for an empty cell's "Not set".
fn param_cell_font(value: &str, family: &str) -> Font {
    if value.is_empty() {
        not_set_font(family)
    } else {
        mono()
    }
}

/// A column title as the header shows it: upper-cased for the locale, at
/// the section size and medium weight, in the muted ink (mockup row 70).
fn header_text<'a>(words: &str, i18n: &I18n, style: GridStyle) -> Element<'a, Message> {
    // The title is catalog text or a parameter name, already rendered; it
    // goes back to the widget as finished text.
    text(UserText::new(i18n.upper_for_display(words)))
        .size(TS.section)
        .font(styles::medium_font())
        .color(style.muted_ink)
        .wrapping(Wrapping::None)
        .boxed()
}

/// A header caption cell.
fn header_cell<'a>(words: &str, i18n: &I18n, width: f32, style: GridStyle) -> Element<'a, Message> {
    container(header_text(words, i18n, style))
        .padding([0.0, style.metrics().pad])
        .width(Length::Fixed(width))
        .height(Length::Fixed(style.metrics().row_height))
        .align_y(Vertical::Center)
        .clip(true)
        .boxed()
}

/// The fill of a row: the accent tint while one of its cells has the
/// focus (the row being edited), else none.
fn row_fill(is_editing: bool, style: GridStyle) -> Color {
    if is_editing {
        style.accent_soft
    } else {
        Color::TRANSPARENT
    }
}

/// `content` on the fill `color`.
fn filled<'a>(content: Element<'a, Message>, color: Color) -> Element<'a, Message> {
    container(content)
        .style(move |_: &Theme| container::Style {
            background: (color.a > 0.0).then_some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// A text input that reads as text until it is hovered or focused. The
/// open cell (`is_open`, the model's focused cell) is drawn as an active
/// input whatever the widget reports (#2900).
fn inline_cell_style(
    style: GridStyle,
    is_open: bool,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_: &Theme, status| {
        let frame = |line: Color| Border {
            width: HAIRLINE,
            color: line,
            radius: styles::Radii::GRAPHITE.control.into(),
            ..Border::default()
        };
        let (background, border) = match status {
            _ if is_open => (Background::Color(style.field_fill), frame(style.accent)),
            text_input::Status::Focused { .. } => {
                (Background::Color(style.field_fill), frame(style.accent))
            }
            text_input::Status::Hovered => (Background::Color(style.field_fill), frame(style.line)),
            text_input::Status::Active | text_input::Status::Disabled => {
                (Background::Color(Color::TRANSPARENT), Border::default())
            }
        };
        text_input::Style {
            background,
            border,
            placeholder: style.muted_ink,
            value: style.ink,
            selection: Color {
                a: 0.4,
                ..style.accent
            },
        }
    }
}

/// The italic UI face of an empty cell's "Not set".
fn not_set_font(family: &str) -> iced::Font {
    iced::Font {
        style: iced::font::Style::Italic,
        ..crate::fonts::iced_font_for_family(family)
    }
}

/// One header cell of a parameter column; its remove button shows while
/// the pointer is over it.
fn param_header<'a>(
    app: &'a Signex,
    key: &str,
    width: f32,
    style: GridStyle,
) -> Element<'a, Message> {
    let i18n = &app.ui_state.i18n;
    let tokens = &app.document_state.panel_ctx.tokens;
    let m = style.metrics();
    let is_hovered = app.ui_state.parameter_table.hovered_column.as_deref() == Some(key);
    let mut content = row![container(header_text(key, i18n, style)).width(Length::Fill)]
        .align_y(Alignment::Center);
    if is_hovered {
        let (ink, hot) = (style.ink, style.accent_soft);
        // raw: a remove glyph, not language.
        let remove = button(text(raw("×")).size(TS.caption).color(ink))
            .padding([0.0, m.pad / 2.0])
            .style(move |_: &Theme, status| iced::widget::button::Style {
                background: matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                )
                .then_some(Background::Color(hot)),
                text_color: ink,
                border: Border {
                    radius: styles::Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            })
            .on_press(Message::ParameterManager(
                ParameterManagerMsg::ParameterRemoved {
                    key: key.to_string(),
                },
            ));
        let tip = container(
            text(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_REMOVE_COLUMN))
                .size(TS.caption)
                .color(style.ink),
        )
        .padding([styles::Spacing::DEFAULT.xs, styles::Spacing::DEFAULT.m])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            styles::ti(tokens.hover),
            styles::ti(tokens.border),
        ));
        content = content.push(
            tooltip(remove, tip, iced::widget::tooltip::Position::Bottom)
                .gap(styles::Spacing::DEFAULT.xs)
                .boxed(),
        );
    }
    let hover = |is_hovered| {
        Message::ParameterManager(ParameterManagerMsg::ColumnHeaderHovered {
            key: key.to_string(),
            is_hovered,
        })
    };
    mouse_area(
        container(content)
            .padding([0.0, m.pad])
            .width(Length::Fixed(width))
            .height(Length::Fixed(m.row_height))
            .align_y(Vertical::Center),
    )
    .on_enter(hover(true))
    .on_exit(hover(false))
    .boxed()
}

/// One editable parameter cell, with the changed mark on its left.
fn param_cell<'a>(
    app: &'a Signex,
    sheet: &Path,
    sym: &'a Symbol,
    key: &str,
    width: f32,
    style: GridStyle,
) -> Element<'a, Message> {
    let m = style.metrics();
    let shown: &str = sym.fields.get(key).map_or("", String::as_str);
    let cell = ParameterCell {
        sheet: Some(sheet.to_path_buf()),
        symbol: sym.uuid,
        key: key.to_string(),
    };
    let table = &app.ui_state.parameter_table;
    let is_changed = table
        .session
        .original(&cell)
        .is_some_and(|before| before.as_deref() != sym.fields.get(key).map(String::as_str));
    let is_open = table.editing.as_ref() == Some(&cell);
    let symbol_uuid = sym.uuid;
    let sheet = sheet.to_path_buf();
    let (focus_sheet, focus_key) = (sheet.clone(), key.to_string());
    let focus = move |is_focused| {
        Message::ParameterManager(ParameterManagerMsg::CellFocused {
            sheet: focus_sheet.clone(),
            symbol_uuid,
            key: focus_key.clone(),
            is_focused,
        })
    };
    let edit_key = key.to_string();
    let input = text_input(
        t!(app.ui_state.i18n, ids::DIALOG_PARAMETER_MANAGER_NOT_SET),
        shown,
    )
    .on_input(move |value| {
        Message::ParameterManager(ParameterManagerMsg::Edit {
            sheet: sheet.clone(),
            symbol_uuid,
            key: edit_key.clone(),
            value,
        })
    })
    .on_submit(Message::ParameterManager(
        ParameterManagerMsg::CellSubmitted,
    ))
    .on_focus(focus(true))
    .on_blur(focus(false))
    .padding([styles::Spacing::DEFAULT.xs, m.pad - CHANGED_MARK])
    .size(TS.body)
    .font(param_cell_font(
        shown,
        &app.document_state.panel_ctx.ui_font_name,
    ))
    .width(Length::Fill)
    .style(inline_cell_style(style, is_open));
    container(row![changed_mark(is_changed, style), input].align_y(Alignment::Center))
        .width(Length::Fixed(width))
        .height(Length::Fixed(m.row_height))
        .align_y(Vertical::Center)
        .boxed()
}

/// A parameter cell of a closed sheet: its value, read-only and muted; a
/// press opens the sheet, so its cells can be edited (Caner 2026-11-25).
fn closed_param_cell<'a>(
    app: &'a Signex,
    sheet: &Path,
    sym: &'a Symbol,
    key: &str,
    width: f32,
    style: GridStyle,
) -> Element<'a, Message> {
    let shown: &str = sym.fields.get(key).map_or("", String::as_str);
    let [_, value_ink, _] = fixed_cell_inks(style);
    let font = param_cell_font(shown, &app.document_state.panel_ctx.ui_font_name);
    let cell = read_only_cell(shown, width, value_ink, font, style.metrics());
    mouse_area(cell)
        .on_press(Message::ParameterManager(
            ParameterManagerMsg::ClosedSheetPressed {
                sheet: sheet.to_path_buf(),
            },
        ))
        .interaction(iced::mouse::Interaction::Pointer)
        .boxed()
}

/// The 2 px accent mark left of a changed cell; clear on an unchanged one.
fn changed_mark<'a>(is_changed: bool, style: GridStyle) -> Element<'a, Message> {
    let color = if is_changed {
        style.accent
    } else {
        Color::TRANSPARENT
    };
    container(Space::new())
        .width(Length::Fixed(CHANGED_MARK))
        .height(Length::Fixed(style.metrics().row_height))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// Longest character count of `header` and `values`.
fn longest<'v>(header: &str, values: impl Iterator<Item = &'v str>) -> usize {
    values
        .map(|v| v.chars().count())
        .chain(std::iter::once(header.chars().count()))
        .max()
        .unwrap_or(0)
}

/// Width of the Sheet column for its longest text of `chars` characters.
fn sheet_column_width(size: f32, pad: f32, chars: usize) -> f32 {
    text_width(chars, size, pad).clamp(SHEET_EM.0 * size, SHEET_EM.1 * size)
}

/// The table for `rows` and the parameter columns `keys`, `width` px wide.
/// `show_sheet` adds a Sheet column after Reference (All sheets).
pub(super) fn view<'a>(
    app: &'a Signex,
    rows: &[PartRow<'a>],
    keys: Vec<String>,
    width: f32,
    show_sheet: bool,
) -> Element<'a, Message> {
    let tokens = &app.document_state.panel_ctx.tokens;
    let i18n = &app.ui_state.i18n;
    let style = GridStyle::from_tokens(tokens, TS.body);
    let m = style.metrics();

    let titles = [
        ids::DIALOG_PARAMETER_MANAGER_COLUMN_REFERENCE,
        ids::DIALOG_PARAMETER_MANAGER_COLUMN_VALUE,
        ids::DIALOG_PARAMETER_MANAGER_COLUMN_FOOTPRINT,
    ]
    .map(|id| t!(i18n, id).to_string());
    let syms = || rows.iter().map(|row| row.symbol);
    let fixed = [
        longest(&titles[0], rows.iter().map(|row| &*row.reference)),
        longest(&titles[1], syms().map(|s| s.value.as_str())),
        longest(&titles[2], syms().map(|s| s.footprint.as_str())),
    ];
    let params: Vec<usize> = keys
        .iter()
        .map(|k| {
            longest(
                k,
                syms().map(|s| s.fields.get(k).map_or("", String::as_str)),
            )
        })
        .collect();
    let sheet_title = t!(i18n, ids::DIALOG_PARAMETER_MANAGER_COLUMN_SHEET).to_string();
    let sheet_width = if show_sheet {
        let chars = longest(&sheet_title, rows.iter().map(|row| row.sheet_name.as_str()));
        sheet_column_width(TS.body, m.pad, chars)
    } else {
        0.0
    };
    let widths = column_widths(TS.body, m.pad, width - sheet_width, fixed, &params);
    let [reference_title, value_title, footprint_title] = titles;

    // The header row right of Reference: scrolls sideways with the body.
    let mut header_cells: Vec<Element<'a, Message>> = Vec::new();
    if show_sheet {
        header_cells.push(header_cell(&sheet_title, i18n, sheet_width, style));
    }
    header_cells.push(header_cell(&value_title, i18n, widths.value, style));
    header_cells.push(header_cell(&footprint_title, i18n, widths.footprint, style));
    for (key, w) in keys.iter().zip(&widths.params) {
        header_cells.push(param_header(app, key, *w, style));
    }
    let header_scroll = iced::widget::scrollable(row(header_cells))
        .id(HEADER_SCROLL_ID)
        .direction(scrollable::Direction::Horizontal(Scrollbar::hidden()))
        .on_scroll(|s| scrolled(ParameterTablePane::Header, s))
        .width(Length::Fill);
    let header_fill = style.header_fill;
    let header = container(
        row![
            header_cell(&reference_title, i18n, widths.reference, style),
            rule(style.line, Length::Fixed(m.row_height), false),
            header_scroll,
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(header_fill)),
        ..container::Style::default()
    });

    // The Reference column: scrolls up and down with the body.
    let mut references: Vec<Element<'a, Message>> = Vec::with_capacity(rows.len() * 2);
    let mut body_rows: Vec<Element<'a, Message>> = Vec::with_capacity(rows.len() * 2);
    let body_width =
        sheet_width + widths.value + widths.footprint + widths.params.iter().sum::<f32>();
    let table = &app.ui_state.parameter_table;
    let [reference_ink, value_ink, footprint_ink] = fixed_cell_inks(style);
    for part in rows {
        let sym = part.symbol;
        let fill = row_fill(table.is_editing_row(part.sheet, sym.uuid), style);
        references.push(filled(
            read_only_cell(
                &part.reference,
                widths.reference,
                reference_ink,
                reference_font(),
                m,
            ),
            fill,
        ));
        references.push(rule(style.line, Length::Fixed(widths.reference), true));
        let mut cells: Vec<Element<'a, Message>> = Vec::new();
        if show_sheet {
            cells.push(read_only_cell(
                &part.sheet_name,
                sheet_width,
                value_ink,
                mono(),
                m,
            ));
        }
        cells.push(read_only_cell(
            &sym.value,
            widths.value,
            value_ink,
            mono(),
            m,
        ));
        cells.push(read_only_cell(
            &sym.footprint,
            widths.footprint,
            footprint_ink,
            mono(),
            m,
        ));
        for (key, w) in keys.iter().zip(&widths.params) {
            let cell = if part.is_closed {
                closed_param_cell(app, part.sheet, part.symbol, key, *w, style)
            } else {
                param_cell(app, part.sheet, part.symbol, key, *w, style)
            };
            cells.push(cell);
        }
        body_rows.push(filled(row(cells).align_y(Alignment::Center).boxed(), fill));
        body_rows.push(rule(style.line, Length::Fixed(body_width), true));
    }
    let reference_scroll = iced::widget::scrollable(column(references))
        .id(REFERENCE_SCROLL_ID)
        .direction(scrollable::Direction::Vertical(Scrollbar::hidden()))
        .on_scroll(|s| scrolled(ParameterTablePane::References, s))
        .width(Length::Fixed(widths.reference))
        .height(Length::Fill);
    let body_scroll = iced::widget::scrollable(column(body_rows))
        .id(BODY_SCROLL_ID)
        .direction(styles::thin_both())
        .style(styles::scrollbar_style(tokens))
        .on_scroll(|s| scrolled(ParameterTablePane::Body, s))
        .width(Length::Fill)
        .height(Length::Fill);

    column![
        header,
        rule(style.line, Length::Fill, true),
        row![
            reference_scroll,
            rule(style.line, Length::Fill, false),
            body_scroll,
        ]
        .height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: f32 = 12.0;
    const PAD: f32 = 6.0;

    #[test]
    fn a_full_part_number_fits_its_column() {
        let mpn = "GRM31CR71E106KA12";
        let w = column_widths(SIZE, PAD, 400.0, [2, 3, 6], &[mpn.len()]);
        assert!(w.params[0] >= text_width(mpn.len(), SIZE, PAD));
    }

    #[test]
    fn a_parameter_column_is_never_under_its_minimum() {
        let w = column_widths(SIZE, PAD, 0.0, [2, 3, 6], &[1, 2]);
        assert!(w.params.iter().all(|&p| p >= PARAM_MIN_EM * SIZE));
        assert_eq!(PARAM_MIN_EM * SIZE, 140.0);
    }

    #[test]
    fn the_parameter_columns_share_the_free_width() {
        let w = column_widths(SIZE, PAD, 2000.0, [2, 3, 6], &[3, 3]);
        let total = w.reference + w.value + w.footprint + HAIRLINE + w.params.iter().sum::<f32>();
        assert!((total - 2000.0).abs() < 0.01, "no empty right edge");
        assert!((w.params[0] - w.params[1]).abs() < 0.01);
    }

    #[test]
    fn fixed_columns_stay_within_their_bounds() {
        let w = column_widths(SIZE, PAD, 900.0, [200, 0, 200], &[]);
        assert_eq!(w.reference, REFERENCE_EM.1 * SIZE);
        assert_eq!(w.value, VALUE_EM.0 * SIZE);
        assert_eq!(w.footprint, FOOTPRINT_EM.1 * SIZE);
    }

    fn graphite_style() -> GridStyle {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        GridStyle::from_tokens(&tokens, TS.body)
    }

    /// Mockup row 70: the column titles read in capitals, catalog title and
    /// parameter name alike.
    #[test]
    fn the_column_titles_read_in_capitals() {
        let en = I18n::en_us();
        let style = graphite_style();
        let window = iced::Size::new(TS.body * 20.0, TS.body * 4.0);
        let reference = t!(en, ids::DIALOG_PARAMETER_MANAGER_COLUMN_REFERENCE).to_string();
        for words in [reference.as_str(), "Tolerance"] {
            let cell = header_cell(words, &en, TS.body * 12.0, style);
            let texts = crate::layout_probe::lay_out_texts(cell, window);
            let shown: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
            assert_eq!(shown, [en.upper_for_display(words)], "{words}");
        }
    }

    /// Mockup row 70: references, values, footprints and parameter values
    /// are mono, the reference at medium weight; an empty cell's "Not set"
    /// stays in the italic UI face.
    #[test]
    fn cell_values_are_mono_and_not_set_is_the_italic_ui_face() {
        let ui = crate::fonts::DEFAULT_UI_FONT;
        assert_eq!(
            mono(),
            crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
        );
        assert_eq!(reference_font().family, mono().family);
        assert_eq!(reference_font().weight, iced::font::Weight::Medium);
        assert_eq!(param_cell_font("1%", ui), mono());
        let not_set = param_cell_font("", ui);
        assert_eq!(not_set.style, iced::font::Style::Italic);
        assert_eq!(
            not_set.family,
            crate::fonts::iced_font_for_family(ui).family
        );
    }

    /// Mockup row 70: the reference is drawn in full ink; the value and the
    /// footprint are muted.
    #[test]
    fn the_value_and_footprint_are_muted_and_the_reference_full_ink() {
        let style = graphite_style();
        assert_ne!(style.ink, style.muted_ink);
        assert_eq!(
            fixed_cell_inks(style),
            [style.ink, style.muted_ink, style.muted_ink]
        );
    }

    /// #2900, mockup row 70: the open cell is drawn as an active input (the
    /// field fill and the accent frame) whatever the widget reports; a
    /// closed cell reads as text until it is hovered.
    #[test]
    fn the_open_cell_is_drawn_as_an_active_input() {
        let style = graphite_style();
        let theme = Theme::Light;
        let open = inline_cell_style(style, true)(&theme, text_input::Status::Active);
        assert_eq!(open.background, Background::Color(style.field_fill));
        assert_eq!(open.border.color, style.accent);
        assert_eq!(open.border.width, HAIRLINE);
        let closed = inline_cell_style(style, false)(&theme, text_input::Status::Active);
        assert_eq!(closed.background, Background::Color(Color::TRANSPARENT));
        assert_eq!(closed.border, Border::default());
        let hovered = inline_cell_style(style, false)(&theme, text_input::Status::Hovered);
        assert_eq!(hovered.border.color, style.line);
    }

    /// The Sheet column (All sheets) fits a sheet name within its bounds.
    #[test]
    fn the_sheet_column_stays_within_its_bounds() {
        assert_eq!(sheet_column_width(SIZE, PAD, 1), SHEET_EM.0 * SIZE);
        assert_eq!(sheet_column_width(SIZE, PAD, 200), SHEET_EM.1 * SIZE);
        let name = "power.snxsch".len();
        assert!(sheet_column_width(SIZE, PAD, name) >= text_width(name, SIZE, PAD));
    }

    /// Mockup row 70: the row being edited is drawn on the accent tint; the
    /// other rows have no fill.
    #[test]
    fn the_row_being_edited_is_tinted() {
        let style = graphite_style();
        assert_eq!(row_fill(true, style), style.accent_soft);
        assert_eq!(row_fill(false, style), Color::TRANSPARENT);
        let m = style.metrics();
        let window = iced::Size::new(TS.body * 10.0, m.row_height);
        let drawn = |is_editing: bool| {
            let cell = read_only_cell("R3", window.width, style.ink, reference_font(), m);
            let pixels =
                crate::layout_probe::draw_pixels(filled(cell, row_fill(is_editing, style)), window);
            // The right end of the cell, clear of its text.
            let (w, y) = (window.width as usize, window.height as usize / 2);
            let i = (y * w + w - 2) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        assert_ne!(drawn(true), drawn(false));
    }
}
