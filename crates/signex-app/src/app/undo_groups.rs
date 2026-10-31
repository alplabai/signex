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
//! first. For redo, a sheet that still has the group in neither stack is
//! skipped (reopened or history trimmed); one that has the group deeper in its
//! undo stack but not on its redo stack (edited after the undo) refuses.
//!
//! A skipped sheet keeps the other half of a split multi-unit part, so every
//! one is named in a Messages panel warning, in both directions:
//! - open, but its history no longer holds the run (reopened, or trimmed past
//!   the history cap);
//! - closed since the run, so no engine holds its history;
//! - rewritten on disk by the run with no engine (undo only; it was never
//!   reverted, so a redo owes it nothing).

use super::*;

/// The sheets one undo group touched.
#[derive(Debug, Clone, Default)]
pub struct UndoGroupSheets {
    /// Sheets whose open engine recorded the group's entry.
    pub live: Vec<std::path::PathBuf>,
    /// Sheets rewritten on disk with no engine, so with no history to undo.
    pub disk_only: Vec<std::path::PathBuf>,
    /// Sheets the last undo of the group actually reverted. A redo requires
    /// exactly these, so a sheet the undo skipped (reopened, history trimmed)
    /// cannot block it, while one edited after the undo still does.
    pub undone: Vec<std::path::PathBuf>,
    /// What the group is, named in its undo and redo messages.
    pub kind: UndoGroupKind,
    /// Sheets the run loaded into tabless parked engines (re-prefix of a
    /// sheet that was not open). Back at their loaded state after an undo,
    /// they are clean, not unsaved.
    pub parked: Vec<std::path::PathBuf>,
    /// Parked sheets an undo took back to disk state and dropped. Their
    /// history is gone, so the group can no longer be redone as a whole.
    pub dropped: Vec<std::path::PathBuf>,
}

/// Which run stamped an undo group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UndoGroupKind {
    #[default]
    Annotate,
    Reprefix,
    Replace,
}

impl UndoGroupSheets {
    fn noun(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Annotate => "annotate",
            UndoGroupKind::Reprefix => "re-prefix",
            UndoGroupKind::Replace => "replace",
        }
    }

    fn title(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Annotate => "Annotate",
            UndoGroupKind::Reprefix => "Re-prefix",
            UndoGroupKind::Replace => "Replace",
        }
    }

    /// What a sheet the step skipped keeps: "keeps its ...".
    fn kept(&self) -> String {
        match self.kind {
            UndoGroupKind::Replace => "replaced text".to_string(),
            _ => format!("{} designators", self.participle()),
        }
    }

    /// What a sheet closed since the step holds: "its ... are whatever was
    /// last saved".
    fn holds(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Replace => "text",
            _ => "designators",
        }
    }

    /// What the run did to the designators, for "keeps its ... designators".
    fn participle(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Annotate => "annotated",
            UndoGroupKind::Reprefix => "re-prefixed",
            UndoGroupKind::Replace => "replaced",
        }
    }
}

fn file_names(paths: &[std::path::PathBuf]) -> String {
    paths
        .iter()
        .map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

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
}

impl Signex {
    /// Allocate a group id for a project-wide step.
    pub(super) fn mint_undo_group(&mut self) -> signex_engine::UndoGroupId {
        let id = signex_engine::UndoGroupId::new(self.document_state.next_undo_group);
        self.document_state.next_undo_group += 1;
        id
    }

    /// Record which sheets a group touched, once the run knows.
    pub(super) fn set_undo_group_sheets(
        &mut self,
        group: signex_engine::UndoGroupId,
        sheets: UndoGroupSheets,
    ) {
        if sheets.live.is_empty() && sheets.disk_only.is_empty() {
            self.document_state.undo_groups.remove(&group);
        } else {
            self.document_state.undo_groups.insert(group, sheets);
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
        let recorded = self
            .document_state
            .undo_groups
            .get(&group)
            .cloned()
            .unwrap_or_default();
        let (noun, title, done) = (recorded.noun(), recorded.title(), recorded.participle());
        let (kept, holds) = (recorded.kept(), recorded.holds());
        if direction == StepDirection::Redo && !recorded.dropped.is_empty() {
            // All or nothing: redoing the other sheets would leave one part
            // under two designators.
            crate::diagnostics::log_warning(format!(
                "Cannot redo the project-wide {noun}: {} went back to its saved state. \
                 Run {title} again.",
                file_names(&recorded.dropped),
            ));
            return false;
        }
        let mut sheets = recorded.live.clone();
        if !sheets.contains(&active_path) {
            sheets.push(active_path.clone());
        }
        sheets.sort();

        let mut members = Vec::new();
        let mut blocked = Vec::new();
        let mut history_gone = Vec::new();
        let mut closed = Vec::new();
        for path in sheets {
            let Some(engine) = self.document_state.engines.get(&path) else {
                // A redo owes nothing to a sheet the undo did not revert.
                if direction == StepDirection::Undo || recorded.undone.contains(&path) {
                    closed.push(path);
                }
                continue;
            };
            if direction.top(engine) == Some(group) {
                members.push(path);
                continue;
            }
            let expected = match direction {
                // Gone from this sheet's undo stack (reopened, or history
                // trimmed): nothing to revert, and it must not block the
                // others. A newer edit above a present entry is blocked.
                StepDirection::Undo => engine.has_undo_group(group),
                // Only the sheets the undo reverted can owe a redo.
                StepDirection::Redo => recorded.undone.contains(&path),
            };
            if expected {
                blocked.push(path);
            } else if direction == StepDirection::Undo {
                history_gone.push(path);
            }
        }
        if !blocked.is_empty() {
            let names = file_names(&blocked);
            let message = match direction {
                StepDirection::Undo => format!(
                    "Cannot undo the project-wide {noun}: {names} was edited after it. \
                     Undo the newer edit on {names} first."
                ),
                StepDirection::Redo => format!(
                    "Cannot redo the project-wide {noun}: {names} was edited after the \
                     undo, so the {noun} can't be redone across sheets."
                ),
            };
            crate::diagnostics::log_warning(message);
            return false;
        }
        let verb = match direction {
            StepDirection::Undo => "undone",
            StepDirection::Redo => "redone",
        };
        if !history_gone.is_empty() {
            crate::diagnostics::log_warning(format!(
                "{title} {verb} on the other sheets but not on {}: its undo history no \
                 longer holds the {noun} (reopened, or trimmed past the history limit), \
                 so it keeps its {kept}.",
                file_names(&history_gone),
            ));
        }
        if !closed.is_empty() {
            crate::diagnostics::log_warning(format!(
                "{title} {verb} on open sheets but not on {}: closed since the {noun}, \
                 so its {holds} are whatever was last saved.",
                file_names(&closed),
            ));
        }
        if direction == StepDirection::Undo && !recorded.disk_only.is_empty() {
            crate::diagnostics::log_warning(format!(
                "{title} undone on open sheets; not open, so still {done} on disk: {}",
                file_names(&recorded.disk_only),
            ));
        }

        let mut active_invalidation = None;
        let mut dropped_now = Vec::new();
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
                Ok(Some(_)) => {
                    // A parked sheet undone back to what was loaded from disk
                    // has nothing to save; redo marks it dirty again.
                    let back_at_disk = direction == StepDirection::Undo
                        && recorded.parked.contains(path)
                        && !engine.can_undo()
                        // A save of the re-prefixed bytes may still be landing;
                        // its `mark_saved` refuses on the revision mismatch, so
                        // clearing the mark now would lose this undo on disk.
                        && !self.document_state.document_saves.is_saving(path)
                        && !self.document_is_held_open(path);
                    if back_at_disk {
                        // A clean tabless engine would own the path and shadow
                        // the file; drop it so the next use re-reads the disk.
                        // Redo is then refused whole (`dropped`).
                        self.document_state.mark_document_clean(path);
                        self.forget_net_colors_of(path);
                        self.document_state.engines.remove(path);
                        dropped_now.push(path.clone());
                    } else {
                        self.document_state.mark_document_dirty(path);
                    }
                }
                Ok(None) => {}
                // `Engine::undo` / `redo` return `Err` never today, and the
                // pre-check above guarantees every member has an entry, so
                // this arm is unreachable. If either ever starts failing,
                // the sheets already stepped stay stepped; it is logged.
                Err(error) => {
                    debug_assert!(false, "group step failed after the pre-check: {error}");
                    let error = anyhow::Error::new(error);
                    crate::diagnostics::log_error("Engine group step failed", &error);
                }
            }
        }
        if direction == StepDirection::Undo
            && let Some(record) = self.document_state.undo_groups.get_mut(&group)
        {
            record.undone = members;
            record.dropped.extend(dropped_now);
        }
        // A member sheet other than the active one changed too.
        self.mark_erc_results_stale();
        match active_invalidation {
            Some(invalidation) => {
                self.finish_schematic_mutation(invalidation, true, update_selection_info)
            }
            None => false,
        }
    }
}
