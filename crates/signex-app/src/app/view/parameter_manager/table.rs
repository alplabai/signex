//! The Parameter Manager table. The header row stays on top and the
//! Reference column stays on the left: each is its own scrollable without
//! a scrollbar, and `TableScrolled` keeps them at the body's offset.
//!
//! The constraint grid (`signex_widgets::constraint_grid`) is not used for
//! the cells: its values are typed lengths, choices and flags over an
//! inheriting row hierarchy, and these are free text on flat rows. Its
//! colours and metrics (`GridStyle`, `GridMetrics`) are reused, so both
//! tables look alike.

use iced::alignment::Vertical;
use iced::widget::scrollable::{self, Scroll, Scrollbar, Source};
use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
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

const TS: TypeScale = TypeScale::DEFAULT;

/// Average glyph advance in em, to size a column to its longest text.
const GLYPH_ADVANCE_EM: f32 = 0.62;
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

/// A cell that shows `value` and cannot be edited.
fn read_only_cell<'a>(value: &str, width: f32, ink: Color, m: GridMetrics) -> Element<'a, Message> {
    container(
        text(UserText::new(value.to_string()))
            .size(TS.body)
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

/// A header caption cell.
fn header_cell<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    width: f32,
    style: GridStyle,
) -> Element<'a, Message> {
    container(
        text(label)
            .size(TS.caption)
            .color(style.muted_ink)
            .wrapping(Wrapping::None),
    )
    .padding([0.0, style.metrics().pad])
    .width(Length::Fixed(width))
    .height(Length::Fixed(style.metrics().row_height))
    .align_y(Vertical::Center)
    .clip(true)
    .boxed()
}

/// A text input that reads as text until it is hovered or focused.
fn inline_cell_style(style: GridStyle) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_: &Theme, status| {
        let frame = |line: Color| Border {
            width: HAIRLINE,
            color: line,
            radius: styles::Radii::GRAPHITE.control.into(),
            ..Border::default()
        };
        let (background, border) = match status {
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
    let mut content = row![
        text(UserText::new(key.to_string()))
            .size(TS.caption)
            .color(style.muted_ink)
            .wrapping(Wrapping::None)
            .width(Length::Fill),
    ]
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
    sym: &'a Symbol,
    key: &str,
    width: f32,
    style: GridStyle,
) -> Element<'a, Message> {
    let m = style.metrics();
    let shown: &str = sym.fields.get(key).map_or("", String::as_str);
    let cell = ParameterCell {
        sheet: app.document_state.active_path.clone(),
        symbol: sym.uuid,
        key: key.to_string(),
    };
    let is_changed = app
        .ui_state
        .parameter_table
        .session
        .original(&cell)
        .is_some_and(|before| before.as_deref() != sym.fields.get(key).map(String::as_str));
    let symbol_uuid = sym.uuid;
    let edit_key = key.to_string();
    let mut input = text_input(
        t!(app.ui_state.i18n, ids::DIALOG_PARAMETER_MANAGER_NOT_SET),
        shown,
    )
    .on_input(move |value| {
        Message::ParameterManager(ParameterManagerMsg::Edit {
            symbol_uuid,
            key: edit_key.clone(),
            value,
        })
    })
    .on_submit(Message::ParameterManager(
        ParameterManagerMsg::CellSubmitted,
    ))
    .padding([styles::Spacing::DEFAULT.xs, m.pad - CHANGED_MARK])
    .size(TS.body)
    .width(Length::Fill)
    .style(inline_cell_style(style));
    if shown.is_empty() {
        input = input.font(not_set_font(&app.document_state.panel_ctx.ui_font_name));
    }
    let mark_color = if is_changed {
        style.accent
    } else {
        Color::TRANSPARENT
    };
    let mark = container(Space::new())
        .width(Length::Fixed(CHANGED_MARK))
        .height(Length::Fixed(m.row_height))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(mark_color)),
            ..container::Style::default()
        });
    container(row![mark, input].align_y(Alignment::Center))
        .width(Length::Fixed(width))
        .height(Length::Fixed(m.row_height))
        .align_y(Vertical::Center)
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

/// The table for `rows` and the parameter columns `keys`, `width` px wide.
pub(super) fn view<'a>(
    app: &'a Signex,
    rows: &[&'a Symbol],
    keys: Vec<String>,
    width: f32,
) -> Element<'a, Message> {
    let tokens = &app.document_state.panel_ctx.tokens;
    let i18n = &app.ui_state.i18n;
    let style = GridStyle::from_tokens(tokens, TS.body);
    let m = style.metrics();

    let titles = [
        t!(i18n, ids::DIALOG_PARAMETER_MANAGER_COLUMN_REFERENCE),
        t!(i18n, ids::DIALOG_PARAMETER_MANAGER_COLUMN_VALUE),
        t!(i18n, ids::DIALOG_PARAMETER_MANAGER_COLUMN_FOOTPRINT),
    ];
    let fixed = [
        longest(
            &titles[0].to_string(),
            rows.iter().map(|s| s.reference.as_str()),
        ),
        longest(
            &titles[1].to_string(),
            rows.iter().map(|s| s.value.as_str()),
        ),
        longest(
            &titles[2].to_string(),
            rows.iter().map(|s| s.footprint.as_str()),
        ),
    ];
    let params: Vec<usize> = keys
        .iter()
        .map(|k| {
            longest(
                k,
                rows.iter()
                    .map(|s| s.fields.get(k).map_or("", String::as_str)),
            )
        })
        .collect();
    let widths = column_widths(TS.body, m.pad, width, fixed, &params);
    let [reference_title, value_title, footprint_title] = titles;

    // The header row right of Reference: scrolls sideways with the body.
    let mut header_cells: Vec<Element<'a, Message>> = vec![
        header_cell(value_title, widths.value, style),
        header_cell(footprint_title, widths.footprint, style),
    ];
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
            header_cell(reference_title, widths.reference, style),
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
    let body_width = widths.value + widths.footprint + widths.params.iter().sum::<f32>();
    for &sym in rows {
        references.push(read_only_cell(
            &sym.reference,
            widths.reference,
            style.ink,
            m,
        ));
        references.push(rule(style.line, Length::Fixed(widths.reference), true));
        let mut cells: Vec<Element<'a, Message>> = vec![
            read_only_cell(&sym.value, widths.value, style.ink, m),
            read_only_cell(&sym.footprint, widths.footprint, style.muted_ink, m),
        ];
        for (key, w) in keys.iter().zip(&widths.params) {
            cells.push(param_cell(app, sym, key, *w, style));
        }
        body_rows.push(row(cells).align_y(Alignment::Center).boxed());
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
}
