//! Annotate + duplicate-designator reset handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::*;

/// Annotate a sheet that exists only on disk and write it back. Returns
/// `true` when the file was rewritten; every failure is logged with the path.
/// Only for sheets with no engine at all — an engine (open or parked) holds the
/// user's unsaved edits and is annotated in place instead (#669).
fn annotate_and_save_unopened(
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
        crate::diagnostics::log_error(
            &format!("Annotate could not save '{}'", path.display()),
            &anyhow::Error::new(error),
        );
        return false;
    }
    crate::diagnostics::log_info(format!("Annotate: saved {}", path.display()));
    true
}

impl Signex {
    /// Record that a non-active engine (a cached tab, or a parked engine whose
    /// tab was closed) now holds unsaved edits. `tab.dirty` alone is not
    /// enough: `dirty_paths` is what the quit guard, the tab-close engine
    /// parking, the title-bar count and Save-All read, and a bare
    /// `tab.dirty = true` never propagates into it (#585). Shared by Annotate
    /// and Reset Duplicate Designators so the two cannot drift (#669).
    fn mark_cached_sheet_dirty(&mut self, path: &std::path::Path) {
        if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
            tab.dirty = true;
        }
        self.document_state.dirty_paths.insert(path.to_path_buf());
    }

    pub(crate) fn handle_annotate(&mut self, mode: signex_engine::AnnotateMode) -> Task<Message> {
        // Share one per-prefix counter across every open sheet so designators
        // don't collide across sheets of the same project.
        let mut next_by_prefix: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let tab_count = self.document_state.tabs.len();

        // The project, from the one assembler: the declared pages plus
        // everything reachable down the child-sheet graph. Seeding off
        // `engines.values()` instead both over-reached (a loose tab from
        // another project claimed numbers here) and under-reached (a
        // hierarchical child never added to `data.sheets` was invisible, so
        // this "project-wide" annotate handed out designators it already
        // used) — #406.
        let (_pages, project_set) =
            crate::app::project_sheets::assemble_active_project_sheets(&self.document_state);

        // Pass A: seed the shared counter from every sheet's already-
        // annotated symbols. This happens inside annotate_with_seed's
        // phase 2, but running a separate seed pass first ensures
        // order-independence — without this, sheet B could reuse
        // numbers it considers free that sheet A actually claims.
        let mut all_existing: Vec<String> = Vec::new();
        for sheet in project_set.sheets.values() {
            for sym in &sheet.symbols {
                if !sym.is_power && !sym.reference.starts_with('#') {
                    all_existing.push(sym.reference.clone());
                }
            }
        }
        for refstr in &all_existing {
            let prefix: String = refstr
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            if prefix.is_empty() {
                continue;
            }
            if let Ok(n) = refstr[prefix.len()..].parse::<u32>() {
                let e = next_by_prefix.entry(prefix).or_insert(0);
                if n > *e {
                    *e = n;
                }
            }
        }

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
            signex_types::schematic::ProjectParts::new(&documents, &locked)
        };
        let mut assigned: std::collections::HashMap<usize, String> =
            std::collections::HashMap::new();
        let mut any_active_changed = false;
        let mut any_cached_changed = false;
        let mut disk_touched = 0usize;
        for (path, why) in &project_set.unreadable {
            crate::diagnostics::log_info(format!("Annotate: sheet '{}' {why}", path.display()));
        }
        for (sheet, path) in ordered_paths.iter().enumerate() {
            let scope = signex_engine::AnnotateScope {
                parts: &parts,
                sheet,
                locked: &locked,
            };
            if active_path.as_deref() == Some(path.as_path()) {
                // The active engine — run through the raw engine method (not
                // Command) so it shares the same counter. That also means it
                // cannot go through the mutation gateway, so the dirty flag
                // is set by hand once the whole pass is done (#585).
                let outcome = self.document_state.active_engine_mut().map(|engine| {
                    engine.annotate_in_project(mode, &mut next_by_prefix, scope, &mut assigned)
                });
                match outcome {
                    Some(Ok(true)) => any_active_changed = true,
                    Some(Err(error)) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error(
                            "Annotate failed on the active sheet",
                            &error,
                        );
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
                        self.mark_cached_sheet_dirty(path);
                        any_cached_changed = true;
                    }
                    Some(Err(error)) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error(
                            &format!("Annotate failed on '{}'", path.display()),
                            &error,
                        );
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
            if annotate_and_save_unopened(
                document,
                path,
                mode,
                &mut next_by_prefix,
                scope,
                &mut assigned,
            ) {
                disk_touched += 1;
            }
        }
        if disk_touched > 0 {
            crate::diagnostics::log_info(format!(
                "Annotate: wrote {} unopened sheet file(s) to disk",
                disk_touched,
            ));
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
        self.sync_canvas_from_visible_schematic(crate::schematic_runtime::RenderInvalidation::FULL);
        self.update_selection_info();
        if any_cached_changed || self.document_state.has_active_engine() {
            self.refresh_panel_ctx();
        }

        self.ui_state.annotate_reset_confirm = false;
        crate::diagnostics::log_info(format!(
            "Annotated symbols across {} sheet(s) ({:?})",
            tab_count.max(1),
            mode,
        ));
        Task::none()
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
        self.close_detached_modal(super::super::super::state::ModalId::AnnotateDialog)
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
        let mut unopened: Vec<(PathBuf, signex_types::schematic::SchematicSheet)> = Vec::new();
        let mut project_paths: HashSet<PathBuf> = HashSet::new();
        for (path, sheet) in project_set.sheets {
            project_paths.insert(path.clone());
            if !self.document_state.engines.contains_key(&path) {
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
                    let prefix: String = sym
                        .reference
                        .chars()
                        .take_while(|c| c.is_ascii_alphabetic())
                        .collect();
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
                }
                Err(err) => {
                    crate::diagnostics::log_info(format!(
                        "Reset Duplicate Designators: save failed for {}: {err}",
                        path.display(),
                    ));
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
        }
        crate::diagnostics::log_info(format!(
            "Reset Duplicate Designators: reset {} duplicate reference(s) across {} sheet(s)",
            duplicates.len(),
            resets,
        ));
        Task::none()
    }

    pub(crate) fn handle_annotate_order_changed(
        &mut self,
        order: super::super::super::state::AnnotateOrder,
    ) -> Task<Message> {
        self.ui_state.annotate_order = order;
        Task::none()
    }
}
