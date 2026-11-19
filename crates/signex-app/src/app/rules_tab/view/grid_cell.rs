//! One cell of the constraint grid: a value set on its row (strong, with
//! ↺), one a constraint set wrote (an accent edge), an inherited one
//! (muted, or hidden while Show inherited values is off), an empty one, a
//! column not checked yet (inert) or one that does not apply to the row
//! (hatched). The tooltip names where the value comes from.

use iced::alignment::Vertical;
use iced::widget::{Row, Space, container, mouse_area};
use iced::{Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _};
use signex_drc_dsl::table::grid::{CellSource, GridScope};
use signex_i18n::{Localized, ids, t};
use signex_widgets::constraint_grid::{CellRef, EDITOR_ID, GridMessage, GridMetrics, GridStyle};
use signex_widgets::{button, text, text_input, tooltip};

use super::table::act;
use super::{Frame, TS, user};
use crate::app::Message;
use crate::app::rules_tab::grid::{CellShow, RowRef, mm_format};
use crate::styles::HAIRLINE;

/// Border width of the selected cell's ring and a set value's edge.
const RING_EM: f32 = 1.5 / 12.0;

fn strong() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

pub(super) fn view<'a>(
    frame: &Frame<'a>,
    at: CellRef,
    style: GridStyle,
    m: GridMetrics,
) -> Element<'a, Message> {
    let grid = frame.grid();
    let is_selected = grid.view.selected() == Some(at);
    let editing = grid.view.edit().filter(|e| e.cell == at);
    let cells = frame.cells();
    let show = cells.show(at);
    if let (Some(edit), true) = (editing, show.is_editable()) {
        let placeholder: signex_widgets::Label<'a> = match cells.parent_value(at) {
            CellShow::Value { mm: Some(mm), .. } => user(mm_format(frame.i18n).number(mm)).into(),
            _ => user(String::new()).into(),
        };
        let is_invalid = edit.is_invalid;
        let input: Element<'a, Message> = text_input(placeholder, edit.text.as_str())
            .id(EDITOR_ID)
            .on_input({
                let id = frame.id;
                move |typed| {
                    super::up(crate::app::rules_tab::RulesTabMsg::GridActed {
                        project_id: id,
                        msg: GridMessage::EditTextChanged(typed),
                    })
                }
            })
            .on_submit(act(frame, GridMessage::EditSubmitted))
            .size(TS.body)
            .padding(Padding::from([0.0, m.pad]))
            .width(Length::Fill)
            .style(move |_: &Theme, _| iced::widget::text_input::Style {
                background: Background::Color(style.field_fill),
                border: Border {
                    color: if is_invalid {
                        style.error
                    } else {
                        style.accent
                    },
                    width: HAIRLINE,
                    radius: 0.0.into(),
                    ..Border::default()
                },
                placeholder: style.muted_ink,
                value: style.ink,
                selection: style.accent_soft,
            })
            .boxed();
        return container(input)
            .width(m.cell_width)
            .height(m.row_height)
            .align_y(Vertical::Center)
            .boxed();
    }
    let is_all = matches!(grid.row(at.row), Some(RowRef::All));
    let body = face(frame, &show, at, is_all, style, m);
    let set_edge = matches!(
        show,
        CellShow::Value {
            source: CellSource::Set { .. },
            ..
        }
    );
    let fill = match show {
        CellShow::NotApplicable | CellShow::NotChecked => Some(style.inert_fill),
        _ if is_selected => Some(style.accent_soft),
        _ => None,
    };
    let ring = style.text_size * RING_EM;
    let edge = if is_selected || set_edge {
        (style.accent, ring)
    } else {
        (style.line, HAIRLINE)
    };
    let cell = container(body)
        .padding(Padding::from([0.0, m.pad]))
        .width(m.cell_width)
        .height(m.row_height)
        .align_y(Vertical::Center)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                color: edge.0,
                width: edge.1,
                radius: 0.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let press = mouse_area(cell).on_press(act(frame, GridMessage::CellPressed(at)));
    tooltip(
        press,
        text(source_tip(frame, &show, at, is_all)).size(TS.caption),
        tooltip::Position::Bottom,
    )
    .style(crate::styles::card(frame.tokens))
    .boxed()
}

/// What the cell shows: the value and, for a value of the row's own, ↺.
fn face<'a>(
    frame: &Frame<'a>,
    show: &CellShow,
    at: CellRef,
    is_all: bool,
    style: GridStyle,
    m: GridMetrics,
) -> Element<'a, Message> {
    let hide_inherited = frame.state.hide_inherited;
    let (value, ink, font): (String, Color, Font) = match show {
        CellShow::NotApplicable => return Space::new().boxed(),
        CellShow::NotChecked => (
            t!(frame.i18n, ids::RULES_CELL_EMPTY).into_string(),
            style.muted_ink,
            Font::DEFAULT,
        ),
        CellShow::Empty => (String::new(), style.muted_ink, Font::DEFAULT),
        CellShow::Value {
            written,
            mm,
            source,
            ..
        } => {
            let shown = mm.map_or_else(|| written.clone(), |mm| mm_format(frame.i18n).number(mm));
            match source {
                CellSource::Own { .. } if !is_all => (shown, style.ink, strong()),
                CellSource::Set { .. } => (shown, style.ink, Font::DEFAULT),
                CellSource::Own { .. } => (shown, style.ink, Font::DEFAULT),
                _ if hide_inherited => (String::new(), style.muted_ink, Font::DEFAULT),
                _ => (shown, style.muted_ink, Font::DEFAULT),
            }
        }
    };
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(m.pad / 2.0)
        .align_y(Vertical::Center)
        .push(
            text(user(value))
                .size(TS.body)
                .color(ink)
                .font(font)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
                .boxed(),
        );
    if show.is_own() && !is_all {
        line = line.push(reset(frame, at, style, m));
    }
    line.boxed()
}

/// The ↺ button: back to the inherited value.
fn reset<'a>(
    frame: &Frame<'a>,
    at: CellRef,
    style: GridStyle,
    m: GridMetrics,
) -> Element<'a, Message> {
    button(
        text(t!(frame.i18n, ids::RULES_GRID_RESET_GLYPH))
            .size(TS.body)
            .color(style.muted_ink),
    )
    .padding(0)
    .width(m.glyph)
    .style(|_: &Theme, _| button::Style::default())
    .on_press(act(frame, GridMessage::CellReset(at)))
    .boxed()
}

/// Where the cell's value comes from, worded for its tooltip.
fn source_tip(frame: &Frame, show: &CellShow, at: CellRef, is_all: bool) -> Localized {
    let i18n = frame.i18n;
    match show {
        CellShow::NotApplicable => t!(i18n, ids::RULES_CELL_NOT_APPLICABLE),
        CellShow::NotChecked => t!(i18n, ids::RULES_CELL_NOT_CHECKED),
        CellShow::Empty => t!(i18n, ids::RULES_CELL_NOT_SET),
        CellShow::Value { source, .. } => match source {
            _ if is_all => t!(i18n, ids::RULES_CELL_FROM_ALL),
            CellSource::Own { .. } => {
                let back = match frame.cells().parent_value(at) {
                    CellShow::Value { mm: Some(mm), .. } => mm_format(i18n).number(mm),
                    _ => t!(i18n, ids::RULES_CELL_EMPTY).into_string(),
                };
                t!(i18n, ids::RULES_CELL_FROM_ROW, "value" => back)
            }
            CellSource::Set { set, .. } => {
                t!(i18n, ids::RULES_CELL_FROM_SET, "set" => set.as_str())
            }
            CellSource::Inherited { from, .. } => {
                t!(i18n, ids::RULES_CELL_FROM_PARENT, "row" => scope_name(frame, from))
            }
            CellSource::Default => t!(i18n, ids::RULES_CELL_FROM_DEFAULT),
        },
    }
}

/// A parent row's name, as the grid shows it.
pub(super) fn scope_name(frame: &Frame, scope: &GridScope) -> String {
    let row = frame
        .grid()
        .row_of_scope(scope)
        .and_then(|id| frame.grid().row(id).cloned());
    match row {
        Some(row) => super::table::row_name(frame, &row).as_str().to_string(),
        None => match scope {
            GridScope::All => t!(frame.i18n, ids::RULES_GRID_ROW_ALL).into_string(),
            GridScope::Class(name) | GridScope::Net(name) | GridScope::Layer(name) => name.clone(),
        },
    }
}
