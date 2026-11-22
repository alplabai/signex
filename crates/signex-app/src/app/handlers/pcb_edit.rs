//! Board edits, undo and redo as the app runs them (#661 slice 7).
//!
//! This is the board's mutation gateway, as `mutation_gateway.rs` is the
//! sheet's: no handler writes to a `PcbBoard`, every edit is a
//! `BoardCommand` run by the board's engine (`state/pcb_edit.rs`), and this
//! file repaints what showed the old board.

use std::path::Path;

use signex_engine::pcb::{BoardCommand, BoardPatch};

use crate::app::Signex;
use crate::app::state::pcb_edit::{BoardEditError, CheckedStep};

impl Signex {
    /// Run `commands` on the board at `path` as one undo step, all or
    /// nothing. With `checked`, the step is labelled and refused when the board
    /// moved past the revision it was built from.
    pub fn execute_board_commands(
        &mut self,
        path: &Path,
        commands: Vec<BoardCommand>,
        checked: Option<CheckedStep<'_>>,
    ) -> Result<Vec<BoardPatch>, BoardEditError> {
        let recorded = self.board_edit_to_record(path, &commands);
        let patches = match self.document_state.execute_board(path, commands, checked) {
            Ok(patches) => patches,
            Err(error) => {
                self.ui_state.command_replay.forget_board_marks();
                return Err(error);
            }
        };
        if patches.is_empty() {
            self.ui_state.command_replay.forget_board_marks();
        } else {
            self.after_board_change(path);
            if let Some(record) = recorded {
                self.note_board_edit(path, record);
            }
        }
        Ok(patches)
    }

    /// [`Self::execute_board_commands`] as a step of undo group `group`.
    pub(crate) fn execute_board_commands_grouped(
        &mut self,
        path: &Path,
        commands: Vec<BoardCommand>,
        step: CheckedStep<'_>,
        group: signex_engine::UndoGroupId,
    ) -> Result<Vec<BoardPatch>, BoardEditError> {
        let patches = self
            .document_state
            .execute_board_grouped(path, commands, step, group)?;
        if !patches.is_empty() {
            self.after_board_change(path);
        }
        Ok(patches)
    }

    /// One undo (`undo`) or redo of the board at `path`. Whether it moved; a
    /// refusal is logged and reads as not moved. A step of an undo group
    /// moves with the sheet steps of that group (#2366).
    pub(crate) fn step_board_history(&mut self, path: &Path, undo: bool) -> bool {
        let group = self.document_state.board_engine(path).and_then(|engine| {
            if undo {
                engine.next_undo_group()
            } else {
                engine.next_redo_group()
            }
        });
        if let Some(group) = group.filter(|g| self.document_state.undo_groups.contains_key(g)) {
            return self.step_board_undo_group(group, undo);
        }
        self.step_board_once(path, undo)
    }

    /// One undo or redo of the board at `path` alone.
    pub(crate) fn step_board_once(&mut self, path: &Path, undo: bool) -> bool {
        let before = self.drc_waiver_step_start(path);
        let result = if undo {
            self.document_state.undo_board(path)
        } else {
            self.document_state.redo_board(path)
        };
        match result {
            Ok(true) => {
                self.carry_drc_over_waiver_step(path, before);
                self.drop_pcb_footprint_draft_of(path);
                self.interaction_state.pcb_primitive.drop_drafts_of(path);
                self.after_board_change(path);
                true
            }
            Ok(false) => false,
            Err(error) => {
                crate::diagnostics::log_warning(error.to_string());
                false
            }
        }
    }

    /// The board at `path` changed: repaint every window that shows it. The
    /// main window's canvas, ratsnest and pick index follow the revision when
    /// it is the active tab; an undocked window keeps its own camera and
    /// repaints its own selection. This repaints only: it does not touch the
    /// tool or the placement in progress.
    fn after_board_change(&mut self, path: &Path) {
        let active = self.active_tab_path().as_deref() == Some(path);
        if active {
            // Also repaints the undocked windows that show the active tab.
            self.sync_pcb_canvas_from_visible_board();
            let canvas = &mut self.interaction_state.pcb_canvas;
            canvas.clear_bg_cache();
            canvas.clear_content_cache();
            self.sync_panel_ctx_from_visible_document();
            self.refresh_pcb_layers_panel();
            self.document_state.panel_ctx.pcb_nets = self.pcb_nets_summary();
            self.document_state.panel_ctx.layer_stack = self.layer_stack_facts();
        } else {
            self.repaint_undocked_boards(path);
        }
        // The DRC panel says its results are stale as soon as the board
        // changes (#2399).
        self.document_state.panel_ctx.pcb_drc = self.pcb_drc_panel();
        // The undocked windows' selections follow the new geometry too (#2240).
        self.refresh_undocked_pcb_highlights(path);
        // So do the panels of a window showing this board (net table, Layer
        // Stack): each keeps its own context. No-op inside a window's swap,
        // where the branch above already refreshed that window's context.
        self.refresh_window_panel_ctxs(None);
    }

    /// Give each undocked window, and the unfocused tile pane (#2249), that
    /// shows the board at `path` its new geometry.
    pub(crate) fn repaint_undocked_boards(&mut self, path: &Path) {
        let Some(board) = self.document_state.pcb_board(path) else {
            return;
        };
        let snapshot = self.undocked_pcb_snapshot(path, board);
        let board_grid = Some(signex_engine::pcb::placement::grid_step_mm(board));
        let view = self.pcb_layer_view(path);
        let (origin_marker, flipped) = (view.origin_marker_visible(), view.flipped());
        let marks = crate::pcb_canvas::BoardMarks::of(board);
        let windows = &self.ui_state.windows;
        let pane = self.interaction_state.tile_panes.board.get_mut(path);
        let canvases = self
            .interaction_state
            .pcb_canvases
            .iter_mut()
            .filter(|(id, _)| {
                matches!(
                    windows.get(id),
                    Some(crate::app::state::WindowKind::UndockedTab { path: shown, .. })
                        if shown == path
                )
            })
            .map(|(_, canvas)| canvas)
            .chain(pane);
        for canvas in canvases {
            canvas.set_renderer_snapshot(Some(snapshot.clone()));
            canvas.set_board_grid(board_grid);
            canvas.set_board_marks(marks.clone());
            canvas.set_origin_marker_visible(origin_marker);
            canvas.set_view_flipped(flipped);
            canvas.clear_content_cache();
        }
    }
}
