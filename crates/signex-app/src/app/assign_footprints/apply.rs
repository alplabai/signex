//! Apply: every picked footprint, on every open sheet, as one undo group.
//!
//! Each change is checked again against the part's live document and its own
//! library row (`footprint_binding_commands`, the Properties path). All sheets
//! change in one `update`; a sheet that refuses puts every earlier one back,
//! so nothing is left half applied. Only open sheets are changed and nothing
//! is written to disk; the user saves.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::state::AssignProblem;
use crate::app::Signex;

/// One sheet Apply changed, before the whole run is committed.
struct Changed {
    path: PathBuf,
    parts: usize,
    invalidation: Option<crate::schematic_runtime::RenderInvalidation>,
}

impl Signex {
    /// Apply the dialog's picks. On success the dialog closes; on a refusal
    /// it stays open with the reason, and no sheet is changed.
    pub(super) fn apply_footprint_assignments(&mut self) {
        let Some(state) = self.ui_state.assign_footprints.as_ref() else {
            return;
        };
        if !state.can_apply() {
            return;
        }
        let mut by_sheet: BTreeMap<PathBuf, Vec<(uuid::Uuid, String)>> = BTreeMap::new();
        for (index, binding) in state.changes() {
            let part = &state.parts()[index];
            by_sheet
                .entry(part.sheet.clone())
                .or_default()
                .push((part.symbol_id, binding));
        }
        match self.assign_across_sheets(by_sheet) {
            Ok(()) => self.ui_state.assign_footprints = None,
            Err(problem) => {
                crate::diagnostics::log_warning(format!(
                    "Assign Footprints: {}",
                    problem.english()
                ));
                if let Some(state) = self.ui_state.assign_footprints.as_mut() {
                    state.error = Some(problem);
                }
            }
        }
    }

    /// Change every sheet of `by_sheet`, all or nothing.
    fn assign_across_sheets(
        &mut self,
        by_sheet: BTreeMap<PathBuf, Vec<(uuid::Uuid, String)>>,
    ) -> Result<(), AssignProblem> {
        // Build every sheet's commands before any sheet changes.
        let mut planned = Vec::new();
        for (path, picks) in by_sheet {
            let Some(engine) = self.document_state.engines.get(&path) else {
                return Err(AssignProblem::SheetClosed(path));
            };
            let doc = engine.document();
            if doc.unknown.newer_minor_source.is_some() {
                return Err(AssignProblem::ReadOnly(path));
            }
            let mut commands = Vec::new();
            for (symbol_id, binding) in &picks {
                let mine = self
                    .footprint_binding_commands(doc, *symbol_id, binding)
                    .map_err(|refusal| AssignProblem::Refused {
                        reference: refusal.reference,
                        why: refusal.why.to_string(),
                    })?;
                commands.extend(mine);
            }
            planned.push((path, picks.len(), commands));
        }

        let group = self.mint_undo_group();
        let mut changed: Vec<Changed> = Vec::new();
        for (path, parts, commands) in planned {
            let Some(engine) = self.document_state.engines.get_mut(&path) else {
                continue;
            };
            let outcome = engine.execute_batch_grouped(commands, Some(group));
            for warning in engine.take_warnings() {
                crate::diagnostics::log_warning(warning);
            }
            match outcome {
                Ok(result) if result.changed => changed.push(Changed {
                    path,
                    parts,
                    invalidation: result
                        .patch_pair
                        .map(|pair| Self::render_invalidation_for_patch(pair.document)),
                }),
                Ok(_) => {}
                Err(error) => {
                    crate::diagnostics::log_error(
                        &format!("Assign Footprints failed on '{}'", path.display()),
                        &anyhow::Error::new(error),
                    );
                    self.roll_back_assignments(group, &changed);
                    return Err(AssignProblem::NotChanged(path));
                }
            }
        }
        self.commit_assignments(group, changed);
        Ok(())
    }

    /// Pop the group's entry off every sheet already changed.
    fn roll_back_assignments(&mut self, group: signex_engine::UndoGroupId, changed: &[Changed]) {
        for done in changed {
            if let Some(engine) = self.document_state.engines.get_mut(&done.path)
                && let Err(error) = engine.discard_undo_group_top(group)
            {
                crate::diagnostics::log_error(
                    &format!(
                        "Assign Footprints: could not put back '{}'",
                        done.path.display()
                    ),
                    &anyhow::Error::new(error),
                );
            }
        }
    }

    /// Every sheet changed: mark them unsaved, register the group, redraw
    /// and say what happened.
    fn commit_assignments(&mut self, group: signex_engine::UndoGroupId, changed: Vec<Changed>) {
        let active = self.document_state.active_path.clone();
        let mut recorded = crate::app::UndoGroupSheets {
            kind: crate::app::UndoGroupKind::Footprints,
            ..Default::default()
        };
        let mut active_invalidation = None;
        let mut parts = 0usize;
        for done in &changed {
            parts += done.parts;
            recorded.live.push(done.path.clone());
            if active.as_deref() == Some(done.path.as_path()) {
                active_invalidation = done.invalidation;
            } else {
                self.document_state.mark_history_edit_dirty(&done.path);
            }
        }
        let sheets = changed.len();
        self.set_undo_group_sheets(group, recorded);
        if let Some(invalidation) = active_invalidation {
            self.finish_schematic_mutation(invalidation, true, true);
        }
        self.document_state.note_sheet_edit();
        self.refresh_panel_ctx();
        crate::diagnostics::log_info(format!(
            "Assign Footprints: {parts} part{} on {sheets} sheet{} changed; save to keep them.",
            if parts == 1 { "" } else { "s" },
            if sheets == 1 { "" } else { "s" },
        ));
    }
}
