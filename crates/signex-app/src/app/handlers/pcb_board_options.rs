//! The board Properties panel's board options (#2374): guides, extra snap
//! grids, the sheet and the naming options. Each edit is one
//! `BoardCommand::SetBoardOptions` undo step on the board in view; the
//! Axis Snap Range is an editor setting for the session (`UiState`).

use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::placement::grid_step_mm;
use signex_i18n::{ids, t};
use signex_types::measure::parse_length_field_mm;
use signex_types::pcb::Point;
use signex_types::pcb_options::{
    BoardGrid, BoardGuide, BoardOptions, GridUse, GuideKind, MARK_COLORS, next_mark_color,
};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::state::pcb_edit::CheckedStep;
use crate::panels::{BoardField, GridCard, pcb_board_properties};

impl Signex {
    /// Run `edit` on a copy of the active board's options and, when it says
    /// something changed, set them as one undo step. A refusal is said in
    /// the Messages panel and changes nothing. Whether the board changed.
    pub(crate) fn edit_pcb_board_options(
        &mut self,
        edit: impl FnOnce(&mut BoardOptions) -> bool,
    ) -> bool {
        let Some(path) = self.active_tab_path() else {
            return false;
        };
        self.edit_pcb_board_options_at(&path, edit)
    }

    /// [`Self::edit_pcb_board_options`] on the open board at `path`.
    pub(crate) fn edit_pcb_board_options_at(
        &mut self,
        path: &std::path::Path,
        edit: impl FnOnce(&mut BoardOptions) -> bool,
    ) -> bool {
        let Some(engine) = self.document_state.board_engine(path) else {
            return false;
        };
        let mut options = engine.board().options.clone();
        if !edit(&mut options) {
            return false;
        }
        let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
        let label = t!(i18n, ids::PCB_BOARD_OPTIONS_UNDO).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: engine.revision(),
        };
        let command = BoardCommand::SetBoardOptions {
            options: Box::new(options),
        };
        match self.execute_board_commands(path, vec![command], Some(step)) {
            Ok(_) => true,
            Err(error) => {
                self.say_pcb_options_refused(error.to_string());
                false
            }
        }
    }

    fn say_pcb_options_refused(&self, why: String) {
        let line = t!(self.ui_state.i18n, ids::PCB_BOARD_OPTIONS_REFUSED, "why" => why);
        crate::diagnostics::log_warning(line.into_string());
    }

    /// A length typed in the panel's unit; `None`, said in Messages, for
    /// text that is not one.
    fn typed_length(&self, text: &str) -> Option<f64> {
        let unit = pcb_board_properties::shown_unit(self.ui_state.unit);
        let mm = parse_length_field_mm(text, unit).filter(|mm| mm.is_finite());
        if mm.is_none() {
            let why = t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_INVALID).into_string();
            self.say_pcb_options_refused(why);
        }
        mm
    }

    /// Enter in one of the panel's fields: apply its draft.
    pub(crate) fn submit_pcb_board_field(&mut self, field: BoardField) {
        let Some(text) = self.ui_state.pcb_board_panel.drafts.remove(&field) else {
            return;
        };
        if let BoardField::Panel(setting) = field {
            self.submit_board_panel_setting(setting, &text);
            return;
        }
        if let BoardField::GridName(id) = field {
            let name = text.trim().to_string();
            if !name.is_empty() {
                self.edit_pcb_board_options(|o| edit_grid(o, id, |g| set(&mut g.name, name)));
            }
            return;
        }
        let Some(mm) = self.typed_length(&text) else {
            return;
        };
        match field {
            BoardField::AxisRange if mm > 0.0 => self.ui_state.pcb_snap.axis_range_mm = mm,
            BoardField::AxisRange => {
                let why = t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_INVALID).into_string();
                self.say_pcb_options_refused(why);
            }
            BoardField::GuideX(id) => {
                self.edit_pcb_board_options(|o| {
                    let x = o.origin.board_x(mm);
                    edit_guide(o, id, |g| set(&mut g.at.x, x))
                });
            }
            BoardField::GuideY(id) => {
                self.edit_pcb_board_options(|o| {
                    let y = o.origin.board_y(mm);
                    edit_guide(o, id, |g| set(&mut g.at.y, y))
                });
            }
            BoardField::GridStep(id) => {
                self.edit_pcb_board_options(|o| edit_grid(o, id, |g| set(&mut g.step_mm, mm)));
            }
            BoardField::GridOriginX(id) => {
                self.edit_pcb_board_options(|o| {
                    let x = o.origin.board_x(mm);
                    edit_grid(o, id, |g| set(&mut g.origin.x, x))
                });
            }
            BoardField::GridOriginY(id) => {
                self.edit_pcb_board_options(|o| {
                    let y = o.origin.board_y(mm);
                    edit_grid(o, id, |g| set(&mut g.origin.y, y))
                });
            }
            BoardField::SheetX => {
                self.edit_pcb_board_options(|o| {
                    let x = o.origin.board_x(mm);
                    set(&mut o.sheet.origin.x, x)
                });
            }
            BoardField::SheetY => {
                // Typed as the bottom-left corner, Y up (#2384).
                self.edit_pcb_board_options(|o| {
                    let y = o.origin.board_y(mm) - o.sheet.height;
                    set(&mut o.sheet.origin.y, y)
                });
            }
            BoardField::SheetWidth => {
                self.edit_pcb_board_options(|o| set(&mut o.sheet.width, mm));
            }
            BoardField::SheetHeight => {
                self.edit_pcb_board_options(|o| set(&mut o.sheet.height, mm));
            }
            BoardField::GridName(_) | BoardField::Panel(_) => {}
        }
    }

    /// Add a guide of `kind` through the board origin: one undo step.
    pub(crate) fn add_pcb_guide(&mut self, kind: GuideKind) {
        let origin = self
            .active_pcb()
            .map_or(Point::ZERO, |b| b.options.origin.at);
        self.add_pcb_guide_at(kind, origin);
    }

    fn add_pcb_guide_at(&mut self, kind: GuideKind, at: Point) {
        self.edit_pcb_board_options(|o| {
            o.guides.push(BoardGuide::new(Uuid::now_v7(), kind, at));
            true
        });
    }

    /// Place: the next click on this window's board sets a guide.
    pub(crate) fn arm_pcb_guide_place(&mut self, kind: GuideKind) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        self.interaction_state.pcb_guide_place = Some((path, kind));
        // Measure would take the guide's click (#2403).
        self.interaction_state.pcb_query.measure = None;
        let hint = t!(self.ui_state.i18n, ids::PCB_BOARD_GUIDE_PLACE_HINT);
        crate::diagnostics::log_info(hint.into_string());
    }

    /// A click while a guide is armed sets it there, on the grid for
    /// non-components. Whether the click belonged to it.
    pub(crate) fn pcb_guide_place_clicked(&mut self, at: Point) -> bool {
        let armed = self.interaction_state.pcb_guide_place.clone();
        let Some((path, kind)) = armed else {
            return false;
        };
        if self.active_tab_path().as_deref() != Some(path.as_path()) {
            return false;
        }
        self.interaction_state.pcb_guide_place = None;
        let Some(board) = self.active_pcb() else {
            return true;
        };
        let at = self.board_grid_point(board, GridUse::NonComponents, at);
        self.add_pcb_guide_at(kind, at);
        true
    }

    pub(crate) fn toggle_pcb_guide(&mut self, id: Uuid) {
        self.edit_pcb_board_options(|o| {
            edit_guide(o, id, |g| {
                g.enabled = !g.enabled;
                true
            })
        });
    }

    pub(crate) fn step_pcb_guide_color(&mut self, id: Uuid) {
        self.edit_pcb_board_options(|o| {
            edit_guide(o, id, |g| {
                g.color = next_mark_color(g.color);
                true
            })
        });
    }

    /// A guide card's Delete: one undo step.
    pub(crate) fn delete_pcb_guide(&mut self, id: Uuid) {
        if self.edit_pcb_board_options(|o| remove_where(&mut o.guides, |g| g.uuid == id)) {
            let view = &mut self.ui_state.pcb_board_panel;
            view.guide_menu = None;
        }
    }

    /// Add a grid at the top priority, on the board's step at the origin,
    /// for components and non-components; select it and show its fields.
    pub(crate) fn add_pcb_grid(&mut self) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let count = board.options.grids.len();
        let grid = BoardGrid {
            uuid: Uuid::now_v7(),
            name: t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_NEW_NAME, "number" => count + 1)
                .into_string(),
            step_mm: grid_step_mm(board),
            origin: Point::ZERO,
            color: MARK_COLORS[count % MARK_COLORS.len()],
            components: true,
            non_components: true,
        };
        let id = grid.uuid;
        if self.edit_pcb_board_options(|o| {
            o.grids.insert(0, grid);
            true
        }) {
            let view = &mut self.ui_state.pcb_board_panel;
            view.grid_editing = Some(GridCard::Extra(id));
        }
    }

    /// A grid card's Delete: one undo step.
    pub(crate) fn delete_pcb_grid(&mut self, id: Uuid) {
        if self.edit_pcb_board_options(|o| remove_where(&mut o.grids, |g| g.uuid == id)) {
            let view = &mut self.ui_state.pcb_board_panel;
            view.grid_menu = None;
            if view.grid_editing == Some(GridCard::Extra(id)) {
                view.grid_editing = None;
            }
        }
    }

    /// Move Up or Move Down: swap the grid with its neighbour in priority,
    /// one undo step. The first extra grid cannot go above the board's own,
    /// nor the last below it.
    pub(crate) fn move_pcb_grid(&mut self, id: Uuid, up: bool) {
        self.edit_pcb_board_options(|o| {
            let Some(at) = o.grids.iter().position(|g| g.uuid == id) else {
                return false;
            };
            let to = if up { at.checked_sub(1) } else { Some(at + 1) };
            match to.filter(|to| *to < o.grids.len()) {
                Some(to) => {
                    o.grids.swap(at, to);
                    true
                }
                None => false,
            }
        });
    }

    /// Flip whether the grid applies to components, or to non-components.
    pub(crate) fn toggle_pcb_grid_use(&mut self, id: Uuid, components: bool) {
        self.edit_pcb_board_options(|o| {
            edit_grid(o, id, |g| {
                let flag = if components {
                    &mut g.components
                } else {
                    &mut g.non_components
                };
                *flag = !*flag;
                true
            })
        });
    }

    pub(crate) fn step_pcb_grid_color(&mut self, id: Uuid) {
        self.edit_pcb_board_options(|o| {
            edit_grid(o, id, |g| {
                g.color = next_mark_color(g.color);
                true
            })
        });
    }

    /// Get Size From Sheet Layer. Turned off, the sheet keeps the box it
    /// had, so nothing jumps: the box around the board shape.
    pub(crate) fn toggle_pcb_sheet_from_board(&mut self) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let shape = shape_box(&board.outline);
        self.edit_pcb_board_options(|o| {
            let sheet = &mut o.sheet;
            sheet.from_board = !sheet.from_board;
            if !sheet.from_board
                && let Some((lo, hi)) = shape
            {
                sheet.origin = lo;
                sheet.width = hi.x - lo.x;
                sheet.height = hi.y - lo.y;
            }
            true
        });
    }
}

/// Set `slot` to `value`; whether it changed.
fn set<T: PartialEq>(slot: &mut T, value: T) -> bool {
    std::mem::replace(slot, value) != *slot
}

fn edit_guide(o: &mut BoardOptions, id: Uuid, f: impl FnOnce(&mut BoardGuide) -> bool) -> bool {
    o.guides.iter_mut().find(|g| g.uuid == id).is_some_and(f)
}

fn edit_grid(o: &mut BoardOptions, id: Uuid, f: impl FnOnce(&mut BoardGrid) -> bool) -> bool {
    o.grids.iter_mut().find(|g| g.uuid == id).is_some_and(f)
}

fn remove_where<T>(items: &mut Vec<T>, hit: impl Fn(&T) -> bool) -> bool {
    let before = items.len();
    items.retain(|item| !hit(item));
    items.len() != before
}

/// The box around a board outline of three or more points; `None` without
/// one (the board area is then the default area, the sheet's default box).
fn shape_box(outline: &[Point]) -> Option<(Point, Point)> {
    if outline.len() < 3 {
        return None;
    }
    let mut lo = outline[0];
    let mut hi = outline[0];
    for p in outline {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Some((lo, hi))
}

#[cfg(test)]
#[path = "pcb_board_options_tests.rs"]
mod tests;
