//! ERC run + diagnostics handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::*;

impl Signex {
    /// The run-time ERC configuration: the user's pin-matrix overrides laid
    /// over the baseline matrix.
    pub(crate) fn erc_config(&self) -> signex_erc::ErcConfig {
        signex_erc::ErcConfig::with_pin_matrix_overrides(&self.ui_state.pin_matrix_overrides)
    }

    /// Start an ERC run. The disk reads and the rule pass run in
    /// `spawn_blocking`; [`Self::handle_erc_run_finished`] applies the result
    /// if no newer run superseded it (#111).
    pub(crate) fn handle_run_erc(&mut self) -> Task<Message> {
        let (generation, inputs) = self.begin_erc_run();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || super::erc_job::run_erc_job(inputs))
                    .await
                    .map_err(|e| format!("ERC worker failed: {e}"))
            },
            move |result| {
                Message::Erc(ErcMsg::RunFinished {
                    generation,
                    result: std::sync::Arc::new(result),
                })
            },
        )
    }

    /// Run ERC to completion on the calling thread - tests only.
    #[cfg(test)]
    pub(crate) fn run_erc_blocking(&mut self) {
        let (generation, inputs) = self.begin_erc_run();
        let outcome = Ok(super::erc_job::run_erc_job(inputs));
        self.apply_erc_run_result(generation, &outcome);
    }

    /// Capture the run's inputs from live state (memory only, no IO), claim a
    /// new generation and flag the run as in flight.
    fn begin_erc_run(&mut self) -> (u64, super::erc_job::ErcRunInputs) {
        let ds = &self.document_state;
        let tab_paths = ds
            .tabs
            .iter()
            .filter(|t| ds.engines.contains_key(&t.path))
            .map(|t| t.path.clone())
            .collect();
        let active = ds
            .tabs
            .get(ds.active_tab)
            .zip(self.active_render_snapshot())
            .map(|(tab, snapshot)| (tab.path.clone(), snapshot.clone()));
        let pages = crate::app::project_sheets::declared_pages(ds);
        let root = ds
            .active_path
            .clone()
            .map(|active| crate::app::project_sheets::resolve_netlist_root(ds).unwrap_or(active));
        let inputs = super::erc_job::ErcRunInputs {
            open: crate::app::project_sheets::open_sheets_snapshot(ds),
            tab_paths,
            active,
            pages,
            root,
            project_dir: ds.active_document_project().map(|p| p.dir().to_path_buf()),
            overrides: self.ui_state.erc_severity_override.clone(),
            config: self.erc_config(),
            fold: crate::app::state::project_case::fold_of(ds.active_document_project()),
        };

        self.ui_state.erc_run_generation += 1;
        self.ui_state.erc_running = true;
        self.refresh_panel_ctx();
        (self.ui_state.erc_run_generation, inputs)
    }

    /// A finished ERC run: apply it, then surface the ERC panel.
    pub(crate) fn handle_erc_run_finished(
        &mut self,
        generation: u64,
        result: &Result<super::erc_job::ErcRunOutcome, String>,
    ) -> Task<Message> {
        if !self.apply_erc_run_result(generation, result) {
            return Task::none();
        }
        // Surface the ERC panel so the user can see the results.
        // `add_panel` could not do that: with ERC already docked it
        // returned early, leaving the results behind whatever tab was
        // active. `show_panel` makes it the active tab (#641).
        self.show_panel(crate::panels::PanelKind::Erc)
    }

    /// Apply a finished run's result - unless a newer run has started since.
    /// Returns whether violations were applied.
    fn apply_erc_run_result(
        &mut self,
        generation: u64,
        result: &Result<super::erc_job::ErcRunOutcome, String>,
    ) -> bool {
        if generation != self.ui_state.erc_run_generation {
            return false;
        }
        self.ui_state.erc_running = false;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(why) => {
                crate::diagnostics::log_warning(why);
                self.refresh_panel_ctx();
                return false;
            }
        };
        for warning in &outcome.warnings {
            crate::diagnostics::log_warning(warning);
        }
        for info in &outcome.infos {
            crate::diagnostics::log_info(info);
        }

        // Repoint the visible list + canvas markers at the active
        // sheet's entry. Updates on tab switch via `sync_active_tab`.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.path.clone());
        self.ui_state.erc_violations_by_path = outcome.by_path.clone();
        self.ui_state.erc_focus_global_index = self
            .ui_state
            .erc_violations_by_path
            .values()
            .any(|v| !v.is_empty())
            .then_some(0);
        self.refresh_active_erc_from_cache(active_path.as_ref());
        self.refresh_panel_ctx();
        true
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
        // Only a point in THIS sheet's own coordinates may be drawn here; a
        // child-space or unplaced one would land on empty canvas (#956).
        self.interaction_state.active_canvas_mut().erc_markers = violations
            .iter()
            .filter(|v| v.location_space == signex_erc::LocationSpace::ThisSheet)
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
                        rule_label: v.display_name(),
                        rule_kind: v.builtin_kind(),
                        message: v.message.clone(),
                        world_x: v.location.x,
                        world_y: v.location.y,
                        select: v.primary,
                        locatable: v.location_space == signex_erc::LocationSpace::ThisSheet,
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
        if !target.locatable {
            // No point on any open sheet to go to: report, don't pan.
            self.ui_state.erc_focus_global_index = Some(clamped);
            return Task::none();
        }
        self.ensure_sheet_open_and_active(&target.sheet_path);

        match target.rule_kind {
            Some(signex_erc::RuleKind::UnusedPin) => {
                let nc = signex_types::schematic::NoConnect {
                    uuid: uuid::Uuid::new_v4(),
                    position: signex_types::schematic::Point::new(target.world_x, target.world_y),
                };
                self.apply_engine_command(
                    signex_engine::Command::PlaceNoConnect { no_connect: nc },
                    false,
                    false,
                );
                // Re-run ERC so the cleared violation drops out of the
                // panel without forcing the user to press F8, then focus
                // the cleared point so the new NoConnect marker is visible.
                // The run is asynchronous; the two tasks are independent.
                Task::batch([
                    self.handle_run_erc(),
                    self.handle_focus_at(target.world_x, target.world_y, None),
                ])
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

        if !target.locatable {
            self.ui_state.erc_focus_global_index = Some(clamped);
            return Task::none();
        }

        // Ensure the target sheet is open and active before focusing its point.
        self.ensure_sheet_open_and_active(&target.sheet_path);

        self.ui_state.erc_focus_global_index = Some(clamped);
        self.handle_focus_at(target.world_x, target.world_y, target.select)
    }

    pub(crate) fn ensure_sheet_open_and_active(&mut self, path: &std::path::Path) {
        let path = &self.resolve_held_spelling(path.to_path_buf());
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

        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "sheet".to_string());
        // A dirty engine parked without a tab holds unsaved edits: reattach
        // it rather than replace it with the on-disk sheet (#1135).
        if self.path_is_owned_by_open_document(path) {
            self.attach_parked_schematic_tab(path.clone(), title);
            self.sync_active_tab();
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
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        Task::none()
    }
}

#[cfg(test)]
mod tests;
