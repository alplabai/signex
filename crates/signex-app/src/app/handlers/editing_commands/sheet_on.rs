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
        let Some(engine) = self.document_state.engines.get_mut(path) else {
            return Err(format!("{} is not loaded", path.display()));
        };
        let group = if undo {
            engine.next_undo_group()
        } else {
            engine.next_redo_group()
        };
        if group.is_some() {
            return Err(
                "the next step belongs to a project-wide run (annotate, replace or re-prefix) \
                 and moves other sheets too; undo it from the sheet it was run on"
                    .to_string(),
            );
        }
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
    fn after_sheet_change(&mut self, path: &Path, invalidation: RenderInvalidation) {
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
