//! Edits and history steps on the sheet at a path, whether or not a window
//! shows it (#2172).
//!
//! Every other edit in the app goes through the active sheet. An agent names
//! its document, so these run on that sheet's engine and touch nothing of the
//! person's interaction state: not the active tab, focus, panels, selection,
//! tool, a drag or a text edit. What follows an edit is the document's own
//! derived state (dirty mark, render caches of windows that show it, the
//! panel context).

use std::path::Path;

use crate::app::Signex;
use crate::schematic_runtime::RenderInvalidation;

impl Signex {
    /// Run `command` on the engine of the sheet at `path`, as one undo step.
    /// `Ok(true)` when the sheet changed, `Ok(false)` when the command found
    /// nothing to change; `Err` carries the reason it was refused.
    pub(crate) fn execute_on_sheet(
        &mut self,
        path: &Path,
        command: signex_engine::Command,
    ) -> Result<bool, String> {
        let Some(engine) = self.document_state.engines.get_mut(path) else {
            return Err(format!("{} is not loaded", path.display()));
        };
        let executed = engine.execute(command);
        Self::report_engine_warnings(engine);
        let invalidation = match executed {
            Ok(result) if result.changed => result
                .patch_pair
                .map(|pair| Self::render_invalidation_for_patch(pair.document))
                .unwrap_or(RenderInvalidation::NONE),
            Ok(_) => return Ok(false),
            Err(error) => {
                let reason = error.to_string();
                crate::diagnostics::log_error("Engine command failed", &anyhow::Error::new(error));
                return Err(reason);
            }
        };
        self.after_sheet_change(path, invalidation);
        Ok(invalidation != RenderInvalidation::NONE)
    }

    /// One Undo (`undo`) or Redo of the sheet at `path`: its newest net-class
    /// pick if that is the newest thing on its timeline, else its engine's
    /// history. `Ok(false)` when there is nothing to step. A step that is part
    /// of a project-wide run (annotate, replace, re-prefix) is refused: it
    /// moves other sheets too, which only the run's own undo may do.
    pub(crate) fn step_sheet_on(&mut self, path: &Path, undo: bool) -> Result<bool, String> {
        if self.step_net_class_pick_on(path, undo) {
            self.after_sheet_change(path, RenderInvalidation::NET_COLORS);
            return Ok(true);
        }
        let Some(engine) = self.document_state.engines.get(path) else {
            return Err(format!("{} is not loaded", path.display()));
        };
        let group = if undo {
            engine.next_undo_group()
        } else {
            engine.next_redo_group()
        };
        if let Some(group) = group {
            return Err(
                if self.document_state.agent_edits.labels.contains_key(&group) {
                    "the next step is an agent's edit step; take it back with `revert`".to_string()
                } else {
                    "the next step belongs to a project-wide run (annotate, replace or re-prefix) \
                 and moves other sheets too; undo it from the sheet it was run on"
                        .to_string()
                },
            );
        }
        let Some(engine) = self.document_state.engines.get_mut(path) else {
            return Err(format!("{} is not loaded", path.display()));
        };
        let stepped = if undo { engine.undo() } else { engine.redo() };
        let invalidation = match stepped {
            Ok(Some(pair)) => Self::render_invalidation_for_patch(pair.document),
            Ok(None) => return Ok(false),
            Err(error) => {
                let reason = error.to_string();
                let failure = if undo {
                    "Engine undo failed"
                } else {
                    "Engine redo failed"
                };
                crate::diagnostics::log_error(failure, &anyhow::Error::new(error));
                return Err(reason);
            }
        };
        // Undo / redo can bring back an older embedded library symbol.
        self.library.drift.check_pending = true;
        self.after_sheet_change(path, invalidation);
        Ok(true)
    }

    /// What follows a change to the sheet at `path`: the document's dirty
    /// mark, ERC's epoch, the windows that show it, the panel context. When
    /// the sheet is also the main window's active one the same refresh keeps
    /// that canvas current.
    pub(crate) fn after_sheet_change(&mut self, path: &Path, invalidation: RenderInvalidation) {
        // Inside an agent's edit list the refresh is owed once, at its end.
        if self.document_state.agent_edits.target.as_deref() == Some(path) {
            self.document_state.agent_edits.owed |= invalidation;
            return;
        }
        let display_only = invalidation == RenderInvalidation::NET_COLORS;
        if !display_only {
            self.document_state.note_sheet_edit();
        }
        self.document_state.sync_history_dirty(path);
        let is_active = self.document_state.active_path.as_deref() == Some(path);
        self.refresh_slots_from_engine(path, is_active, invalidation, false);
        if is_active {
            self.prune_selected_wires();
            if !display_only {
                self.refresh_erc_after_edit();
            }
        }
        self.refresh_panel_ctx();
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
    /// window or an undocked one, that an agent's `revert` must not cut under:
    /// drawing, typing into an item of the sheet, or a move armed (#2178).
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
