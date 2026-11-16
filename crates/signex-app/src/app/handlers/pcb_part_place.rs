//! A library part placed on a board from the Components panel (#2364).
//!
//! Place on a board tab (row menu, picker, a row dragged onto the board)
//! reads the row and its footprint off the update thread
//! ([`resolve_board_part`]); the completion arms a Place carry: the footprint
//! follows the pointer on the board's grid, drawn by the board renderer and
//! dimmed. Space turns it, Y or Flip Selection moves it to the other side,
//! and each click places a copy as one undo step with the next designator.
//! Esc, a right-click, a tab switch or a newer pick ends it. A row with no
//! footprint, or one the board cannot hold, is refused with why in the
//! Messages panel and nothing is armed. The domain half is
//! `signex_engine::pcb::part_place`.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::part_place::PartToPlace;
use signex_engine::pcb::placement::grid_for;
use signex_i18n::{ids, t};
use signex_library::RowId;
use signex_library::primitive::PrimitiveRef;
use signex_renderer::pcb::PcbSnapshot;
use signex_types::pcb::{PcbBoard, Point};

use crate::app::{Message, Signex};
use crate::library::messages::{BoardPartError, LibraryMessage, ResolvedBoardPart};
use crate::library::place::resolve_board_part;
use crate::pcb_canvas::{CarryKind, PcbCarry};

impl Signex {
    /// Start placing the row on the board in view: read it off the update
    /// thread, then put its footprint on the cursor.
    pub(crate) fn start_board_part_placement(
        &mut self,
        library_path: PathBuf,
        table: String,
        row_id: RowId,
        footprint: Option<PrimitiveRef>,
    ) -> Task<Message> {
        let Some(board) = self.active_tab_path() else {
            return Task::none();
        };
        if self.library.library_at(&library_path).is_none() {
            self.report_place_failure(crate::app::state::untranslated(format!(
                "Library {} is no longer open.",
                library_path.display()
            )));
            return Task::none();
        }
        let arm_epoch = self.interaction_state.placement_epoch;
        let window = self.interaction_state.acting_window;
        let open_libraries: Vec<(uuid::Uuid, PathBuf)> = self
            .library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    resolve_board_part(&library_path, &table, row_id, footprint, &open_libraries)
                })
                .await
                .unwrap_or_else(|e| Err(BoardPartError::Read(format!("place worker failed: {e}"))))
            },
            move |result| {
                Message::Library(LibraryMessage::BoardPartResolved {
                    board: board.clone(),
                    window,
                    arm_epoch,
                    result: result.map(Box::new),
                })
            },
        )
    }

    /// Completion of [`Self::start_board_part_placement`]: put the part on
    /// the cursor of the board it was picked for, or say why not.
    pub(crate) fn handle_board_part_resolved(
        &mut self,
        board: PathBuf,
        window: Option<iced::window::Id>,
        arm_epoch: u64,
        result: Result<Box<ResolvedBoardPart>, BoardPartError>,
    ) -> Task<Message> {
        // A read started in an undocked window arms there.
        if let Some(id) = window.filter(|id| self.interaction_state.acting_window != Some(*id)) {
            return self.run_in_window(id, false, move |app| {
                app.handle_board_part_resolved(board, window, arm_epoch, result)
            });
        }
        if arm_epoch != self.interaction_state.placement_epoch {
            // Esc, a tool change or a newer pick came in meanwhile.
            return Task::none();
        }
        if let Err(detail) = self.arm_board_part(&board, result) {
            self.report_place_failure(crate::app::state::untranslated(detail));
        }
        Task::none()
    }

    fn arm_board_part(
        &mut self,
        board: &Path,
        result: Result<Box<ResolvedBoardPart>, BoardPartError>,
    ) -> Result<(), String> {
        let i18n = &self.ui_state.i18n;
        let refused = |part: &str, reason: &str| {
            t!(i18n, ids::PCB_PLACE_PART_REFUSED, "part" => part, "reason" => reason).into_string()
        };
        let resolved = match result {
            Ok(resolved) => *resolved,
            Err(BoardPartError::Read(detail)) => return Err(detail),
            Err(BoardPartError::Refused { part, reason }) => return Err(refused(&part, &reason)),
        };
        let pn = resolved.placement.internal_pn.clone();
        if self.active_tab_path().as_deref() != Some(board) || !self.has_active_pcb() {
            let board = board.display().to_string();
            return Err(t!(
                self.ui_state.i18n,
                ids::PCB_PLACE_PART_BOARD_GONE,
                "board" => board.as_str(),
                "part" => pn.as_str()
            )
            .into_string());
        }
        let prefix = self.placement_prefix_for(board, &resolved.placement);
        let part = PartToPlace::new(resolved.footprint, prefix, resolved.placement.value)
            .map_err(|why| refused(&pn, &why.to_string()))?;
        crate::diagnostics::log_info(
            t!(
                self.ui_state.i18n,
                ids::PCB_PLACE_PART_HINT,
                "part" => pn.as_str(),
                "footprint" => part.footprint_name()
            )
            .into_string(),
        );
        let window = self.interaction_state.acting_window;
        self.interaction_state.pcb_tool = None;
        // Measure would take the placement's clicks (#2403).
        self.interaction_state.pcb_query.measure = None;
        self.interaction_state.pcb_carry = Some(PcbCarry::place(board.to_path_buf(), window, part));
        self.refresh_pcb_highlight();
        Ok(())
    }

    /// The Place carry of the board this window shows.
    fn own_place_carry(&self) -> Option<&PcbCarry> {
        self.interaction_state
            .pcb_carry
            .as_ref()
            .filter(|carry| carry.kind == CarryKind::Place && self.has_own_pcb_carry())
    }

    /// Whether a library part is on the cursor of the board in view.
    pub(crate) fn has_pcb_part_in_hand(&self) -> bool {
        self.own_place_carry().is_some()
    }

    /// The pointer moved over the board: the part follows it on the grid.
    pub(crate) fn pcb_place_followed(&mut self, at: Point) {
        let (Some(_), Some(board)) = (self.own_place_carry(), self.active_pcb()) else {
            return;
        };
        let at = on_grid(board, at);
        if let Some(carry) = self.interaction_state.pcb_carry.as_mut()
            && carry.anchor != Some(at)
        {
            carry.anchor = Some(at);
            self.refresh_pcb_highlight();
        }
    }

    /// A click on the board while a part is on the cursor: place a copy
    /// there as one undo step and keep the part for the next one. Whether
    /// the click belonged to the placement.
    pub(crate) fn pcb_place_clicked(&mut self, at: Point) -> bool {
        let (Some(carry), Some(board)) = (self.own_place_carry(), self.active_pcb()) else {
            return false;
        };
        let Some(part) = carry.part.as_deref() else {
            return false;
        };
        let at = on_grid(board, at);
        let command = part.place_command(board, uuid::Uuid::now_v7(), at);
        self.run_placement("Place", Ok(vec![command]));
        if let Some(carry) = self.interaction_state.pcb_carry.as_mut() {
            carry.anchor = Some(at);
        }
        // The next copy shows the next designator.
        self.refresh_pcb_highlight();
        true
    }

    /// A row dropped on the board: the part on the cursor lands where it
    /// shows and the placement ends. A read still in flight arms the part
    /// on the cursor when it lands, for a click to place.
    pub(crate) fn drop_pcb_part(&mut self) {
        let Some(at) = self.own_place_carry().and_then(|carry| carry.anchor) else {
            return;
        };
        self.pcb_place_clicked(at);
        self.end_pcb_carry();
    }

    /// Space: turn the part on the cursor a quarter. `false` when none is.
    pub(crate) fn rotate_pcb_part(&mut self, clockwise: bool) -> bool {
        // The board's y axis points down the screen: +90° is clockwise.
        let quarter_turns = if clockwise { 1 } else { -1 };
        self.turn_pcb_part(|part| part.rotate(quarter_turns))
    }

    /// Y or Flip Selection: move the part on the cursor to the other side.
    pub(crate) fn flip_pcb_part(&mut self) -> bool {
        self.turn_pcb_part(PartToPlace::flip)
    }

    /// X: mirror the part on the cursor top to bottom, a flip and a half
    /// turn, as for placed footprints.
    pub(crate) fn mirror_pcb_part_top_bottom(&mut self) -> bool {
        self.turn_pcb_part(|part| {
            part.flip();
            part.rotate(2);
        })
    }

    fn turn_pcb_part(&mut self, turn: impl FnOnce(&mut PartToPlace)) -> bool {
        if self.own_place_carry().is_none() {
            return false;
        }
        if let Some(part) = self
            .interaction_state
            .pcb_carry
            .as_mut()
            .and_then(|carry| carry.part.as_deref_mut())
        {
            turn(part);
        }
        self.refresh_pcb_highlight();
        true
    }

    /// What the canvas of `carry`'s board draws on the cursor: the board
    /// holding only the part where the next click puts it, as the view of
    /// that board shows its layers.
    pub(crate) fn pcb_part_ghost(&self, carry: Option<&PcbCarry>) -> Option<PcbSnapshot> {
        let carry = carry?;
        let part = carry.part.as_deref()?;
        let at = carry.anchor?;
        let board = self.document_state.pcb_board(&carry.path)?;
        Some(self.pcb_snapshot_for(&carry.path, &part.preview_board(board, at)))
    }
}

/// `at` moved to the nearest point of `board`'s grid.
fn on_grid(board: &PcbBoard, at: Point) -> Point {
    grid_for(board, signex_types::pcb_options::GridUse::Components).snap(at)
}

#[cfg(test)]
#[path = "pcb_part_place_tests.rs"]
mod tests;
