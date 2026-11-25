//! The project-wide annotate walk, shared by Annotate and (#1315) Re-prefix.
//! The sheets always come from a read worker (#859, #1756); nothing here reads
//! or writes disk. Unopened sheets are parked, unsaved (#2760).

use iced::Task;

use super::super::super::*;
use super::annotate::{PassFailed, annotate_unopened_in_memory, read_only_sheets};
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
    /// Annotate the project `project_set` (read by a worker): one shared
    /// counter across every sheet, in the order `ordered_project_sheet_paths`
    /// defines. Open sheets are annotated in place; each unopened sheet loads
    /// into a parked engine, unsaved until the user saves (#2760).
    pub(super) fn apply_annotate(
        &mut self,
        mode: signex_engine::AnnotateMode,
        project_set: ProjectSheetSet,
    ) -> Task<Message> {
        self.number_project(
            AnnotatePass {
                mode,
                reprefix: None,
                label: "Annotate",
            },
            project_set,
        )
    }

    /// Number `project_set` under `pass`. Shared by Annotate and Re-prefix:
    /// each reaches it with sheets a worker read.
    pub(super) fn number_project(
        &mut self,
        pass: AnnotatePass<'_>,
        project_set: ProjectSheetSet,
    ) -> Task<Message> {
        let AnnotatePass {
            mode,
            reprefix,
            label,
        } = pass;
        // Designators before the pass, to tell afterwards which symbols it
        // renumbered.
        let before: std::collections::HashMap<_, std::collections::HashMap<uuid::Uuid, String>> =
            self.document_state
                .engines
                .iter()
                .map(|(path, engine)| {
                    let refs = (engine.document().symbols.iter())
                        .map(|s| (s.uuid, s.reference.clone()))
                        .collect();
                    (path.clone(), refs)
                })
                .collect();
        // Every sheet the counter must cover - cached tabs, unopened sheets
        // and the active engine alike - in the ONE order
        // `ordered_project_sheet_paths` defines. The preview walks the same
        // order over the same set, and numbers a sheet file placed more than
        // once per placement as this pass does, so the number it promises for
        // a sheet is the number this pass assigns it (#435, #1919).
        let active_path = self.document_state.active_path.clone();
        let ordered_paths = crate::app::project_sheets::ordered_project_sheet_paths(
            &project_set,
            active_path.as_deref(),
        );
        // Sheets the Annotate dialog unticked (#75) stay as they are: their
        // symbols act as locked, so their designators keep their numbers in
        // the counter. Re-prefix always covers the whole project.
        let unticked = if reprefix.is_none() {
            self.annotate_unticked_sheets()
        } else {
            Default::default()
        };
        let locked = super::annotate_preview::rows::with_unticked_locked(
            &self.ui_state.annotate_locked,
            &unticked,
            ordered_paths.iter().map(|path| {
                let symbols = (self.document_state.engines.get(path))
                    .map(|engine| engine.document().symbols.as_slice())
                    .or_else(|| project_set.sheets.get(path).map(|s| s.symbols.as_slice()))
                    .unwrap_or(&[]);
                (path.as_path(), symbols)
            }),
        );
        let in_run: Vec<std::path::PathBuf> = (ordered_paths.iter())
            .filter(|path| !unticked.contains(*path))
            .cloned()
            .collect();
        let read_only = read_only_sheets(&self.document_state, &project_set);
        let resets = reprefix.is_none()
            && matches!(
                mode,
                signex_engine::AnnotateMode::ResetOnly
                    | signex_engine::AnnotateMode::ResetAndRenumber
            );
        // A Reset & Renumber that skipped a sheet would number around the
        // designators that sheet keeps and could collide with them (#1753).
        if reprefix.is_none()
            && matches!(mode, signex_engine::AnnotateMode::ResetAndRenumber)
            && self.refuse_reset_that_skips(label, &project_set, &in_run, &read_only)
        {
            return Task::none();
        }
        // Reset All numbers nothing, but one undo must still cover the whole
        // project, so a sheet it cannot reach refuses it too (#2760). A
        // read-only sheet stays skipped and named, as it cannot change.
        if reprefix.is_none()
            && matches!(mode, signex_engine::AnnotateMode::ResetOnly)
            && self.refuse_reset_all_that_skips(label, &project_set, &in_run)
        {
            return Task::none();
        }
        // A re-prefix or reset that left one sheet out would split a prefix
        // or a part; Incremental still skips a sheet it cannot change.
        let all_or_nothing = reprefix.is_some() || resets;
        let numbered: Vec<std::path::PathBuf> = (in_run.iter())
            .filter(|path| !read_only.contains(*path))
            .cloned()
            .collect();
        let super::annotate_placements::Walked {
            parts,
            seed: mut next_by_prefix,
            first_unit,
            placements,
            blocks,
        } = self.walk_project(&project_set, &ordered_paths, &locked, resets, &numbered);
        // A page scheme that cannot give every page a block of its own
        // refuses the run before any sheet changes (#1292).
        let blocks = match blocks {
            Ok(blocks) => blocks,
            Err(why) => {
                self.refuse_run(label, &format!("{why}. Nothing was changed"));
                return Task::none();
            }
        };
        let mut assigned: std::collections::HashMap<usize, String> =
            std::collections::HashMap::new();
        let mut any_active_changed = false;
        let mut any_cached_changed = false;
        // One project-wide undo step per run, unless the project (or, when
        // it has no choice of its own, the preference) asks for one per
        // sheet (#1692). Which sheets it touched is filled in below.
        let undo_group = (reprefix.is_some()
            || self.annotate_undo_of(project_set.root.as_deref())
                == crate::annotate_undo::AnnotateUndo::Project)
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
        // Re-prefix and Reset are all or nothing: a sheet that fails undoes
        // the rest, so no project is left half done. What they put back:
        let mut failed = false;
        let mut parked_here: std::collections::HashSet<std::path::PathBuf> = Default::default();
        let mut newly_dirty: Vec<std::path::PathBuf> = Vec::new();
        let active_was_clean = (active_path.as_deref())
            .is_some_and(|path| !self.document_state.is_document_dirty(path));
        for (sheet, path) in ordered_paths.iter().enumerate() {
            let scope = signex_engine::AnnotateScope {
                parts: &parts,
                sheet: first_unit[sheet],
                locked: &locked,
                undo_group,
                reprefix,
                placements: &placements[sheet],
                blocks: blocks.as_ref(),
            };
            if read_only.contains(path) || unticked.contains(path) {
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
                        if all_or_nothing {
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
                        if all_or_nothing {
                            failed = true;
                            break;
                        }
                    }
                    Some(Ok(false)) | None => {}
                }
                continue;
            }
            // A project sheet with no engine: numbered with the shared counter
            // so designators stay unique project-wide, and loaded into a parked
            // engine, dirty and unsaved, in the same undo group as every other
            // sheet. Nothing is written behind the user's back (#669, #2760).
            let Some(mut document) = project_set.sheets.get(path).cloned() else {
                continue;
            };
            if self.unopened_write_blocked("Annotate", path) {
                if all_or_nothing {
                    failed = true;
                    break;
                }
                continue;
            }
            let read_hash = document.take_source_hash();
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
                    self.settle_parked_sheet(path, read_hash);
                    parked_here.insert(path.clone());
                    newly_dirty.push(path.clone());
                    self.document_state.mark_document_dirty(path);
                    any_cached_changed = true;
                    touched_sheets.live.push(path.clone());
                }
                Ok(None) => {}
                Err(PassFailed) => {
                    if all_or_nothing {
                        failed = true;
                        break;
                    }
                }
            }
        }
        if failed {
            return self.roll_back_run(
                undo_group,
                &touched_sheets.live,
                &parked_here,
                &newly_dirty,
                label,
            );
        }
        self.warn_mixed_after_renumbering(&before);
        // Windows and tile panes on the other annotated sheets follow; the
        // engine method reports no patch, so they are redrawn whole.
        for path in &touched_sheets.live {
            if active_path.as_deref() != Some(path.as_path()) {
                self.repaint_member_sheet(path, signex_engine::DocumentPatch::FULL);
            }
        }
        let changed_sheets = touched_sheets.live.len();
        touched_sheets.parked = parked_here.iter().cloned().collect();
        touched_sheets.clean_before = newly_dirty;
        if any_active_changed && active_was_clean {
            touched_sheets.clean_before.extend(active_path.clone());
        }
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
        // Force a render + panel refresh as if a command had fired. An
        // unchanged active sheet kept its revision, so SYMBOLS would describe
        // whatever edit last moved it, not this run: redraw it whole.
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.sync_canvas_from_visible_schematic(if any_active_changed {
            crate::schematic_runtime::RenderInvalidation::SYMBOLS
        } else {
            crate::schematic_runtime::RenderInvalidation::FULL
        });
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
                "{label}: changed {changed_sheets} sheet(s) in memory ({mode:?}); nothing is \
                 written until you save",
            ));
        }
        self.report_read_only_skipped(label, &read_only);
        Task::none()
    }
}

impl Signex {
    /// A sheet of an all-or-nothing run failed: undo every sheet already
    /// changed (the pass's own entry is on top of each history) and forget the
    /// sheets it parked, so the project is exactly as it was. `group` is `None`
    /// for a run under the per-sheet Annotate undo setting.
    pub(super) fn roll_back_run(
        &mut self,
        group: Option<signex_engine::UndoGroupId>,
        touched: &[std::path::PathBuf],
        parked: &std::collections::HashSet<std::path::PathBuf>,
        newly_dirty: &[std::path::PathBuf],
        label: &str,
    ) -> Task<Message> {
        let mut not_rolled_back = std::collections::HashSet::new();
        for path in touched {
            if parked.contains(path) {
                self.abandon_parked_sheet(path);
            } else if let Some(engine) = self.document_state.engines.get_mut(path) {
                // Discarded, not undone: an undo would leave the half run on
                // the redo stack for Ctrl+Y to bring back. No library drift
                // check: a take-back of the same call is not a Step.
                let discarded = match group {
                    Some(group) => engine.discard_undo_group_top(group),
                    None => engine.discard_undo_top(),
                };
                match discarded {
                    Ok(Some(_)) => {}
                    // The run is still in the sheet: it must stay dirty.
                    Ok(None) => {
                        not_rolled_back.insert(path.clone());
                    }
                    Err(error) => {
                        not_rolled_back.insert(path.clone());
                        crate::diagnostics::log_error(
                            &format!("{label}: could not undo '{}'", path.display()),
                            &anyhow::Error::new(error),
                        );
                    }
                }
            }
        }
        for path in newly_dirty {
            if !not_rolled_back.contains(path) {
                self.document_state.clear_document_dirty(path);
            }
        }
        // The message below says these sheets stay unsaved: make that true.
        // The active sheet is in no `newly_dirty` (the pass marks it only
        // after it succeeds), so without this it would read clean.
        for path in &not_rolled_back {
            if self.document_state.active_path.as_ref() == Some(path) {
                self.mark_active_document_dirty();
            } else {
                self.document_state.mark_document_dirty(path);
            }
        }
        if not_rolled_back.is_empty() {
            crate::diagnostics::log_warning(format!(
                "{label}: a sheet could not be changed, so the whole run was undone and \
                 nothing was changed. Any redo history the touched sheets had before the run \
                 is gone."
            ));
        } else {
            let mut names: Vec<String> = not_rolled_back
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            names.sort();
            crate::diagnostics::log_warning(format!(
                "{label}: a sheet could not be changed and the run could not be undone on: \
                 {}. Those sheets keep the run's changes and stay unsaved; undo them by hand.",
                names.join(", ")
            ));
        }
        // The active sheet may have been rolled back or never touched; only
        // a whole redraw is right for both.
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.sync_canvas_from_visible_schematic(crate::schematic_runtime::RenderInvalidation::FULL);
        self.update_selection_info();
        self.refresh_panel_ctx();
        Task::none()
    }
}
