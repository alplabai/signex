//! Annotate + duplicate-designator reset handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::*;
use super::annotate_pass::AnnotatePass;

/// Annotate a sheet that exists only on disk and write it back. Returns
/// `true` when the file was rewritten; every failure is logged with the path.
/// Only for sheets with no engine at all — an engine (open or parked) holds the
/// user's unsaved edits and is annotated in place instead (#669).
pub(super) fn annotate_and_save_unopened(
    sheet: signex_types::schematic::SchematicSheet,
    path: &std::path::Path,
    mode: signex_engine::AnnotateMode,
    next_by_prefix: &mut std::collections::HashMap<String, u32>,
    scope: signex_engine::AnnotateScope<'_>,
    assigned: &mut std::collections::HashMap<usize, String>,
) -> bool {
    let context = format!("Annotate failed on unopened sheet '{}'", path.display());
    let mut engine = match signex_engine::Engine::new(sheet) {
        Ok(engine) => engine,
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            return false;
        }
    };
    engine.set_path(Some(path.to_path_buf()));
    match engine.annotate_in_project(mode, next_by_prefix, scope, assigned) {
        Ok(true) => {}
        Ok(false) => return false,
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            return false;
        }
    }
    if let Err(error) = engine.save() {
        let error = anyhow::Error::new(error);
        crate::diagnostics::log_error(
            &format!("Annotate could not save '{}'", path.display()),
            &error,
        );
        crate::app::handlers::document_files::log_foreign_column_offenders(&error);
        return false;
    }
    crate::diagnostics::log_info(format!("Annotate: saved {}", path.display()));
    true
}

/// Annotate a sheet that exists only on disk, in memory, WITHOUT writing it.
/// Returns the engine (path set) when the pass changed it, so the caller can
/// park it dirty like any other unsaved sheet (#669): nothing reaches disk
/// until the user saves, and the engine's history makes the change undoable.
/// Re-prefix uses this; plain Annotate keeps `annotate_and_save_unopened`.
/// `Err` means the sheet failed and was logged.
pub(super) fn annotate_unopened_in_memory(
    sheet: signex_types::schematic::SchematicSheet,
    path: &std::path::Path,
    mode: signex_engine::AnnotateMode,
    next_by_prefix: &mut std::collections::HashMap<String, u32>,
    scope: signex_engine::AnnotateScope<'_>,
    assigned: &mut std::collections::HashMap<usize, String>,
) -> Result<Option<signex_engine::Engine>, PassFailed> {
    let context = format!("Annotate failed on unopened sheet '{}'", path.display());
    let mut engine = match signex_engine::Engine::new(sheet) {
        Ok(engine) => engine,
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            return Err(PassFailed);
        }
    };
    engine.set_path(Some(path.to_path_buf()));
    match engine.annotate_in_project(mode, next_by_prefix, scope, assigned) {
        Ok(true) => Ok(Some(engine)),
        Ok(false) => Ok(None),
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            Err(PassFailed)
        }
    }
}

/// A sheet of the pass failed (already logged with its path).
pub(super) struct PassFailed;

/// Sheets in the project written by a newer minor format version. This build
/// holds them read-only (every save is refused), so the bulk passes leave them
/// untouched: editing one would only strand a dirty sheet that can never be
/// saved. Their designators still count as taken in the annotate seed pass.
pub(super) fn read_only_sheets(
    documents: &DocumentState,
    project_set: &crate::app::project_sheets::ProjectSheetSet,
) -> std::collections::BTreeSet<std::path::PathBuf> {
    project_set
        .sheets
        .iter()
        .filter(|(path, sheet)| {
            let unknown = documents
                .engines
                .get(*path)
                .map_or(&sheet.unknown, |engine| &engine.document().unknown);
            unknown.newer_minor_source.is_some()
        })
        .map(|(path, _)| path.clone())
        .collect()
}

/// The sentence appended to a bulk pass's result when sheets were skipped.
fn skipped_summary(skipped: &std::collections::BTreeSet<std::path::PathBuf>) -> String {
    let names: Vec<String> = skipped
        .iter()
        .map(|p| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        })
        .collect();
    format!(
        "{} sheet(s) were written by a newer Signex and are read-only in this build, so \
         they were left unchanged: {}. Open the project in the newer version to number them.",
        names.len(),
        names.join(", ")
    )
}

impl Signex {
    /// Whether a bulk pass must leave the tab-less sheet at `path` alone
    /// because a History restore of it is still running (even a cancelled
    /// one) or a read of it is in flight: the write would race the worker,
    /// and a read landing later would install the pre-pass content over it.
    /// The reason leads the Messages line.
    pub(super) fn unopened_write_blocked(&self, pass: &str, path: &std::path::Path) -> bool {
        let reason = if self.document_state.pending_restores.contains(path) {
            "a History restore of it is still running"
        } else if self.document_state.pending_opens.contains(path) {
            "a read of it is still in flight"
        } else {
            return false;
        };
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        crate::diagnostics::log_warning(format!("{pass}: {name} was left unchanged, {reason}"));
        true
    }

    /// Tell the user which sheets a bulk pass skipped: Messages panel plus a
    /// dialog, because a quiet log line would read as "everything was done".
    pub(super) fn report_read_only_skipped(
        &mut self,
        pass: &str,
        skipped: &std::collections::BTreeSet<std::path::PathBuf>,
    ) {
        if skipped.is_empty() {
            return;
        }
        let summary = skipped_summary(skipped);
        crate::diagnostics::log_warning(format!("{pass}: {summary}"));
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::sheet_edit(
            format!("{pass} skipped read-only sheets. {summary}"),
        ));
    }

    /// Record that a non-active engine (a cached tab, or a parked engine whose
    /// tab was closed) now holds unsaved edits. `tab.dirty` alone is not
    /// enough: `dirty_paths` is what the quit guard, the tab-close engine
    /// parking, the title-bar count and Save-All read, and a bare
    /// `tab.dirty = true` never propagates into it (#585). Shared by Annotate
    /// and Reset Duplicate Designators so the two cannot drift (#669).
    pub(in crate::app) fn mark_cached_sheet_dirty(&mut self, path: &std::path::Path) {
        if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
            tab.dirty = true;
        }
        self.document_state.dirty_paths.insert(path.to_path_buf());
    }

    /// Refuse a project-wide rewrite up front, with a notice, when any of its
    /// sheets lies in a project whose folder is being renamed: the unopened
    /// sheets would be written into the old folder. True when refused.
    pub(super) fn refuse_annotate_while_project_moves(
        &mut self,
        project_set: &crate::app::project_sheets::ProjectSheetSet,
        what: &str,
    ) -> bool {
        let Some(path) = project_set
            .sheets
            .keys()
            .find(|p| self.path_is_in_moving_project(p))
        else {
            return false;
        };
        let detail = format!(
            "{what} was not run: {} is in a project whose folder is being renamed. Try again once the rename has finished.",
            path.display()
        );
        crate::diagnostics::log_warning(detail.clone());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
        true
    }

    pub(crate) fn handle_annotate(&mut self, mode: signex_engine::AnnotateMode) -> Task<Message> {
        self.annotate_project(AnnotatePass {
            mode,
            reprefix: None,
            label: "Annotate",
        })
    }

    pub(crate) fn handle_open_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = true;
        self.interaction_state.context_menu = None;
        // Altium parity: these big modals live in their own OS window
        // from the moment they open — no in-window overlay, no drag-off
        // dance. `handle_detach_modal` is idempotent, so re-opening
        // while a window already exists just no-ops.
        self.handle_detach_modal(super::super::super::state::ModalId::AnnotateDialog)
    }

    pub(crate) fn handle_close_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = false;
        // A Re-prefix request belongs to this dialog session: leaving it
        // makes any lookup in flight and any open confirm stale.
        self.invalidate_reprefix();
        let close_confirm = self.drain_reprefix_window_close();
        let close = self.close_detached_modal(super::super::super::state::ModalId::AnnotateDialog);
        Task::batch([close, close_confirm])
    }

    /// Altium's "Reset Duplicate Designators" — find references that
    /// appear on more than one symbol across the WHOLE project, reset
    /// just those to `{prefix}?`. Everything else keeps its current
    /// value. The project is the one assembler's answer
    /// ([`crate::app::project_sheets::assemble_project_sheets`]): the
    /// declared pages plus everything reachable down the child-sheet
    /// graph. Unopened sheets are re-saved through the native
    /// `.snxsch` writer so the fix is project-wide.
    pub(crate) fn handle_reset_duplicate_designators(&mut self) -> Task<Message> {
        use std::collections::HashSet;
        use std::path::PathBuf;

        // The project, not "every open engine plus the declared pages".
        // The old set both over- and under-reached: a loose tab from
        // another project made its R1 collide with this project's R1,
        // while a hierarchical child that was never added to
        // `data.sheets` was invisible — so its duplicate refdes
        // survived into the netlist and onto the board (#406).
        let (_pages, project_set) =
            crate::app::project_sheets::assemble_active_project_sheets(&self.document_state);
        if self.refuse_annotate_while_project_moves(&project_set, "Reset Duplicate Designators") {
            return Task::none();
        }

        // Split by "is it open as an engine": open sheets are reset
        // through their engine so the edit is undoable, the rest are
        // written straight back to disk. The already-parsed
        // `SchematicSheet` is kept for phase 2 so counting and reset
        // see the same state.
        // A designator is a duplicate when more than one PART holds it across
        // the whole project; units of one multi-unit part sharing a designator
        // (wherever they are placed) are not (#691).
        let duplicates: HashSet<String> = signex_types::schematic::duplicate_designators(
            project_set
                .sheets
                .values()
                .flat_map(|sheet| sheet.symbols.iter()),
        );
        let read_only = read_only_sheets(&self.document_state, &project_set);
        let mut unopened: Vec<(PathBuf, signex_types::schematic::SchematicSheet)> = Vec::new();
        let mut project_paths: HashSet<PathBuf> = HashSet::new();
        for (path, sheet) in project_set.sheets {
            if read_only.contains(&path) {
                continue;
            }
            project_paths.insert(path.clone());
            if !self.document_state.engines.contains_key(&path) {
                if self.unopened_write_blocked("Reset Duplicate Designators", &path) {
                    continue;
                }
                unopened.push((path, sheet));
            }
        }
        for (path, why) in &project_set.unreadable {
            crate::diagnostics::log_info(format!(
                "Reset Duplicate Designators: sheet '{}' {why}",
                path.display(),
            ));
        }
        // Deterministic write order — `sheets` is a `HashMap`.
        unopened.sort_by(|a, b| a.0.cmp(&b.0));

        if duplicates.is_empty() {
            crate::diagnostics::log_info("Reset Duplicate Designators: no duplicates found");
            self.report_read_only_skipped("Reset Duplicate Designators", &read_only);
            return Task::none();
        }

        // Reset helper: for each symbol whose current reference is in
        // the duplicates set, reset to `{prefix}?`. Returns whether
        // anything changed in the sheet.
        fn reset_in(
            sheet: &mut signex_types::schematic::SchematicSheet,
            dupes: &HashSet<String>,
        ) -> bool {
            let mut changed = false;
            for sym in sheet.symbols.iter_mut() {
                if sym.is_power || sym.reference.starts_with('#') {
                    continue;
                }
                if dupes.contains(sym.reference.trim()) {
                    let prefix = signex_types::designator::alphabetic_prefix(&sym.reference);
                    if !prefix.is_empty() {
                        sym.reference = format!("{prefix}?");
                        changed = true;
                    }
                }
            }
            changed
        }

        let mut resets = 0_usize;
        // Active engine — goes through ReplaceDocument so undo records the
        // snapshot, and through the mutation gateway so the sheet reaches
        // `dirty_paths` (#585). The clone is taken first and the engine
        // borrow released, because the gateway needs `&mut self`.
        let reset_sheet = self.document_state.active_engine().and_then(|engine| {
            if engine.document().unknown.newer_minor_source.is_some() {
                return None;
            }
            let mut sheet = engine.document().clone();
            reset_in(&mut sheet, &duplicates).then_some(sheet)
        });
        if let Some(document) = reset_sheet {
            self.apply_engine_command(
                signex_engine::Command::ReplaceDocument { document },
                true,
                false,
            );
            resets += 1;
        }
        // Every other live engine of this project — a cached tab, or a dirty
        // sheet whose tab was closed and whose engine is parked (#669). The
        // project set counted parked engines (it prefers `engines`), so they
        // must be reset too, in memory: never written behind the user's back.
        // Driven by `engines`, not `tabs`; a loose engine from another
        // project was never counted, so it is not reset either.
        let active_path = self.document_state.active_path.clone();
        let mut cached_paths: Vec<PathBuf> = self
            .document_state
            .engines
            .keys()
            .filter(|path| {
                project_paths.contains(*path) && active_path.as_deref() != Some(path.as_path())
            })
            .cloned()
            .collect();
        cached_paths.sort();
        for path in cached_paths {
            let applied = self.document_state.engines.get_mut(&path).map(|engine| {
                let mut sheet = engine.document().clone();
                if !reset_in(&mut sheet, &duplicates) {
                    return false;
                }
                // Not the gateway: this is a cached, non-active engine, so
                // the dirty bookkeeping is done by hand below. The result is
                // still read rather than discarded — a failure here means the
                // sheet was reported as reset when it was not (#585).
                match engine.execute(signex_engine::Command::ReplaceDocument { document: sheet }) {
                    Ok(result) => result.changed,
                    Err(error) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error(
                            &format!("Reset Duplicate Designators failed for {}", path.display()),
                            &error,
                        );
                        false
                    }
                }
            });
            if let Some(true) = applied {
                self.mark_cached_sheet_dirty(&path);
                resets += 1;
            }
        }
        // Unopened sheets — mutate the already-parsed copy and write
        // back. NOTE: direct save, not undoable from within Signex;
        // the user would need to re-open the sheet and Ctrl+Z manually.
        let mut recovery_cleanup: Vec<Task<Message>> = Vec::new();
        for (path, mut sheet) in unopened {
            if !reset_in(&mut sheet, &duplicates) {
                continue;
            }
            let mut engine = match signex_engine::Engine::new(sheet) {
                Ok(eng) => eng,
                Err(err) => {
                    crate::diagnostics::log_info(format!(
                        "Reset Duplicate Designators: engine construct failed for {}: {err}",
                        path.display(),
                    ));
                    continue;
                }
            };
            engine.set_path(Some(path.clone()));
            match engine.save() {
                Ok(_) => {
                    resets += 1;
                    self.document_state.dirty_paths.remove(&path);
                    // Rewritten outside the document save path: call the
                    // autosave hook so a recovery copy of the old content goes.
                    recovery_cleanup.push(self.autosave_after_disk_write(&path));
                }
                Err(err) => {
                    crate::diagnostics::log_info(format!(
                        "Reset Duplicate Designators: save failed for {}: {err}",
                        path.display(),
                    ));
                    crate::app::handlers::document_files::log_foreign_column_offenders(
                        &anyhow::Error::new(err),
                    );
                }
            }
        }

        // The active sheet's repaint is the gateway's job now. This refresh
        // is for the *cached* tabs, whose dirty dots would otherwise only
        // appear after the next unrelated event — the old condition was
        // `any_active_changed`, so a reset that touched no open-and-active
        // sheet refreshed nothing at all.
        if resets > 0 {
            self.refresh_panel_ctx();
            // Unopened sheets were rewritten on disk, which the in-memory
            // fingerprint cannot see.
            self.invalidate_annotate_preview();
        }
        crate::diagnostics::log_info(format!(
            "Reset Duplicate Designators: {} duplicate reference(s) found project-wide; reset on {} sheet(s)",
            duplicates.len(),
            resets,
        ));
        self.report_read_only_skipped("Reset Duplicate Designators", &read_only);
        Task::batch(recovery_cleanup)
    }

    pub(crate) fn handle_annotate_order_changed(
        &mut self,
        order: super::super::super::state::AnnotateOrder,
    ) -> Task<Message> {
        self.ui_state.annotate_order = order;
        // The cached preview rows were ordered under the old setting.
        self.recompute_annotate_entries();
        Task::none()
    }
}
