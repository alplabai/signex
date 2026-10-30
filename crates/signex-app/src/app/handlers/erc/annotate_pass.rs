//! The project-wide annotate walk, shared by Annotate and (#1315) Re-prefix.

use iced::Task;

use super::super::super::*;
use super::annotate::{
    PassFailed, annotate_unopened_for_write, annotate_unopened_in_memory, read_only_sheets,
};
use super::annotate_job::{PassApplied, PendingWrite, RunUndo};
use crate::app::project_sheets::ProjectSheetSet;

/// What one project-wide pass does: the engine mode, and (#1315) an optional
/// re-prefix map that makes the pass one Re-prefix to standard run.
pub(super) struct AnnotatePass<'a> {
    pub mode: signex_engine::AnnotateMode,
    /// Symbol uuid -> the prefix its part is re-issued under. Present means
    /// the pass always records ONE undo group, whatever the Annotate undo
    /// preference says: a re-prefix that undid sheet by sheet would leave the
    /// project half-converted.
    pub reprefix: Option<&'a std::collections::HashMap<uuid::Uuid, String>>,
    /// The pass's name in the Messages panel.
    pub label: &'static str,
}

impl Signex {
    /// Re-prefix's entry (#1315): assemble the project here and number it in
    /// this one `update`. Re-prefix writes nothing (it parks the unopened sheets
    /// dirty), but its sheet read is still inline; Annotate itself runs through
    /// `annotate_job` and reaches [`Self::number_project`] with sheets a worker
    /// read.
    pub(super) fn annotate_project(&mut self, pass: AnnotatePass<'_>) -> Task<Message> {
        let label = pass.label;
        if self.ui_state.annotate_runs.is_running() {
            let detail = format!(
                "{label} was not run: an Annotate or Reset Duplicate Designators run is still going. \
                 Try again once it has finished."
            );
            crate::diagnostics::log_warning(detail.clone());
            self.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::annotate(detail));
            return Task::none();
        }
        // The project, from the one assembler: the declared pages plus
        // everything reachable down the child-sheet graph (#406).
        let (_pages, project_set) =
            crate::app::project_sheets::assemble_active_project_sheets(&self.document_state);
        // Refused up front, before any sheet is touched, so a re-prefix stays
        // all or nothing and an annotate never half-runs.
        let sheets = || project_set.sheets.keys().map(std::path::PathBuf::as_path);
        if self.refuse_annotate_while_project_moves(sheets(), label)
            || self.refuse_annotate_while_path_op(sheets(), label)
        {
            return Task::none();
        }
        let applied = self.number_project(pass, project_set);
        debug_assert!(applied.writes.is_empty(), "a re-prefix writes nothing");
        applied.task
    }

    /// Annotate the project `project_set` (read by a worker): one shared
    /// counter across every sheet, in the order `ordered_project_sheet_paths`
    /// defines. Open sheets are annotated in place; each unopened sheet comes
    /// back as bytes for the write worker.
    pub(super) fn apply_annotate(
        &mut self,
        mode: signex_engine::AnnotateMode,
        project_set: ProjectSheetSet,
    ) -> PassApplied {
        self.number_project(
            AnnotatePass {
                mode,
                reprefix: None,
                label: "Annotate",
            },
            project_set,
        )
    }

    fn number_project(
        &mut self,
        pass: AnnotatePass<'_>,
        project_set: ProjectSheetSet,
    ) -> PassApplied {
        let AnnotatePass {
            mode,
            reprefix,
            label,
        } = pass;
        let mut writes: Vec<PendingWrite> = Vec::new();
        // Pass A: seed the shared counter from every sheet's already-
        // annotated symbols. This happens inside annotate_with_seed's
        // phase 2, but running a separate seed pass first ensures
        // order-independence — without this, sheet B could reuse
        // numbers it considers free that sheet A actually claims.
        // One per-prefix counter shared by every project sheet, so designators
        // don't collide across the sheets of the project.
        let mut next_by_prefix = signex_types::designator::seed_counters(
            project_set
                .sheets
                .values()
                .flat_map(|sheet| sheet.symbols.iter())
                .filter(|sym| !sym.is_power && !sym.reference.starts_with('#'))
                .map(|sym| sym.reference.as_str()),
        );

        // Pass B: walk every sheet the counter must cover — cached tabs,
        // unopened sheets, and the active engine alike — in the ONE order
        // `ordered_project_sheet_paths` defines. The preview walks the exact
        // same order over the exact same set, so the number it promises for
        // a sheet is the number this pass assigns it (#435): the active
        // engine no longer gets an unconditional last turn regardless of
        // where its path sorts.
        let locked = self.ui_state.annotate_locked.clone();
        let active_path = self.document_state.active_path.clone();
        let ordered_paths = crate::app::project_sheets::ordered_project_sheet_paths(
            &project_set,
            active_path.as_deref(),
        );
        // Group units into parts across the WHOLE project before any sheet is
        // touched: a part whose units sit on different sheets must stay one
        // part through a reset, which a per-sheet grouping cannot see (#691).
        // A live engine is the document, so it wins over the assembler's copy.
        let parts = {
            let documents: Vec<&[signex_types::schematic::Symbol]> = ordered_paths
                .iter()
                .map(|path| {
                    self.document_state
                        .engines
                        .get(path)
                        .map(|engine| engine.document().symbols.as_slice())
                        .or_else(|| {
                            project_set
                                .sheets
                                .get(path)
                                .map(|sheet| sheet.symbols.as_slice())
                        })
                        .unwrap_or(&[])
                })
                .collect();
            signex_types::schematic::ProjectParts::new(
                &documents,
                &locked,
                self.ui_state.annotate_order,
            )
        };
        let mut assigned: std::collections::HashMap<usize, String> =
            std::collections::HashMap::new();
        let read_only = read_only_sheets(&self.document_state, &project_set);
        let mut any_active_changed = false;
        let mut any_cached_changed = false;
        // One project-wide undo step per run, unless the preference asks for
        // one per sheet. Which sheets it touched is filled in below.
        let undo_group = (reprefix.is_some()
            || self.ui_state.annotate_undo == crate::annotate_undo::AnnotateUndo::Project)
            .then(|| self.mint_undo_group());
        let mut touched_sheets = crate::app::UndoGroupSheets {
            kind: if reprefix.is_some() {
                crate::app::UndoGroupKind::Reprefix
            } else {
                crate::app::UndoGroupKind::Annotate
            },
            ..Default::default()
        };
        for (path, why) in &project_set.unreadable {
            crate::diagnostics::log_info(format!("Annotate: sheet '{}' {why}", path.display()));
        }
        // Re-prefix is all or nothing: a sheet that fails undoes the rest, so
        // no part is left half converted. What it needs to put back:
        let mut failed = false;
        let mut parked_here: std::collections::HashSet<std::path::PathBuf> = Default::default();
        let mut newly_dirty: Vec<std::path::PathBuf> = Vec::new();
        for (sheet, path) in ordered_paths.iter().enumerate() {
            let scope = signex_engine::AnnotateScope {
                parts: &parts,
                sheet,
                locked: &locked,
                undo_group,
                reprefix,
            };
            if read_only.contains(path) {
                continue;
            }
            if active_path.as_deref() == Some(path.as_path()) {
                // The active engine — run through the raw engine method (not
                // Command) so it shares the same counter. That also means it
                // cannot go through the mutation gateway, so the dirty flag
                // is set by hand once the whole pass is done (#585).
                let outcome = self.document_state.active_engine_mut().map(|engine| {
                    engine.annotate_in_project(mode, &mut next_by_prefix, scope, &mut assigned)
                });
                match outcome {
                    Some(Ok(true)) => {
                        any_active_changed = true;
                        touched_sheets.live.push(path.clone());
                    }
                    Some(Err(error)) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error(
                            "Annotate failed on the active sheet",
                            &error,
                        );
                        if reprefix.is_some() {
                            failed = true;
                            break;
                        }
                    }
                    Some(Ok(false)) | None => {}
                }
                continue;
            }
            if self.document_state.engines.contains_key(path) {
                // A live engine that is not the active one: a cached tab, or
                // a dirty sheet whose tab was closed and whose engine is
                // parked (#669). Either way the engine is the document, so
                // annotate it in memory and leave it dirty for the user to
                // save — never write it behind their back.
                let result = self.document_state.engines.get_mut(path).map(|engine| {
                    engine.annotate_in_project(mode, &mut next_by_prefix, scope, &mut assigned)
                });
                match result {
                    Some(Ok(true)) => {
                        if !self.document_state.is_document_dirty(path) {
                            newly_dirty.push(path.clone());
                        }
                        self.document_state.mark_document_dirty(path);
                        any_cached_changed = true;
                        touched_sheets.live.push(path.clone());
                    }
                    Some(Err(error)) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error(
                            &format!("Annotate failed on '{}'", path.display()),
                            &error,
                        );
                        if reprefix.is_some() {
                            failed = true;
                            break;
                        }
                    }
                    Some(Ok(false)) | None => {}
                }
                continue;
            }
            // A project sheet that isn't currently open as a tab — annotate
            // the already-parsed copy with the shared counter and write it
            // back. Altium's Annotate-Across-Project covers even the sheets
            // the user hasn't opened so designators stay unique project-wide.
            let Some(document) = project_set.sheets.get(path).cloned() else {
                continue;
            };
            if self.unopened_write_blocked("Annotate", path) {
                // A re-prefix is all or nothing: leaving one sheet out would
                // split a prefix across the project.
                if reprefix.is_some() {
                    failed = true;
                    break;
                }
                continue;
            }
            if reprefix.is_some() {
                // Re-prefix never writes behind the user's back: the sheet
                // is loaded into a parked engine, dirty and unsaved, inside
                // the same undo group as every other sheet (#669).
                match annotate_unopened_in_memory(
                    document,
                    path,
                    mode,
                    &mut next_by_prefix,
                    scope,
                    &mut assigned,
                ) {
                    Ok(Some(engine)) => {
                        self.document_state.insert_engine(path.clone(), engine);
                        parked_here.insert(path.clone());
                        newly_dirty.push(path.clone());
                        self.document_state.mark_document_dirty(path);
                        any_cached_changed = true;
                        touched_sheets.live.push(path.clone());
                    }
                    Ok(None) => {}
                    Err(PassFailed) => {
                        failed = true;
                        break;
                    }
                }
                continue;
            }
            if let Some(bytes) = annotate_unopened_for_write(
                document,
                path,
                mode,
                &mut next_by_prefix,
                scope,
                &mut assigned,
            ) {
                writes.push(PendingWrite {
                    path: path.clone(),
                    bytes,
                });
            }
        }
        if failed {
            let task = self.roll_back_reprefix(
                undo_group,
                &touched_sheets.live,
                &parked_here,
                &newly_dirty,
                label,
            );
            return PassApplied::without_writes(task);
        }
        let changed_sheets = touched_sheets.live.len();
        touched_sheets.parked = parked_here.iter().cloned().collect();
        // Owed to disk already, so an undo before the write lands still names them.
        touched_sheets.disk_only = writes.iter().map(|w| w.path.clone()).collect();
        if let Some(group) = undo_group {
            self.set_undo_group_sheets(group, touched_sheets);
        }
        // The active engine was annotated through a non-`Command` method, so
        // nothing has marked it dirty. Without this the quit guard sees a
        // clean workspace and the annotation goes out with the process
        // (#585). Cached tabs already did their own bookkeeping above.
        if any_active_changed {
            self.mark_active_document_dirty();
        }
        // Force a render + panel refresh as if a command had fired.
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.sync_canvas_from_visible_schematic(
            crate::schematic_runtime::RenderInvalidation::SYMBOLS,
        );
        self.update_selection_info();
        if any_cached_changed || self.document_state.has_active_engine() {
            self.refresh_panel_ctx();
        }

        self.ui_state.annotate_reset_confirm = false;
        if reprefix.is_some() {
            crate::diagnostics::log_info(format!(
                "{label}: re-issued designators on {changed_sheets} sheet(s)",
            ));
        } else {
            crate::diagnostics::log_info(format!(
                "Annotated symbols on {changed_sheets} open sheet(s) ({mode:?}), {} unopened \
                 sheet(s) to write",
                writes.len(),
            ));
        }
        self.report_read_only_skipped(label, &read_only);
        PassApplied {
            writes,
            task: Task::none(),
            undo: undo_group.map(|group| RunUndo { group }),
        }
    }
}

impl Signex {
    /// A re-prefix sheet failed: undo every sheet already changed (the pass's
    /// own entry is on top of each history) and forget the sheets it parked,
    /// so the project is exactly as it was and no part is left half converted.
    fn roll_back_reprefix(
        &mut self,
        group: Option<signex_engine::UndoGroupId>,
        touched: &[std::path::PathBuf],
        parked: &std::collections::HashSet<std::path::PathBuf>,
        newly_dirty: &[std::path::PathBuf],
        label: &str,
    ) -> Task<Message> {
        for path in touched {
            if parked.contains(path) {
                self.document_state.remove_engine(path);
            } else if let Some(engine) = self.document_state.engines.get_mut(path)
                && let Some(group) = group
                // Discarded, not undone: an undo would leave the half run on
                // the redo stack for Ctrl+Y to bring back.
                && let Err(error) = engine.discard_undo_group_top(group)
            {
                crate::diagnostics::log_error(
                    &format!("{label}: could not undo '{}'", path.display()),
                    &anyhow::Error::new(error),
                );
            }
        }
        for path in newly_dirty {
            self.document_state.mark_document_clean(path);
        }
        crate::diagnostics::log_warning(format!(
            "{label}: a sheet could not be converted, so the whole run was undone and nothing \
             was changed. Any redo history the touched sheets had before the run is gone."
        ));
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.sync_canvas_from_visible_schematic(
            crate::schematic_runtime::RenderInvalidation::SYMBOLS,
        );
        self.update_selection_info();
        self.refresh_panel_ctx();
        Task::none()
    }
}
