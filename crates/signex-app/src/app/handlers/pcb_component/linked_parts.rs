//! The schematic half of a board footprint swap (#2366): the linked part of
//! each swapped footprint takes the new footprint the way Update PCB's
//! Choose footprint sets it (#2371), so a later Update PCB stays quiet. The
//! sheet edits share one undo group with the board step.

use std::collections::BTreeMap;
use std::path::PathBuf;

use signex_library::PrimitiveRef;
use signex_types::pcb::SymbolLink;
use uuid::Uuid;

use crate::app::Signex;
use crate::library::place::lib_id_of;

/// Why a linked part kept its footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NotUpdated {
    /// No open sheet holds the part.
    SheetNotOpen,
    /// Its sheet opened read-only.
    SheetReadOnly,
}

/// The sheet edits of a swap, and the parts left as they were.
#[derive(Debug, Default)]
pub(super) struct LinkedPlan {
    /// The commands to run on each open sheet.
    pub sheets: BTreeMap<PathBuf, Vec<signex_engine::Command>>,
    /// Designators of the parts not updated, with why.
    pub not_updated: Vec<(String, NotUpdated)>,
}

/// A sheet edited by the swap, and the redraw it needs when it is in view.
pub(super) struct ChangedSheet {
    pub path: PathBuf,
    pub invalidation: Option<crate::schematic_runtime::RenderInvalidation>,
}

impl Signex {
    /// The sheet commands that give each linked part `footprint`: a binding
    /// of its library row when the row offers it, else the footprint named
    /// by its library key. `links` are the designator and link of each
    /// swapped footprint that has one.
    pub(super) fn plan_linked_footprints(
        &self,
        links: &[(String, SymbolLink)],
        footprint: PrimitiveRef,
    ) -> LinkedPlan {
        let binding = lib_id_of(&footprint);
        let mut plan = LinkedPlan::default();
        for (designator, link) in links {
            let Some((path, part)) = self.linked_part_sheet(link) else {
                plan.not_updated
                    .push((designator.clone(), NotUpdated::SheetNotOpen));
                continue;
            };
            let Some(doc) = self.document_state.engines.get(&path).map(|e| e.document()) else {
                continue;
            };
            if doc.is_read_only() {
                plan.not_updated
                    .push((designator.clone(), NotUpdated::SheetReadOnly));
                continue;
            }
            let commands = self
                .footprint_binding_commands(doc, part, &binding)
                .unwrap_or_else(|_| {
                    signex_engine::placed_unit_ids(doc, part)
                        .into_iter()
                        .map(|symbol_id| signex_engine::Command::UpdateSymbolFootprint {
                            symbol_id,
                            footprint: binding.clone(),
                        })
                        .collect()
                });
            plan.sheets.entry(path).or_default().extend(commands);
        }
        plan
    }

    /// The open sheet holding the part `link` names, and the unit found
    /// there. Of several files with the unit (a reused sheet), the one the
    /// link's placement shows.
    fn linked_part_sheet(&self, link: &SymbolLink) -> Option<(PathBuf, Uuid)> {
        let unit_in = |path: &PathBuf| {
            let doc = self.document_state.engines.get(path)?.document();
            link.units
                .iter()
                .copied()
                .find(|unit| doc.symbols.iter().any(|s| s.uuid == *unit))
        };
        let mut candidates: Vec<(PathBuf, Uuid)> = self
            .document_state
            .engines
            .keys()
            .filter_map(|path| unit_in(path).map(|unit| (path.clone(), unit)))
            .collect();
        candidates.sort();
        let first = candidates.first()?.clone();
        let placed: Vec<PathBuf> = self
            .hierarchy_describing(&first.0)
            .map(|(h, _)| {
                h.paths_at(&link.sheet)
                    .into_iter()
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default();
        Some(
            candidates
                .into_iter()
                .find(|(path, _)| placed.contains(path))
                .unwrap_or(first),
        )
    }

    /// Run each sheet's commands as a step of `group`. A sheet whose step
    /// fails is left as it was and named in Messages.
    pub(super) fn run_linked_footprints(
        &mut self,
        sheets: BTreeMap<PathBuf, Vec<signex_engine::Command>>,
        group: signex_engine::UndoGroupId,
    ) -> Vec<ChangedSheet> {
        let mut changed = Vec::new();
        for (path, commands) in sheets {
            let Some(engine) = self.document_state.engines.get_mut(&path) else {
                continue;
            };
            let outcome = engine.execute_batch_grouped(commands, Some(group));
            for warning in engine.take_warnings() {
                crate::diagnostics::log_warning(warning);
            }
            match outcome {
                Ok(result) if result.changed => changed.push(ChangedSheet {
                    path,
                    invalidation: result
                        .patch_pair
                        .map(|pair| Self::render_invalidation_for_patch(pair.document)),
                }),
                Ok(_) => {}
                Err(error) => crate::diagnostics::log_error(
                    &format!("Change Footprint: '{}' was not updated", path.display()),
                    &anyhow::Error::new(error),
                ),
            }
        }
        changed
    }

    /// Take the group's step back off every changed sheet: the board step
    /// was refused.
    pub(super) fn roll_back_linked_footprints(
        &mut self,
        changed: &[ChangedSheet],
        group: signex_engine::UndoGroupId,
    ) {
        for sheet in changed {
            if let Some(engine) = self.document_state.engines.get_mut(&sheet.path)
                && let Err(error) = engine.discard_undo_group_top(group)
            {
                crate::diagnostics::log_error(
                    &format!(
                        "Change Footprint: could not put back '{}'",
                        sheet.path.display()
                    ),
                    &anyhow::Error::new(error),
                );
            }
        }
    }

    /// The changed sheets read unsaved, the one in view is redrawn, and the
    /// group is recorded with the board so one Undo moves them all.
    pub(super) fn commit_linked_footprints(
        &mut self,
        changed: Vec<ChangedSheet>,
        group: signex_engine::UndoGroupId,
        board: PathBuf,
    ) {
        let active = self.document_state.active_path.clone();
        let mut recorded = crate::app::UndoGroupSheets {
            kind: crate::app::UndoGroupKind::FootprintSwap,
            boards: vec![board],
            ..Default::default()
        };
        let mut in_view = None;
        for sheet in changed {
            if active.as_ref() == Some(&sheet.path) {
                in_view = sheet.invalidation;
            } else {
                self.document_state.mark_history_edit_dirty(&sheet.path);
            }
            recorded.live.push(sheet.path);
        }
        self.set_undo_group_sheets(group, recorded);
        match in_view {
            Some(invalidation) => {
                self.finish_schematic_mutation(invalidation, true, true);
            }
            None => self.document_state.note_sheet_edit(),
        }
    }
}
