use iced::Task;

use super::super::*;

impl Signex {
    /// Run ERC across every sheet in the project — open tabs (live
    /// engine or cached session) plus every sheet in `project_data`
    /// that isn't open yet (parsed on-the-fly). Results are cached
    /// per-path in `erc_violations_by_path`; the visible list
    /// `erc_violations` repoints at the active sheet's entry so the
    /// Messages panel + canvas markers stay consistent with what's
    /// on screen.
    pub(crate) fn handle_run_erc(&mut self) -> Task<Message> {
        let dsl_eval_fns = self.load_project_dsl_eval_fns();
        let overrides = self.ui_state.erc_severity_override.clone();
        let apply_overrides =
            |mut violations: Vec<signex_erc::Violation>| -> Vec<signex_erc::Violation> {
                for v in &mut violations {
                    if let Some(&sev) = overrides.get(&v.rule) {
                        v.severity = sev;
                    }
                }
                violations.retain(|v| v.severity != signex_erc::Severity::Off);
                violations.sort_by_key(|v| {
                    let bucket = match v.severity {
                        signex_erc::Severity::Error => 0,
                        signex_erc::Severity::Warning => 1,
                        signex_erc::Severity::Info => 2,
                        signex_erc::Severity::Off => 3,
                    };
                    (bucket, format!("{:?}", v.rule))
                });
                violations
            };

        let mut by_path: std::collections::HashMap<std::path::PathBuf, Vec<signex_erc::Violation>> =
            std::collections::HashMap::new();

        // First pass: collect every sheet's snapshot keyed by BOTH its
        // absolute path AND its bare filename. BadHierSheetPin looks up
        // children by the filename stored on the parent's sheet symbol,
        // which is often just the basename (e.g. "power.standard_sch").
        let mut snapshots_by_path: std::collections::HashMap<
            std::path::PathBuf,
            signex_render::schematic::SchematicRenderSnapshot,
        > = std::collections::HashMap::new();
        let mut children: std::collections::HashMap<
            String,
            signex_render::schematic::SchematicRenderSnapshot,
        > = std::collections::HashMap::new();

        let open_paths: std::collections::HashSet<std::path::PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect();
        let project_root = self
            .document_state
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(std::path::PathBuf::from));

        let push_snap = |path: std::path::PathBuf,
                         snap: signex_render::schematic::SchematicRenderSnapshot,
                         by_path_out: &mut std::collections::HashMap<
            std::path::PathBuf,
            signex_render::schematic::SchematicRenderSnapshot,
        >,
                         children_out: &mut std::collections::HashMap<
            String,
            signex_render::schematic::SchematicRenderSnapshot,
        >| {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                children_out.insert(name.to_string(), snap.clone());
            }
            by_path_out.insert(path, snap);
        };

        // Active tab.
        if let Some(tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && let Some(snapshot) = self.active_render_snapshot()
        {
            push_snap(
                tab.path.clone(),
                snapshot.clone(),
                &mut snapshots_by_path,
                &mut children,
            );
        }
        // Cached tabs — engines for every open schematic tab live in
        // `document_state.engines`, keyed by path. The active one was
        // already handled above via the render snapshot.
        for (idx, tab) in self.document_state.tabs.iter().enumerate() {
            if idx == self.document_state.active_tab {
                continue;
            }
            if let Some(engine) = self.document_state.engines.get(&tab.path) {
                let snapshot = signex_render::schematic::SchematicRenderSnapshot::from_sheet(
                    engine.document(),
                );
                push_snap(
                    tab.path.clone(),
                    snapshot,
                    &mut snapshots_by_path,
                    &mut children,
                );
            }
        }
        // Unopened project sheets.
        if let Some(pd) = self.document_state.project_data.as_ref() {
            for sheet in &pd.sheets {
                let path = match project_root.as_ref() {
                    Some(root) => root.join(&sheet.filename),
                    None => std::path::PathBuf::from(&sheet.filename),
                };
                if open_paths.contains(&path) || snapshots_by_path.contains_key(&path) {
                    continue;
                }
                let Ok(parsed) = standard_parser::parse_schematic_file(&path) else {
                    continue;
                };
                let snapshot =
                    signex_render::schematic::SchematicRenderSnapshot::from_sheet(&parsed);
                push_snap(path, snapshot, &mut snapshots_by_path, &mut children);
            }
        }

        // Second pass: run ERC with the shared children map so
        // BadHierSheetPin can cross-check each sheet symbol against
        // the actual child schematic.
        for (path, snapshot) in &snapshots_by_path {
            let violations = if let Some(eval_fns) = dsl_eval_fns.as_ref() {
                apply_overrides(signex_erc::run_with_project_and_dsl(
                    snapshot,
                    &children,
                    eval_fns,
                ))
            } else {
                apply_overrides(signex_erc::run_with_project(snapshot, &children))
            };
            by_path.insert(path.clone(), violations);
        }

        let total: usize = by_path.values().map(|v| v.len()).sum();
        crate::diagnostics::log_info(format!(
            "ERC: {} total violations across {} sheet(s)",
            total,
            by_path.len(),
        ));

        // Repoint the visible list + canvas markers at the active
        // sheet's entry. Updates on tab switch via `sync_active_tab`.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.path.clone());
        self.ui_state.erc_violations_by_path = by_path;
        self.ui_state.erc_focus_global_index = self
            .ui_state
            .erc_violations_by_path
            .values()
            .any(|v| !v.is_empty())
            .then_some(0);
        self.refresh_active_erc_from_cache(active_path.as_ref());

        // Surface the ERC panel so the user can see the results.
        self.document_state.dock.add_panel(
            crate::dock::PanelPosition::Bottom,
            crate::panels::PanelKind::Erc,
        );
        Task::none()
    }

    fn load_project_dsl_eval_fns(&self) -> Option<Vec<signex_erc::engine::EvalFn>> {
        let project_root = self
            .document_state
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(std::path::PathBuf::from))?;

        let dsl_path_candidates = [
            project_root.join("erc.dsl"),
            project_root.join("signex.erc.dsl"),
        ];
        let dsl_path = dsl_path_candidates.iter().find(|p| p.exists())?;

        let Ok(src) = std::fs::read_to_string(dsl_path) else {
            crate::diagnostics::log_info(format!(
                "ERC DSL: failed to read {}",
                dsl_path.display()
            ));
            return None;
        };

        match signex_erc_dsl::parse_validate_compile_to_eval_fns(&src) {
            Ok(eval_fns) => {
                crate::diagnostics::log_info(format!(
                    "ERC DSL: loaded {} compiled rule(s) from {}",
                    eval_fns.len(),
                    dsl_path.display()
                ));
                Some(eval_fns)
            }
            Err(errors) => {
                for error in errors {
                    crate::diagnostics::log_info(format!("ERC DSL: {error}"));
                }
                None
            }
        }
    }

    /// Repoint `erc_violations` + canvas markers at whatever the
    /// per-sheet cache holds for `active_path`. Empty vec when the
    /// sheet has never had ERC run, which is the right behaviour
    /// pre-Run-ERC.
    pub(crate) fn refresh_active_erc_from_cache(
        &mut self,
        active_path: Option<&std::path::PathBuf>,
    ) {
        let violations = active_path
            .and_then(|p| self.ui_state.erc_violations_by_path.get(p))
            .cloned()
            .unwrap_or_default();
        self.interaction_state.active_canvas_mut().erc_markers = violations
            .iter()
            .map(|v| crate::canvas::ErcMarker {
                x: v.location.x,
                y: v.location.y,
                severity: match v.severity {
                    signex_erc::Severity::Error => crate::canvas::ErcMarkerSeverity::Error,
                    signex_erc::Severity::Warning => crate::canvas::ErcMarkerSeverity::Warning,
                    _ => crate::canvas::ErcMarkerSeverity::Info,
                },
                primary_uuid: v.primary.as_ref().map(|s| s.uuid),
            })
            .collect();
        self.interaction_state.active_canvas_mut().clear_overlay_cache();
        self.ui_state.erc_violations = violations;
    }

    pub(crate) fn build_erc_diagnostic_entries(&self) -> Vec<crate::panels::ErcDiagnosticEntry> {
        let mut paths: Vec<_> = self.ui_state.erc_violations_by_path.keys().cloned().collect();
        paths.sort();

        let mut out = Vec::new();
        for path in paths {
            let sheet_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            if let Some(list) = self.ui_state.erc_violations_by_path.get(&path) {
                for v in list {
                    out.push(crate::panels::ErcDiagnosticEntry {
                        global_index: out.len(),
                        sheet_name: sheet_name.clone(),
                        sheet_path: path.clone(),
                        severity: match v.severity {
                            signex_erc::Severity::Error => crate::panels::ErcSeverityLite::Error,
                            signex_erc::Severity::Warning => {
                                crate::panels::ErcSeverityLite::Warning
                            }
                            _ => crate::panels::ErcSeverityLite::Info,
                        },
                        rule_label: v.rule.label(),
                        message: v.message.clone(),
                        world_x: v.location.x,
                        world_y: v.location.y,
                        select: v.primary,
                    });
                }
            }
        }

        out
    }

    pub(crate) fn handle_focus_erc_diagnostic_offset(&mut self, delta: isize) -> Task<Message> {
        let entries = self.build_erc_diagnostic_entries();
        if entries.is_empty() {
            self.ui_state.erc_focus_global_index = None;
            return Task::none();
        }
        let current = self.ui_state.erc_focus_global_index.unwrap_or(0) as isize;
        let len = entries.len() as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        self.handle_focus_erc_diagnostic_index(next)
    }

    pub(crate) fn handle_focus_erc_diagnostic_index(&mut self, index: usize) -> Task<Message> {
        let entries = self.build_erc_diagnostic_entries();
        if entries.is_empty() {
            self.ui_state.erc_focus_global_index = None;
            return Task::none();
        }
        let clamped = index.min(entries.len() - 1);
        let target = entries[clamped].clone();

        // Ensure the target sheet is open and active before focusing its point.
        self.ensure_sheet_open_and_active(&target.sheet_path);

        self.ui_state.erc_focus_global_index = Some(clamped);
        self.handle_focus_at(target.world_x, target.world_y, target.select)
    }

    fn ensure_sheet_open_and_active(&mut self, path: &std::path::PathBuf) {
        if let Some(index) = self.document_state.tabs.iter().position(|tab| &tab.path == path) {
            if index != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = index;
                self.sync_active_tab();
            }
            return;
        }

        let is_schematic = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e, "standard_sch" | "snxsch"))
            .unwrap_or(false);
        if !is_schematic {
            return;
        }

        let Ok(sheet) = standard_parser::parse_schematic_file(path) else {
            crate::diagnostics::log_info(format!(
                "ERC navigation: failed to open sheet {}",
                path.display()
            ));
            return;
        };
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "sheet".to_string());
        self.open_schematic_tab(path.clone(), title, sheet);
        self.sync_active_tab();
    }

    /// Move the viewport to center on a world-space point and optionally
    /// replace the current selection. Used by the ERC panel's
    /// click-to-zoom and by future Find/Replace result navigation.
    pub(crate) fn handle_focus_at(
        &mut self,
        world_x: f64,
        world_y: f64,
        select: Option<signex_types::schematic::SelectedItem>,
    ) -> Task<Message> {
        if let Some(item) = select {
            self.interaction_state.active_canvas_mut().selected = vec![item];
            self.update_selection_info();
            self.interaction_state.active_canvas_mut().clear_overlay_cache();
        }
        // Stage a tighter fit target around the target point so
        // navigation jumps both center and zoom in to the issue.
        let half = 8.0_f32;
        self.interaction_state
            .canvas
            .pending_fit
            .set(Some(iced::Rectangle {
                x: world_x as f32 - half,
                y: world_y as f32 - half,
                width: half * 2.0,
                height: half * 2.0,
            }));
        Task::none()
    }

    pub(crate) fn handle_toggle_auto_focus(&mut self) -> Task<Message> {
        self.ui_state.auto_focus = !self.ui_state.auto_focus;
        // Mirror the flag onto the canvas so the renderer can compute
        // the focus uuid set locally without reaching into app state.
        self.interaction_state.active_canvas_mut().auto_focus = self.ui_state.auto_focus;
        self.interaction_state.active_canvas_mut().clear_content_cache();
        self.interaction_state.active_canvas_mut().clear_overlay_cache();
        Task::none()
    }

    pub(crate) fn handle_annotate(&mut self, mode: signex_engine::AnnotateMode) -> Task<Message> {
        use std::path::PathBuf;
        // Share one per-prefix counter across every open sheet so designators
        // don't collide across sheets of the same project.
        let mut next_by_prefix: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let tab_count = self.document_state.tabs.len();

        // Pass A: seed the shared counter from every sheet's already-
        // annotated symbols. This happens inside annotate_with_seed's
        // phase 2, but running a separate seed pass first ensures
        // order-independence — without this, sheet B could reuse
        // numbers it considers free that sheet A actually claims.
        // Every open schematic tab's engine lives in the HashMap, so
        // one pass over `engines.values()` covers active + background.
        let mut all_existing: Vec<String> = Vec::new();
        for engine in self.document_state.engines.values() {
            for sym in &engine.document().symbols {
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
                if idx == active_idx {
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

        // Pass B2: walk every sheet in the project that isn't currently
        // open as a tab — parse from disk, annotate with the shared
        // counter, and write back. Altium's Annotate-Across-Project
        // covers even the sheets the user hasn't opened so designators
        // stay unique project-wide.
        let open_paths: std::collections::HashSet<std::path::PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect();
        let project_root = self
            .document_state
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(std::path::PathBuf::from));
        let unopened_sheet_paths: Vec<std::path::PathBuf> = self
            .document_state
            .project_data
            .as_ref()
            .map(|pd| {
                pd.sheets
                    .iter()
                    .filter_map(|s| {
                        let path = match project_root.as_ref() {
                            Some(root) => root.join(&s.filename),
                            None => std::path::PathBuf::from(&s.filename),
                        };
                        if open_paths.contains(&path) {
                            None
                        } else {
                            Some(path)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut disk_touched = 0usize;
        for sheet_path in unopened_sheet_paths {
            let Ok(sheet) = standard_parser::parse_schematic_file(&sheet_path) else {
                crate::diagnostics::log_info(format!(
                    "Annotate: failed to parse unopened sheet {}",
                    sheet_path.display()
                ));
                continue;
            };
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
        self.interaction_state.active_canvas_mut().clear_content_cache();
        self.sync_canvas_from_visible_schematic(signex_render::schematic::RenderInvalidation::FULL);
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
        self.handle_detach_modal(super::super::state::ModalId::AnnotateDialog)
    }

    pub(crate) fn handle_close_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = false;
        self.close_detached_modal(super::super::state::ModalId::AnnotateDialog)
    }

    /// Altium's "Reset Duplicate Designators" — find references that
    /// appear on more than one symbol across the WHOLE project, reset
    /// just those to `{prefix}?`. Everything else keeps its current
    /// value. Walks open tabs (live + cached engines) and every sheet
    /// in `project_data.sheets` not opened as a tab; unopened sheets
    /// are re-saved through `standard-writer` so the fix is project-wide.
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
        // All open schematic engines live in the HashMap — one loop
        // covers the active tab plus every background tab.
        for engine in self.document_state.engines.values() {
            bump(&mut counts, engine.document());
        }
        let open_paths: HashSet<PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect();
        let project_root = self
            .document_state
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(PathBuf::from));
        let unopened_paths: Vec<PathBuf> = self
            .document_state
            .project_data
            .as_ref()
            .map(|pd| {
                pd.sheets
                    .iter()
                    .filter_map(|s| {
                        let path = match project_root.as_ref() {
                            Some(root) => root.join(&s.filename),
                            None => PathBuf::from(&s.filename),
                        };
                        if open_paths.contains(&path) {
                            None
                        } else {
                            Some(path)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Parse each unopened sheet ONCE up-front and keep the
        // `SchematicSheet` around for phase 2 — saves a second disk
        // parse and guarantees counting + reset see the same state.
        let mut unopened: Vec<(PathBuf, signex_types::schematic::SchematicSheet)> =
            Vec::with_capacity(unopened_paths.len());
        for path in unopened_paths {
            match standard_parser::parse_schematic_file(&path) {
                Ok(sheet) => {
                    bump(&mut counts, &sheet);
                    unopened.push((path, sheet));
                }
                Err(err) => {
                    crate::diagnostics::log_info(format!(
                        "Reset Duplicate Designators: failed to parse unopened sheet {}: {err}",
                        path.display(),
                    ));
                }
            }
        }

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
                if idx == active_idx {
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
                    let _ = engine.execute(signex_engine::Command::ReplaceDocument {
                        document: sheet,
                    });
                    true
                } else {
                    false
                }
            });
            if let Some(true) = applied {
                if let Some(tab) = self.document_state.tabs.get_mut(idx) {
                    tab.dirty = true;
                }
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
            self.interaction_state.active_canvas_mut().clear_content_cache();
            self.sync_canvas_from_visible_schematic(
                signex_render::schematic::RenderInvalidation::FULL,
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
        order: super::super::state::AnnotateOrder,
    ) -> Task<Message> {
        self.ui_state.annotate_order = order;
        Task::none()
    }

    pub(crate) fn handle_open_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = true;
        self.interaction_state.context_menu = None;
        self.handle_detach_modal(super::super::state::ModalId::ErcDialog)
    }

    pub(crate) fn handle_close_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = false;
        self.close_detached_modal(super::super::state::ModalId::ErcDialog)
    }

    pub(crate) fn handle_erc_severity_changed(
        &mut self,
        rule: signex_erc::RuleKind,
        severity: signex_erc::Severity,
    ) -> Task<Message> {
        if severity == rule.default_severity() {
            // Match default → remove override so the map stays minimal.
            self.ui_state.erc_severity_override.remove(&rule);
        } else {
            self.ui_state.erc_severity_override.insert(rule, severity);
        }
        // Persist so the override survives restart. Silent on I/O errors —
        // this is a preference, not critical state.
        crate::fonts::write_erc_severity_overrides(&self.ui_state.erc_severity_override);
        Task::none()
    }

    pub(crate) fn handle_open_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = true;
        self.handle_detach_modal(super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_close_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = false;
        self.close_detached_modal(super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_modal_drag_start(
        &mut self,
        modal: super::super::state::ModalId,
        x: f32,
        y: f32,
    ) -> Task<Message> {
        self.ui_state.modal_dragging = Some((modal, x, y));
        Task::none()
    }

    pub(crate) fn handle_modal_drag_end(&mut self) -> Task<Message> {
        self.ui_state.modal_dragging = None;
        self.ui_state.tab_dragging = None;
        Task::none()
    }

    /// Pop tab `idx` into its own OS window. The tab stays in
    /// `document_state.tabs` so reattach is a pure UI flip — closing the
    /// popped-out window via `SecondaryWindowClosed` just drops the entry
    /// from `ui_state.windows` and the tab re-appears in the tab bar.
    pub(crate) fn handle_undock_tab(&mut self, idx: usize) -> Task<Message> {
        let Some(tab) = self.document_state.tabs.get(idx) else {
            return Task::none();
        };
        let path = tab.path.clone();
        // Don't re-undock a tab that already has a window.
        if self.ui_state.windows.values().any(
            |k| matches!(k, super::super::state::WindowKind::UndockedTab { path: p, .. } if p == &path),
        ) {
            return Task::none();
        }
        let title = tab.title.clone();

        // Make the tab active so the duplicated view in the new window
        // lands on that tab's content. Main window's active_tab is
        // shared — if the user wants to keep editing a different tab in
        // main, they can switch after the window opens.
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }

        let size = iced::Size::new(1400.0, 900.0);
        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            decorations: true,
            ..Default::default()
        });
        // Stash immediately so the first frame in the new window has a
        // target; `UndockedTabOpened` refreshes the title afterwards.
        self.ui_state.windows.insert(
            id,
            super::super::state::WindowKind::UndockedTab {
                path: path.clone(),
                title,
            },
        );
        open_task.map(move |settled_id| Message::UndockedTabOpened {
            path: path.clone(),
            id: settled_id,
        })
    }

    /// Remove the floating panel at `idx` and open an OS window that
    /// renders that panel's content. Closing the OS window re-docks the
    /// panel to the right column — see `SecondaryWindowClosed` in
    /// dispatch/mod.rs.
    pub(crate) fn handle_detach_floating_panel(&mut self, idx: usize) -> Task<Message> {
        let Some(fp) = self.document_state.dock.floating.get(idx) else {
            return Task::none();
        };
        let kind = fp.kind;
        let size = iced::Size::new(fp.width.max(420.0), fp.height.max(360.0));
        self.document_state.dock.floating.remove(idx);

        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            decorations: true,
            ..Default::default()
        });
        self.ui_state
            .windows
            .insert(id, super::super::state::WindowKind::DetachedPanel(kind));
        open_task.map(move |settled_id| Message::DetachedPanelOpened {
            kind,
            id: settled_id,
        })
    }

    /// Find any OS window that currently hosts `modal` and request the
    /// OS to close it. Used by the in-body Close button so pressing Close
    /// inside a detached modal both dismisses the modal state and cleans
    /// up the popped-out window — without this, the window would stay
    /// open rendering an orphaned modal body.
    pub(crate) fn close_detached_modal(
        &mut self,
        modal: super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::state::WindowKind;
        let maybe_id = self.ui_state.windows.iter().find_map(|(id, kind)| {
            if matches!(kind, WindowKind::DetachedModal(m) if *m == modal) {
                Some(*id)
            } else {
                None
            }
        });
        if let Some(id) = maybe_id {
            self.ui_state.windows.remove(&id);
            iced::window::close(id)
        } else {
            Task::none()
        }
    }

    /// Pop `modal` out of the main window into its own OS window. The
    /// window's initial size matches the modal's in-app dimensions so the
    /// user sees continuity; position falls back to default (centered on
    /// the OS) since we don't know where to anchor absent monitor query.
    pub(crate) fn handle_detach_modal(
        &mut self,
        modal: super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::state::ModalId;
        // Don't open a second window for the same modal — treat detach
        // on an already-detached modal as a no-op.
        if self.ui_state.windows.values().any(
            |kind| matches!(kind, super::super::state::WindowKind::DetachedModal(m) if *m == modal),
        ) {
            return Task::none();
        }

        let size = match modal {
            ModalId::AnnotateDialog => iced::Size::new(1100.0, 760.0),
            ModalId::ErcDialog => iced::Size::new(1000.0, 600.0),
            ModalId::AnnotateResetConfirm => iced::Size::new(420.0, 180.0),
            ModalId::MoveSelection => iced::Size::new(420.0, 240.0),
            ModalId::NetColorPalette => iced::Size::new(520.0, 480.0),
            ModalId::ParameterManager => iced::Size::new(900.0, 560.0),
            ModalId::Preferences => iced::Size::new(900.0, 620.0),
            ModalId::FindReplace => iced::Size::new(420.0, 180.0),
            ModalId::CloseTabConfirm => iced::Size::new(420.0, 180.0),
        };

        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            // No OS chrome — the modal body supplies its own header with
            // an X close button and a click-to-drag region.
            decorations: false,
            ..Default::default()
        });
        // Stash the mapping right away — view(id) for the new window
        // fires before open_task resolves on some platforms, and without
        // the entry the detached window would render empty.
        self.ui_state
            .windows
            .insert(id, super::super::state::WindowKind::DetachedModal(modal));
        // When the OS finishes opening the window, forward the id so the
        // update can double-check and clear any leftover drag state.
        open_task.map(move |settled_id| Message::DetachedModalOpened {
            modal,
            id: settled_id,
        })
    }

    pub(crate) fn handle_open_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection = super::super::state::MoveSelectionState {
            open: true,
            dx: "0".to_string(),
            dy: "0".to_string(),
        };
        self.handle_detach_modal(super::super::state::ModalId::MoveSelection)
    }

    pub(crate) fn handle_close_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection.open = false;
        Task::none()
    }

    pub(crate) fn handle_move_selection_apply(&mut self) -> Task<Message> {
        let dx = self
            .ui_state
            .move_selection
            .dx
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        let dy = self
            .ui_state
            .move_selection
            .dy
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        if dx == 0.0 && dy == 0.0 {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        let items = self.interaction_state.active_canvas_mut().selected.clone();
        if items.is_empty() {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        if let Some(engine) = self.document_state.active_engine_mut() {
            let _ = engine.execute(signex_engine::Command::MoveSelection { items, dx, dy });
        }
        self.ui_state.move_selection.open = false;
        self.interaction_state.active_canvas_mut().clear_content_cache();
        self.interaction_state.active_canvas_mut().clear_overlay_cache();
        self.sync_canvas_from_visible_schematic(signex_render::schematic::RenderInvalidation::FULL);
        self.update_selection_info();
        Task::none()
    }

    pub(crate) fn handle_parameter_manager_edit(
        &mut self,
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    ) -> Task<Message> {
        if let Some(engine) = self.document_state.active_engine_mut() {
            let _ = engine.execute(signex_engine::Command::SetSymbolField {
                symbol_id: symbol_uuid,
                key,
                value,
            });
            self.interaction_state.active_canvas_mut().clear_content_cache();
            self.sync_canvas_from_visible_schematic(
                signex_render::schematic::RenderInvalidation::FULL,
            );
            self.refresh_panel_ctx();
        }
        Task::none()
    }

    /// Ask the OS to start a borderless-window drag for whichever window
    /// currently hosts `modal`. Wired to the decorations:false detached
    /// modal header so the user can move the window without an OS
    /// title bar.
    pub(crate) fn handle_start_detached_window_drag(
        &mut self,
        modal: super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::state::WindowKind;
        let id = self.ui_state.windows.iter().find_map(|(id, kind)| {
            if matches!(kind, WindowKind::DetachedModal(m) if *m == modal) {
                Some(*id)
            } else {
                None
            }
        });
        match id {
            Some(id) => iced::window::drag(id),
            None => Task::none(),
        }
    }
}
