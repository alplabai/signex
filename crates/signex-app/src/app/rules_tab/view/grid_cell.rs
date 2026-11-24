//! One cell of the constraint grid: a value set on its row (strong, with
//! ↺), one a constraint set wrote (an accent edge), an inherited one
//! (muted, or hidden while Show inherited values is off), an empty one, a
//! column not checked yet (inert) or one that does not apply to the row
//! (hatched). The tooltip names where the value comes from. An empty
//! clearance cell shows, muted, the clearance Run DRC checks with anyway;
//! All shows the fab tier's floor once a tier is picked, and the fixed No
//! and On of Short-circuit and Un-routed net (mockup row 89).

use std::collections::BTreeMap;

use iced::alignment::Vertical;
use iced::widget::{Row, Space, container, mouse_area};
use iced::{Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _};
use signex_drc_dsl::table::LimitField;
use signex_drc_dsl::table::grid::{CellSource, GridScope};
use signex_i18n::{Localized, ids, t};
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use signex_widgets::constraint_grid::{CellRef, EDITOR_ID, GridMessage, GridMetrics, GridStyle};
use signex_widgets::{button, text, text_input, tooltip};

use super::table::act;
use super::{Frame, TS, user};
use crate::app::Message;
use crate::app::rules_tab::grid::{CellShow, RowRef};
use crate::styles::HAIRLINE;

/// Border width of the selected cell's ring and a set value's edge.
const RING_EM: f32 = 1.5 / 12.0;

/// The clearance Run DRC checks a pair with while no rule asks more: the
/// open board's setup clearance (0.2 mm when it sets none or no board is
/// open), raised for a net class's nets by the class's own clearance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BuiltinClearance {
    board_mm: f64,
    classes: BTreeMap<String, f64>,
}

impl BuiltinClearance {
    /// `None` when the board's setup clearance is not usable (the check
    /// refuses the board too).
    pub(crate) fn of(board: &PcbBoard, classes: &[NetClass]) -> Option<Self> {
        let rules = signex_drc::DrcRules::for_board(board, &[]).ok()?;
        Some(Self::with_base(
            crate::app::rules_tab::json::mm(rules.clearance),
            classes,
        ))
    }

    /// With no board open: the check's default, which a board that sets no
    /// setup clearance is checked with.
    pub(crate) fn without_board(classes: &[NetClass]) -> Self {
        Self::with_base(signex_drc::DEFAULT_CLEARANCE_MM, classes)
    }

    fn with_base(board_mm: f64, classes: &[NetClass]) -> Self {
        let classes = classes
            .iter()
            .filter(|class| class.clearance.is_finite() && class.clearance > 0.0)
            .map(|class| (class.name.clone(), class.clearance))
            .collect();
        Self { board_mm, classes }
    }

    /// For `row`, whose parents are `parents` (nearest first); `None` for
    /// a heading or a scoped rule.
    pub(crate) fn for_row(&self, row: &RowRef, parents: &[GridScope]) -> Option<f64> {
        let class = match row {
            RowRef::Heading(_) | RowRef::Scoped(_) => return None,
            RowRef::Class(name) => Some(name.as_str()),
            _ => parents.iter().find_map(|parent| match parent {
                GridScope::Class(name) => Some(name.as_str()),
                _ => None,
            }),
        };
        let raised = class
            .and_then(|class| self.classes.get(class))
            .copied()
            .unwrap_or(0.0);
        Some(self.board_mm.max(raised))
    }
}

/// What an empty clearance cell checks with anyway; `None` for any other
/// column or a board whose setup clearance is not usable.
fn builtin_clearance(frame: &Frame, at: CellRef) -> Option<f64> {
    let grid = frame.grid();
    let column = grid.column(at.column)?;
    if column.field != Some(LimitField::MinClearance) {
        return None;
    }
    let row = grid.row(at.row)?;
    frame
        .builtin_clearance
        .as_ref()?
        .for_row(row, grid.parents(at.row))
}

/// The fixed value All shows in a column Run DRC always checks with no
/// rule to set it: shorts are never allowed and un-routed nets are always
/// checked (mockup row 89). `None` for any other cell.
fn fixed_value(frame: &Frame, at: CellRef) -> Option<Localized> {
    let grid = frame.grid();
    if !matches!(grid.row(at.row), Some(RowRef::All)) {
        return None;
    }
    match grid.column(at.column)?.id {
        "short_circuit_allowed" => Some(t!(frame.i18n, ids::RULES_CELL_NO)),
        "unrouted_net_check" => Some(t!(frame.i18n, ids::RULES_CELL_ON)),
        _ => None,
    }
}

fn strong() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..signex_widgets::ui_font::regular()
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
        let parent = cells.parent_value(at);
        let placeholder: signex_widgets::Label<'a> =
            user(parent.written().unwrap_or_default().to_string()).into();
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
    let is_inert = match show {
        CellShow::NotApplicable => true,
        CellShow::NotChecked => fixed_value(frame, at).is_none(),
        _ => false,
    };
    let fill = if is_inert {
        Some(style.inert_fill)
    } else if is_selected {
        Some(style.accent_soft)
    } else {
        None
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
        container(text(source_tip(frame, &show, at, is_all)).size(TS.caption))
            .style(crate::styles::card(frame.tokens)),
        tooltip::Position::Bottom,
    )
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
            fixed_value(frame, at)
                .unwrap_or_else(|| t!(frame.i18n, ids::RULES_CELL_EMPTY))
                .into_string(),
            style.muted_ink,
            signex_widgets::ui_font::regular(),
        ),
        // All keeps its base value in view, as a value of its own would.
        CellShow::Empty => match builtin_clearance(frame, at) {
            Some(mm) if is_all || !hide_inherited => (
                format!("{mm}mm"),
                style.muted_ink,
                signex_widgets::ui_font::regular(),
            ),
            _ => (
                String::new(),
                style.muted_ink,
                signex_widgets::ui_font::regular(),
            ),
        },
        CellShow::Value {
            written, source, ..
        } => {
            let shown = written.clone();
            match source {
                CellSource::Own { .. } if !is_all => (shown, style.ink, strong()),
                CellSource::Set { .. } => (shown, style.ink, signex_widgets::ui_font::regular()),
                CellSource::Own { .. } => (shown, style.ink, signex_widgets::ui_font::regular()),
                // All keeps the fab tier's floor in view, as its own value.
                _ if hide_inherited && !is_all => (
                    String::new(),
                    style.muted_ink,
                    signex_widgets::ui_font::regular(),
                ),
                _ => (shown, style.muted_ink, signex_widgets::ui_font::regular()),
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
        CellShow::NotChecked if fixed_value(frame, at).is_some() => {
            t!(i18n, ids::RULES_CELL_ALWAYS_CHECKED)
        }
        CellShow::NotChecked => t!(i18n, ids::RULES_CELL_NOT_CHECKED),
        CellShow::Empty if builtin_clearance(frame, at).is_some() => {
            t!(i18n, ids::RULES_CELL_FROM_DEFAULT)
        }
        CellShow::Empty => t!(i18n, ids::RULES_CELL_NOT_SET),
        CellShow::Value { source, .. } => match source {
            _ if is_all => t!(i18n, ids::RULES_CELL_FROM_ALL),
            CellSource::Own { .. } => {
                let parent = frame.cells().parent_value(at);
                let back = match parent.written() {
                    Some(written) => written.to_string(),
                    None => t!(i18n, ids::RULES_CELL_EMPTY).into_string(),
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

#[cfg(test)]
#[path = "grid_cell_tests.rs"]
mod tests;
