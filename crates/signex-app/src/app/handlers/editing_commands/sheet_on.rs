//! Edits and history steps on the sheet at a path, whether or not a window
//! shows it (#2172).
//!
//! Every other edit in the app goes through the active sheet. An agent names
//! its document, so these run on that sheet's engine and touch nothing of the
//! person's interaction state: not the active tab, focus, panels, selection,
//! tool, a drag or a text edit. What follows an edit is the document's own
//! derived state (dirty mark, render caches of windows that show it, the
//! panel context).
//!
//! These are tail wrappers (#2704, until slice 6): the core half in
//! `state::sheet_change`, then the projection in `app::projector`.

use std::path::Path;

use signex_engine::DocumentPatch;

use crate::app::Signex;
use crate::app::core::event::{ChangeCause, DocumentChanged};
use crate::app::core::mutation_outcome::MutationOutcome;
use crate::app::projector::RefreshScope;

impl Signex {
    /// Run `command` on the engine of the sheet at `path`, as one undo step.
    /// `Ok(true)` when the sheet changed, `Ok(false)` when the command found
    /// nothing to change; `Err` carries the reason it was refused.
    pub(crate) fn execute_on_sheet(
        &mut self,
        path: &Path,
        command: signex_engine::Command,
    ) -> Result<bool, String> {
        let edit = self.document_state.execute_at(path, command);
        self.project_events(edit.events, RefreshScope::Sheet);
        match edit.refusal {
            Some(reason) => Err(reason),
            None => Ok(edit.outcome == MutationOutcome::Changed),
        }
    }

    /// One Undo (`undo`) or Redo of the sheet at `path`: its newest net-class
    /// pick if that is the newest thing on its timeline, else its engine's
    /// history. `Ok(false)` when there is nothing to step. A step that is part
    /// of a project-wide run (annotate, replace, re-prefix) is refused: it
    /// moves other sheets too, which only the run's own undo may do.
    pub(crate) fn step_sheet_on(&mut self, path: &Path, undo: bool) -> Result<bool, String> {
        if self.step_net_class_pick_on(path, undo) {
            self.after_sheet_change(path, DocumentPatch::NET_COLORS);
            return Ok(true);
        }
        let step = self.document_state.step_history_at(path, undo);
        self.project_events(step.events, RefreshScope::Sheet);
        match step.refusal {
            Some(reason) => Err(reason),
            None => Ok(step.stepped),
        }
    }

    /// What follows a change to the sheet at `path`: the document's dirty
    /// mark, ERC's epoch, the windows that show it, the panel context. When
    /// the sheet is also the main window's active one the same refresh keeps
    /// that canvas current.
    pub(crate) fn after_sheet_change(&mut self, path: &Path, patch: DocumentPatch) {
        let events = self
            .document_state
            .settle_sheet_at(path, patch, ChangeCause::Edit);
        self.project_events(events, RefreshScope::Sheet);
    }

    /// Repaint the undocked windows and tile pane showing `path`, a member of
    /// a multi-sheet run or grouped step other than the active sheet. Only
    /// the drawing: the caller keeps its own dirty mark and ERC epoch.
    pub(crate) fn repaint_member_sheet(&mut self, path: &Path, patch: DocumentPatch) {
        // Inside an agent's edit list the refresh is owed once, at its end.
        if self.document_state.agent_edits.target.as_deref() == Some(path) {
            self.document_state.agent_edits.owed |= patch;
            return;
        }
        let path = path.to_path_buf();
        let cause = ChangeCause::Edit;
        let changed = DocumentChanged { path, patch, cause };
        self.project_events(vec![changed.into()], RefreshScope::Member);
    }

    /// After a person's edit run through `on_edit_target` on `path`: when
    /// that sheet is the main window's active one, its selection overlay and
    /// the Properties panel's selection fields follow the edit, as the
    /// gateway's own refresh does for an edit of the active sheet.
    pub(crate) fn refresh_selection_on(&mut self, path: &Path) {
        if self.document_state.active_path.as_deref() != Some(path) {
            return;
        }
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        self.update_selection_info();
    }
}

impl Signex {
    /// Whether a wire, bus, arc, polyline or shape is being drawn on the
    /// sheet at `path`, which the main window shows.
    pub(crate) fn is_drawing_on(&self, path: &Path) -> bool {
        let interaction = &self.interaction_state;
        self.document_state.active_path.as_deref() == Some(path)
            && (interaction.wire_drawing
                || !interaction.arc_points.is_empty()
                || !interaction.polyline_points.is_empty()
                || interaction.shape_anchor.is_some())
    }

    /// What the person is in the middle of on the sheet at `path`, in the main
    /// window or an undocked one, that an agent's `revert` and an operand
    /// `undo` or `redo` must not cut under: drawing, typing into an item of
    /// the sheet, or a move armed (#2178, #2548).
    pub(crate) fn person_busy_on(&self, path: &Path) -> Option<&'static str> {
        let windows = &self.ui_state.windows;
        let undocked = self.interaction_state.canvases.iter().filter(|(id, _)| {
            matches!(
                windows.get(id),
                Some(crate::app::state::WindowKind::UndockedTab { path: shown, .. })
                    if shown == path
            )
        });
        let main_shows = self.document_state.active_path.as_deref() == Some(path);
        let slots: Vec<&crate::canvas::CanvasSlot> = undocked
            .map(|(_, slot)| slot)
            .chain(main_shows.then_some(&self.interaction_state.canvas))
            .collect();
        let drawing = |slot: &crate::canvas::CanvasSlot| {
            slot.drawing_mode
                || !slot.wire_preview.is_empty()
                || !slot.arc_points.is_empty()
                || !slot.polyline_points.is_empty()
                || slot.shape_anchor.is_some()
        };
        if self.is_drawing_on(path) || slots.iter().copied().any(drawing) {
            return Some("the person is drawing on this sheet; try again when the wire is done");
        }
        if self.text_edit_targets_sheet_at(path) {
            return Some("the person is typing into an item of this sheet; try again when done");
        }
        // A move is armed on the slot of the window it was armed from
        // (`arm_move_pick`); `ui_state.move_pick` is one for the whole app.
        if slots.iter().any(|slot| slot.move_pick.is_some()) {
            return Some("the person has a move armed on this sheet; try again when it is done");
        }
        None
    }
}
