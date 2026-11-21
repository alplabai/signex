//! The clipboard on the board in view (#2361): Select All, Copy, Cut, Paste,
//! Paste Special and Duplicate. The copy is a
//! `signex_engine::pcb::BoardClipboard` in `InteractionState::pcb_clipboard`;
//! a cut, a paste and a duplicate are each one `execute_board_commands` step.
//!
//! One clipboard serves sheets and boards: a copy on a board clears the
//! sheet copy (and empties the system clipboard, which holds no board
//! fragment yet), a copy on a sheet clears the board copy, and a paste of
//! the other kind is refused with the reason in the Messages panel.

use iced::Task;
use pcb_geom::{BoardIndex, ItemRef};
use signex_engine::pcb::placement::{self, grid_step_mm};
use signex_engine::pcb::{BoardClipboard, BoardItemId};
use signex_types::pcb::Point;
use uuid::Uuid;

use crate::app::handlers::clipboard_workflows::{CopyOutcome, DEFAULT_PASTE_OFFSET_MM};
use crate::app::handlers::pcb_placement::deletable_items;
use crate::app::{Message, Signex};
use crate::pcb_canvas::{CarryKind, PasteLoad, PcbCarry, snap_point_on};

impl Signex {
    /// Select All on a board. Whether the active tab is one.
    pub(crate) fn select_all_on_pcb(&mut self) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        if self.with_pcb_selection(|state, board| state.select_all(board)) == Some(true) {
            self.refresh_pcb_highlight();
        }
        true
    }

    /// Copy on a board: the task that mirrors it on the system clipboard,
    /// or `None` when the active tab is not a board.
    pub(crate) fn copy_pcb_selection(&mut self) -> Option<Task<Message>> {
        if !self.has_active_pcb() {
            return None;
        }
        Some(match self.copy_on_board() {
            CopyOutcome::Carried => board_copy_written(),
            CopyOutcome::NothingToCarry | CopyOutcome::Failed => Task::none(),
        })
    }

    /// Cut on a board: copy, then delete the copied items as one step. A
    /// locked footprint refuses the whole cut before anything is copied.
    /// `None` when the active tab is not a board.
    pub(crate) fn cut_pcb_selection(&mut self) -> Option<Task<Message>> {
        if !self.has_active_pcb() {
            return None;
        }
        let items = self.pcb_copy_items();
        let (Some(board), Some(path)) = (self.active_pcb(), self.active_tab_path()) else {
            return Some(Task::none());
        };
        let commands = match placement::delete_commands(board, items) {
            Ok(commands) => commands,
            Err(refusal) => {
                crate::diagnostics::log_warning(format!("Cut: {refusal}; nothing changed"));
                return Some(Task::none());
            }
        };
        let before = (
            self.interaction_state.pcb_clipboard.clone(),
            self.interaction_state.clipboard.clone(),
            self.interaction_state.clipboard_text.clone(),
        );
        if self.copy_on_board() != CopyOutcome::Carried {
            crate::diagnostics::log_warning(
                "Cut cancelled: the selection could not be copied, so nothing was deleted.",
            );
            return Some(Task::none());
        }
        match self.execute_board_commands(&path, commands, None) {
            Ok(_) => {
                self.interaction_state.pcb_selection.clear_selection();
                self.refresh_pcb_highlight();
                Some(board_copy_written())
            }
            Err(error) => {
                crate::diagnostics::log_warning(format!(
                    "Cut cancelled: {error}; nothing was deleted or copied."
                ));
                let (board_copy, sheet_copy, text) = before;
                self.interaction_state.pcb_clipboard = board_copy;
                self.interaction_state.clipboard = sheet_copy;
                self.interaction_state.clipboard_text = text;
                Some(Task::none())
            }
        }
    }

    /// Paste on a board: the copy follows the pointer on the grid until a
    /// click places it. Whether the active tab is a board.
    pub(crate) fn paste_on_pcb(&mut self) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        let Some(copy) = self.board_paste_source("Paste") else {
            return true;
        };
        let (Some(board), Some(path)) = (self.active_pcb(), self.active_tab_path()) else {
            return true;
        };
        let ghost: Vec<pcb_geom::Shape> = BoardIndex::build_selectable(&copy.preview_board(board))
            .items()
            .map(|indexed| indexed.shape.clone())
            .collect();
        let start = self.board_cursor(&path).unwrap_or(copy.origin());
        let view = self.pcb_snap_view().without_objects();
        let at = snap_point_on(board, start, copy.grid_use(), view);
        let window = self.interaction_state.acting_window;
        let load = PasteLoad { copy, ghost };
        // A paste on the cursor replaces a placement tool (#2373) and
        // Measure, which would take its click (#2403).
        self.release_board_tool();
        self.interaction_state.pcb_query.measure = None;
        self.interaction_state.pcb_carry = Some(PcbCarry::paste(path, window, load, at));
        crate::diagnostics::log_info("Paste: click where the copy should go (Esc to cancel)");
        self.refresh_pcb_highlight();
        true
    }

    /// Paste Special on a board: the copy is placed at once beside where it
    /// was copied, as on a sheet. Whether the active tab is a board.
    pub(crate) fn paste_special_on_pcb(&mut self) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        if let Some(copy) = self.board_paste_source("Paste Special") {
            self.paste_beside("Paste Special", &copy);
        }
        true
    }

    /// Duplicate on a board: copy the selection (the copy is the clipboard,
    /// as for any Copy) and place it at once beside itself. `None` when the
    /// active tab is not a board.
    pub(crate) fn duplicate_pcb_selection(&mut self) -> Option<Task<Message>> {
        if !self.has_active_pcb() {
            return None;
        }
        if self.copy_on_board() != CopyOutcome::Carried {
            return Some(Task::none());
        }
        if let Some(copy) = self.interaction_state.pcb_clipboard.clone() {
            self.paste_beside("Duplicate", &copy);
        }
        Some(board_copy_written())
    }

    /// The pointer moved while a paste is on the cursor: follow it.
    pub(crate) fn pcb_paste_followed(&mut self, at: Point) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let Some(load) = self
            .live_carry(CarryKind::Paste)
            .and_then(|carry| carry.paste)
        else {
            return;
        };
        let view = self.pcb_snap_view().without_objects();
        let to = snap_point_on(board, at, load.copy.grid_use(), view);
        if let Some(carry) = self.interaction_state.pcb_carry.as_mut()
            && carry.delta != to
        {
            carry.delta = to;
            self.refresh_pcb_highlight();
        }
    }

    /// A click while a paste is on the cursor places it there as one step.
    /// Whether the click belonged to the paste.
    pub(crate) fn pcb_paste_clicked(&mut self, at: Point) -> bool {
        let Some(load) = self
            .live_carry(CarryKind::Paste)
            .and_then(|carry| carry.paste)
        else {
            return false;
        };
        self.end_pcb_carry();
        if let Some(board) = self.active_pcb() {
            let view = self.pcb_snap_view().without_objects();
            let to = snap_point_on(board, at, load.copy.grid_use(), view);
            self.place_board_copy("Paste", &load.copy, to);
        }
        true
    }

    /// Copy the board selection to the in-app clipboard; see [`CopyOutcome`].
    fn copy_on_board(&mut self) -> CopyOutcome {
        let items = self.pcb_copy_items();
        let Some(board) = self.active_pcb() else {
            return CopyOutcome::Failed;
        };
        if items.is_empty() {
            return CopyOutcome::Failed;
        }
        match BoardClipboard::copy(board, &items, grid_step_mm(board)) {
            Ok(Some(copy)) => {
                let interaction = &mut self.interaction_state;
                interaction.pcb_clipboard = Some(copy);
                interaction.clipboard = None;
                interaction.clipboard_text = None;
                CopyOutcome::Carried
            }
            Ok(None) => CopyOutcome::NothingToCarry,
            Err(error) => {
                crate::diagnostics::log_warning(format!("Copy failed: {error}."));
                CopyOutcome::Failed
            }
        }
    }

    /// What Copy carries of the board selection: footprints (a selected
    /// pad's included), tracks and vias; the board copy leaves drawn areas
    /// out.
    fn pcb_copy_items(&mut self) -> Vec<BoardItemId> {
        self.with_pcb_selection(|state, _| deletable_items(state.selection()))
            .unwrap_or_default()
    }

    /// The board copy to paste, or `None` with the reason in the Messages
    /// panel.
    fn board_paste_source(&self, what: &str) -> Option<BoardClipboard> {
        if let Some(copy) = &self.interaction_state.pcb_clipboard {
            return Some(copy.clone());
        }
        let reason = if self.interaction_state.clipboard.is_some() {
            "the clipboard holds schematic objects, which cannot be pasted on a board"
        } else {
            "nothing has been copied from a board"
        };
        crate::diagnostics::log_warning(format!("{what}: {reason}; nothing was pasted."));
        None
    }

    /// Place `copy` at once, shifted by the Duplicate offset from where it
    /// was copied and put on the grid.
    fn paste_beside(&mut self, what: &str, copy: &BoardClipboard) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let origin = copy.origin();
        let shifted = Point::new(
            origin.x + DEFAULT_PASTE_OFFSET_MM,
            origin.y + DEFAULT_PASTE_OFFSET_MM,
        );
        let at = placement::grid_for(board, copy.grid_use()).snap(shifted);
        self.place_board_copy(what, copy, at);
    }

    /// Add `copy` to the board in view with its origin at `at` as one step,
    /// and select what was added. Designators follow the paste preference,
    /// as on a sheet.
    fn place_board_copy(&mut self, what: &str, copy: &BoardClipboard, at: Point) {
        let (Some(board), Some(path)) = (self.active_pcb(), self.active_tab_path()) else {
            return;
        };
        let mut numbering = self.board_paste_numbering(board);
        let mut rename = |old: &str| numbering.rename(old);
        let (command, added) = copy.paste_command(board, at, &mut Uuid::now_v7, &mut rename);
        match self.execute_board_commands(&path, vec![command], None) {
            Ok(_) => {
                let picked: Vec<ItemRef> = added.into_iter().filter_map(item_ref).collect();
                self.select_pcb_items(&picked);
                self.refresh_pcb_highlight();
            }
            Err(error) => {
                crate::diagnostics::log_warning(format!("{what}: {error}; nothing changed"));
            }
        }
    }

    /// The pointer's last board position, when it was over the board at
    /// `path`.
    fn board_cursor(&self, path: &std::path::Path) -> Option<Point> {
        let ui = &self.ui_state;
        (ui.cursor_document.as_deref() == Some(path))
            .then(|| Point::new(ui.cursor_x, ui.cursor_y))
            .filter(|p| p.x.is_finite() && p.y.is_finite())
    }
}

/// The system-clipboard side of a board copy: emptied, so a sheet fragment
/// left there cannot be pasted in its place.
fn board_copy_written() -> Task<Message> {
    iced::clipboard::write(String::new()).discard()
}

fn item_ref(id: BoardItemId) -> Option<ItemRef> {
    match id {
        BoardItemId::Footprint(id) => Some(ItemRef::Footprint(id)),
        BoardItemId::Segment(id) => Some(ItemRef::Track(id)),
        BoardItemId::Via(id) => Some(ItemRef::Via(id)),
        BoardItemId::Text(id) => Some(ItemRef::Text(id)),
        BoardItemId::Arc(id) => Some(ItemRef::Arc(id)),
        BoardItemId::FreePad(id) => Some(ItemRef::FreePad(id)),
        BoardItemId::Dimension(id) => Some(ItemRef::Dimension(id)),
        BoardItemId::Zone(_) | BoardItemId::Graphic(_) => None,
    }
}
