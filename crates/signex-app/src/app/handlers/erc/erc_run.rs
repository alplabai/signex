//! ERC run + diagnostics handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::*;

impl Signex {
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

        // First pass: collect every project sheet's snapshot keyed by its
        // absolute path (live engine snapshots for open tabs, disk parses for
        // the rest).
        let mut snapshots_by_path: std::collections::HashMap<
            std::path::PathBuf,
            crate::schematic_runtime::SchematicRenderSnapshot,
        > = std::collections::HashMap::new();

        // Active tab.
        if let Some(tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && let Some(snapshot) = self.active_render_snapshot()
        {
            snapshots_by_path.insert(tab.path.clone(), snapshot.clone());
        }
        // Cached tabs — engines for every open schematic tab live in
        // `document_state.engines`, keyed by path. The active one was
        // already handled above via the render snapshot.
        for (idx, tab) in self.document_state.tabs.iter().enumerate() {
            if idx == self.document_state.active_tab {
                continue;
            }
            if let Some(engine) = self.document_state.engines.get(&tab.path) {
                snapshots_by_path.insert(tab.path.clone(), engine.document().clone());
            }
        }
        // The project itself, from the one assembler the export and the cached
        // canvas netlist also use: the declared pages *plus* everything
        // reachable down the `child_sheets` graph. Reading the declared pages
        // alone is what made `BadHierSheetPin` fire on a child that was
        // sitting unopened on disk beside its parent (#406).
        let (_pages, project_set) =
            crate::app::project_sheets::assemble_active_project_sheets(&self.document_state);

        // The child sheet-map keyed by the exact `ChildSheet.filename` each
        // parent references (not the bare basename) — the shared view the
        // netlist stitcher and ERC's `BadHierSheetPin` both read (ADR-0002 D8).
        // Built from the project's own sheets only: an open tab belonging to
        // some *other* project must never resolve this project's child
        // references.
        let children = crate::app::project_sheets::project_children_map(&project_set.sheets);

        // ERC still reports on every sheet the user has open — a loose tab is
        // not part of the project, but it is on screen and its violations are
        // wanted — plus every project sheet, opened or not.
        for (path, sheet) in project_set.sheets {
            snapshots_by_path.entry(path).or_insert(sheet);
        }

        // Second pass: run ERC with the shared children map so
        // BadHierSheetPin can cross-check each sheet symbol against
        // the actual child schematic.
        for (path, snapshot) in &snapshots_by_path {
            let violations = if let Some(eval_fns) = dsl_eval_fns.as_ref() {
                apply_overrides(signex_erc::run_with_project_and_dsl(
                    snapshot, &children, eval_fns,
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
            .active_document_project()
            .map(|p| p.dir().to_path_buf())?;

        let dsl_path_candidates = [
            project_root.join("erc.dsl"),
            project_root.join("signex.erc.dsl"),
        ];
        let dsl_path = dsl_path_candidates.iter().find(|p| p.exists())?;

        let Ok(src) = std::fs::read_to_string(dsl_path) else {
            crate::diagnostics::log_info(format!("ERC DSL: failed to read {}", dsl_path.display()));
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
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        self.ui_state.erc_violations = violations;
    }

    pub(crate) fn build_erc_diagnostic_entries(&self) -> Vec<crate::panels::ErcDiagnosticEntry> {
        let mut paths: Vec<_> = self
            .ui_state
            .erc_violations_by_path
            .keys()
            .cloned()
            .collect();
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
                        rule_kind: v.rule,
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

    /// Quick Fix dispatch from the Messages-panel chip.
    /// `UnusedPin` places a `NoConnect` at the violation's world
    /// coords and re-runs ERC so the row disappears immediately;
    /// every other rule falls back to the row-click "zoom + select"
    /// path, which is exactly the affordance the user wants 90% of
    /// the time even without a mutating fix.
    pub(crate) fn handle_erc_quick_fix(&mut self, index: usize) -> Task<Message> {
        let entries = self.build_erc_diagnostic_entries();
        if entries.is_empty() {
            return Task::none();
        }
        let clamped = index.min(entries.len() - 1);
        let target = entries[clamped].clone();

        // Open + activate the violation's sheet first, regardless of
        // rule kind — the engine command path operates on the active
        // engine, and even the non-mutating rules want the canvas to
        // scroll to the offending point.
        self.ensure_sheet_open_and_active(&target.sheet_path);

        match target.rule_kind {
            signex_erc::RuleKind::UnusedPin => {
                let nc = signex_types::schematic::NoConnect {
                    uuid: uuid::Uuid::new_v4(),
                    position: signex_types::schematic::Point::new(target.world_x, target.world_y),
                };
                self.apply_engine_command(
                    signex_engine::Command::PlaceNoConnect { no_connect: nc },
                    false,
                    false,
                );
                // Re-run ERC so the cleared violation drops out of
                // the panel without forcing the user to press F8.
                let _ = self.handle_run_erc();
                // Focus the cleared point so the new NoConnect marker
                // is visible — a small but reassuring "the fix landed
                // here" cue.
                self.handle_focus_at(target.world_x, target.world_y, None)
            }
            _ => {
                self.ui_state.erc_focus_global_index = Some(clamped);
                self.handle_focus_at(target.world_x, target.world_y, target.select)
            }
        }
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
        if let Some(index) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| &tab.path == path)
        {
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
            .map(|e| matches!(e, "snxsch"))
            .unwrap_or(false);
        if !is_schematic {
            return;
        }

        let Ok(text) = std::fs::read_to_string(path) else {
            crate::diagnostics::log_info(format!(
                "ERC navigation: failed to read sheet {}",
                path.display()
            ));
            return;
        };
        let Ok(sheet) = signex_types::format::SnxSchematic::parse(&text).map(|snx| snx.sheet)
        else {
            crate::diagnostics::log_info(format!(
                "ERC navigation: failed to parse sheet {}",
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
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
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
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        Task::none()
    }
}
