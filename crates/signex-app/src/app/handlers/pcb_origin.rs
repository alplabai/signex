//! Edit ▸ Origin (2026-11-18): set the board origin at a clicked point,
//! reset it to the bottom-left of the board outline (of the board area
//! without one), or snap it to the outline corner nearest the pointer. Each
//! change is one `SetBoardOptions` undo step on `BoardOptions::origin`, so it
//! marks the board changed and is written on the next save. A read-only
//! board takes none. See `docs/PCB_BOARD_OPTIONS.md`, "Origin".

use signex_i18n::{ids, t};
use signex_types::pcb::Point;
use std::path::Path;

use signex_types::pcb_options::{BoardOptions, BoardOrigin, GridUse};
use signex_types::pcb_origin::nearest_area_corner;

use crate::app::Signex;
use crate::app::contracts::BoardOriginMsg;

impl Signex {
    pub(crate) fn handle_board_origin(&mut self, event: BoardOriginMsg) {
        match event {
            BoardOriginMsg::SetPicked => self.arm_pcb_origin_place(),
            BoardOriginMsg::ResetPicked => {
                let Some(path) = self.active_tab_path() else {
                    return;
                };
                if let Err(why) = self.reset_pcb_origin_at(&path) {
                    crate::diagnostics::log_info(format!("Reset origin: {why}"));
                }
            }
            BoardOriginMsg::SnapToCornerPicked => self.snap_pcb_origin_to_corner(),
        }
    }

    /// The outline of the board in view when it takes an edit.
    fn editable_board_outline(&self) -> Option<Vec<Point>> {
        self.active_pcb()
            .filter(|board| board.unknown.newer_minor_source.is_none())
            .map(|board| board.outline.clone())
    }

    /// Set: the next click on the main window's board sets the origin.
    fn arm_pcb_origin_place(&mut self) {
        if self.editable_board_outline().is_none() {
            return;
        }
        let Some(path) = self.active_tab_path() else {
            return;
        };
        self.interaction_state.pcb_origin_place = Some(path);
        // A guide or Measure would take the click (#2374, #2403).
        self.interaction_state.pcb_guide_place = None;
        self.interaction_state.pcb_query.measure = None;
        let hint = t!(self.ui_state.i18n, ids::PCB_ORIGIN_SET_HINT);
        crate::diagnostics::log_info(hint.into_string());
    }

    /// A click while Set is armed sets the origin there, on the grid for
    /// non-components. Whether the click belonged to it.
    pub(crate) fn pcb_origin_place_clicked(&mut self, at: Point) -> bool {
        let Some(path) = self.interaction_state.pcb_origin_place.clone() else {
            return false;
        };
        let main_window = self.interaction_state.acting_window.is_none();
        if !main_window || self.active_tab_path().as_deref() != Some(path.as_path()) {
            return false;
        }
        self.interaction_state.pcb_origin_place = None;
        let Some(board) = self.active_pcb() else {
            return true;
        };
        let at = self.board_grid_point(board, GridUse::NonComponents, at);
        self.set_pcb_origin(at);
        true
    }

    /// Snap to Outline Corner: the corner nearest the pointer's last place
    /// on this board; Messages says so when the pointer has not been on it.
    fn snap_pcb_origin_to_corner(&mut self) {
        let Some(outline) = self.editable_board_outline() else {
            return;
        };
        let on_board = self.ui_state.cursor_document.is_some()
            && self.ui_state.cursor_document == self.active_tab_path();
        if !on_board {
            let line = t!(self.ui_state.i18n, ids::PCB_ORIGIN_NO_POINTER);
            crate::diagnostics::log_info(line.into_string());
            return;
        }
        let pointer = Point::new(self.ui_state.cursor_x, self.ui_state.cursor_y);
        self.set_pcb_origin(nearest_area_corner(&outline, pointer));
    }

    /// Reset on the board at `path`, the menu row's and the
    /// `reset_board_origin` command's one path: the origin to the bottom-left
    /// of the outline (of the board area without one), one undo step.
    /// Whether it moved; why not when the board is not open or read-only.
    pub(crate) fn reset_pcb_origin_at(&mut self, path: &Path) -> Result<bool, String> {
        let board = self
            .document_state
            .pcb_board(path)
            .ok_or("the board is not open")?;
        if board.unknown.newer_minor_source.is_some() {
            return Err("the board opens read-only".to_string());
        }
        let at = BoardOrigin::of_new_board(&board.outline).at;
        Ok(self.edit_pcb_board_options_at(path, |options| move_origin(options, at)))
    }

    /// Move the origin to `at`: one undo step, nothing when it is there.
    fn set_pcb_origin(&mut self, at: Point) {
        self.edit_pcb_board_options(|options| move_origin(options, at));
    }
}

/// Put the origin of `options` at `at`; whether it moved.
fn move_origin(options: &mut BoardOptions, at: Point) -> bool {
    let origin = BoardOrigin::new(at);
    let moved = options.origin != origin;
    options.origin = origin;
    moved
}

#[cfg(test)]
#[path = "pcb_origin_menu_tests.rs"]
mod tests;
