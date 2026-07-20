//! Annotate + duplicate-designator reset handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::*;

impl Signex {
    pub(crate) fn handle_annotate(&mut self, mode: signex_engine::AnnotateMode) -> Task<Message> {
        use std::path::PathBuf;
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

        // Pass B: apply to cached (non-active) tabs via the shared counter.
        let locked = self.ui_state.annotate_locked.clone();
        let mut any_cached_changed = false;
        let active_idx = self.document_state.active_tab;
        let paths: Vec<(usize, PathBuf)> = self
            .document_state
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(idx, tab)| {
                // This project's sheets only — a loose tab from elsewhere was
                // never seeded, so it must not draw from this counter.
                if idx == active_idx || !project_set.sheets.contains_key(&tab.path) {
                    None
                } else {
                    Some((idx, tab.path.clone()))
                }
            })
            .collect();
        for (idx, path) in paths {
            let result = self.document_state.engines.get_mut(&path).map(|engine| {
                engine.annotate_with_seed_and_locks(mode, &mut next_by_prefix, &locked)
            });
            if let Some(Ok(true)) = result {
                if let Some(tab) = self.document_state.tabs.get_mut(idx) {
                    tab.dirty = true;
                }
                any_cached_changed = true;
            }
        }

        // Pass B2: every project sheet that isn't currently open as a tab —
        // annotate the already-parsed copy with the shared counter and write
        // it back. Altium's Annotate-Across-Project covers even the sheets
        // the user hasn't opened so designators stay unique project-wide.
        let mut unopened_sheets: Vec<(
            std::path::PathBuf,
            signex_types::schematic::SchematicSheet,
        )> = project_set
            .sheets
            .iter()
            .filter(|(path, _)| !self.document_state.engines.contains_key(*path))
            .map(|(path, sheet)| (path.clone(), sheet.clone()))
            .collect();
        // Deterministic: `sheets` is a `HashMap`, and the counter hands out
        // numbers in walk order.
        unopened_sheets.sort_by(|a, b| a.0.cmp(&b.0));
        let mut disk_touched = 0usize;
        for (path, why) in &project_set.unreadable {
            crate::diagnostics::log_info(format!("Annotate: sheet '{}' {why}", path.display()));
        }
        for (sheet_path, sheet) in unopened_sheets {
            let Ok(mut engine) = signex_engine::Engine::new(sheet) else {
                continue;
            };
            engine.set_path(Some(sheet_path.clone()));
            let Ok(changed) =
                engine.annotate_with_seed_and_locks(mode, &mut next_by_prefix, &locked)
            else {
                continue;
            };
            if !changed {
                continue;
            }
            if engine.save().is_ok() {
                disk_touched += 1;
                self.document_state.dirty_paths.remove(&sheet_path);
                crate::diagnostics::log_info(format!("Annotate: saved {}", sheet_path.display()));
            }
        }

        // Pass C: apply to the active engine so the canvas, Properties
        // panel, and render cache all refresh. Run through the raw engine
        // method (not Command) so it shares the same counter.
        if let Some(engine) = self.document_state.active_engine_mut() {
            let _ = engine.annotate_with_seed_and_locks(mode, &mut next_by_prefix, &locked);
        }
        if disk_touched > 0 {
            crate::diagnostics::log_info(format!(
                "Annotate: wrote {} unopened sheet file(s) to disk",
                disk_touched,
            ));
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
        use std::collections::{HashMap, HashSet};
        use std::path::PathBuf;

        // Phase 1: walk every sheet and count how many symbols hold
        // each (non-power, non-hash) reference.
        let mut counts: HashMap<String, usize> = HashMap::new();
        let bump = |counts: &mut HashMap<String, usize>,
                    sheet: &signex_types::schematic::SchematicSheet| {
            for sym in &sheet.symbols {
                if sym.is_power || sym.reference.starts_with('#') {
                    continue;
                }
                if sym.reference.ends_with('?') {
                    continue;
                }
                *counts.entry(sym.reference.clone()).or_insert(0) += 1;
            }
        };
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
        let mut unopened: Vec<(PathBuf, signex_types::schematic::SchematicSheet)> = Vec::new();
        let mut project_paths: HashSet<PathBuf> = HashSet::new();
        for (path, sheet) in project_set.sheets {
            bump(&mut counts, &sheet);
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

        // Phase 2: anything seen more than once is a duplicate that
        // needs resetting.
        let duplicates: HashSet<String> = counts
            .into_iter()
            .filter_map(|(r, n)| if n > 1 { Some(r) } else { None })
            .collect();
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
                if dupes.contains(&sym.reference) {
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
        let mut any_active_changed = false;
        // Active engine — goes through ReplaceDocument so undo records
        // the snapshot.
        if let Some(engine) = self.document_state.active_engine_mut() {
            let mut sheet = engine.document().clone();
            if reset_in(&mut sheet, &duplicates) {
                let _ = engine.execute(signex_engine::Command::ReplaceDocument { document: sheet });
                any_active_changed = true;
                resets += 1;
            }
        }
        // Cached tabs — same ReplaceDocument path; each tab's own
        // history records the reset.
        let active_idx = self.document_state.active_tab;
        let cached_paths: Vec<(usize, PathBuf)> = self
            .document_state
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(idx, tab)| {
                // Only this project's sheets: a loose tab from somewhere else
                // was never counted, so it must not be reset either.
                if idx == active_idx || !project_paths.contains(&tab.path) {
                    None
                } else {
                    Some((idx, tab.path.clone()))
                }
            })
            .collect();
        for (idx, path) in cached_paths {
            let applied = self.document_state.engines.get_mut(&path).map(|engine| {
                let mut sheet = engine.document().clone();
                if reset_in(&mut sheet, &duplicates) {
                    let _ =
                        engine.execute(signex_engine::Command::ReplaceDocument { document: sheet });
                    true
                } else {
                    false
                }
            });
            if let Some(true) = applied {
                if let Some(tab) = self.document_state.tabs.get_mut(idx) {
                    tab.dirty = true;
                }
                self.document_state.dirty_paths.insert(path);
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

        if any_active_changed {
            self.interaction_state
                .active_canvas_mut()
                .clear_content_cache();
            self.sync_canvas_from_visible_schematic(
                crate::schematic_runtime::RenderInvalidation::FULL,
            );
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
