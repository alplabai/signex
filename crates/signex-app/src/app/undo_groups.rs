//! Project-wide undo steps: one history entry per sheet, stamped with a shared
//! [`signex_engine::UndoGroupId`], undone and redone together.
//!
//! An annotate run under "Annotate undo: One step for the whole project"
//! stamps each touched sheet's entry with one group. Ctrl+Z / Ctrl+Y on any of
//! those sheets then moves every live sheet of the group in one step.
//!
//! Out-of-order rule: a grouped step moves only when every live sheet of the
//! group has the group's entry on top of the relevant stack. A sheet edited
//! after the run has a newer entry on top, so undoing the group would revert
//! the run underneath that edit; the step is refused instead, with a Messages
//! panel line naming the sheets, and nothing changes. Undo that newer edit
//! first (or run it per sheet). For redo, a sheet whose redo stack no longer
//! holds the group (it was edited after the undo) refuses the same way.
//! Sheets with no live engine (closed and clean, or rewritten on disk by the
//! run) have no history to move and are skipped.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StepDirection {
    Undo,
    Redo,
}

impl StepDirection {
    fn top(self, engine: &signex_engine::Engine) -> Option<signex_engine::UndoGroupId> {
        match self {
            StepDirection::Undo => engine.next_undo_group(),
            StepDirection::Redo => engine.next_redo_group(),
        }
    }

    fn verb(self) -> &'static str {
        match self {
            StepDirection::Undo => "undo",
            StepDirection::Redo => "redo",
        }
    }
}

impl Signex {
    /// Allocate a group id for a project-wide step touching `paths`.
    pub(super) fn mint_undo_group(
        &mut self,
        paths: Vec<std::path::PathBuf>,
    ) -> signex_engine::UndoGroupId {
        let id = self.document_state.next_undo_group;
        self.document_state.next_undo_group += 1;
        self.document_state.undo_groups.insert(id, paths);
        id
    }

    /// Record which sheets a group touched, once the run knows.
    pub(super) fn set_undo_group_sheets(
        &mut self,
        group: signex_engine::UndoGroupId,
        paths: Vec<std::path::PathBuf>,
    ) {
        if paths.is_empty() {
            self.document_state.undo_groups.remove(&group);
        } else {
            self.document_state.undo_groups.insert(group, paths);
        }
    }

    /// Undo or redo the grouped entry on top of the active engine across every
    /// live sheet of the group. Returns whether anything moved.
    pub(super) fn apply_grouped_step(
        &mut self,
        group: signex_engine::UndoGroupId,
        direction: StepDirection,
        update_selection_info: bool,
    ) -> bool {
        let Some(active_path) = self.document_state.active_path.clone() else {
            return false;
        };
        let mut sheets = self
            .document_state
            .undo_groups
            .get(&group)
            .cloned()
            .unwrap_or_default();
        if !sheets.contains(&active_path) {
            sheets.push(active_path.clone());
        }
        sheets.sort();

        let mut members = Vec::new();
        let mut blocked = Vec::new();
        for path in sheets {
            let Some(engine) = self.document_state.engines.get(&path) else {
                continue;
            };
            if direction.top(engine) == Some(group) {
                members.push(path);
            } else if direction == StepDirection::Undo && !engine.has_undo_group(group) {
                // The group's entry is gone from this sheet (history trimmed
                // or reloaded): nothing to revert there. A newer edit on top
                // of a still-present entry is the blocked case.
            } else {
                blocked.push(path);
            }
        }
        if !blocked.is_empty() {
            let names: Vec<String> = blocked
                .iter()
                .map(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                })
                .collect();
            crate::diagnostics::log_warning(format!(
                "Cannot {} the project-wide step: {} was edited since, so reverting \
                 it would run out of order. Undo that sheet's newer edit first, or \
                 set Preferences > Annotation > Annotate undo to Per sheet.",
                direction.verb(),
                names.join(", "),
            ));
            return false;
        }

        let mut active_invalidation = None;
        for path in &members {
            let Some(engine) = self.document_state.engines.get_mut(path) else {
                continue;
            };
            let result = match direction {
                StepDirection::Undo => engine.undo(),
                StepDirection::Redo => engine.redo(),
            };
            match result {
                Ok(Some(patch_pair)) if *path == active_path => {
                    active_invalidation =
                        Some(Self::render_invalidation_for_patch(patch_pair.document));
                }
                Ok(Some(_)) => self.mark_cached_sheet_dirty(path),
                Ok(None) => {}
                Err(error) => {
                    let error = anyhow::Error::new(error);
                    crate::diagnostics::log_error("Engine group step failed", &error);
                }
            }
        }
        match active_invalidation {
            Some(invalidation) => {
                self.finish_schematic_mutation(invalidation, true, update_selection_info)
            }
            None => false,
        }
    }
}
