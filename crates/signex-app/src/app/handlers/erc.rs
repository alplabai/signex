use iced::Task;

use super::super::*;

impl Signex {
    /// Run ERC on the active schematic snapshot. Results land in
    /// `ui_state.erc_violations` and are displayed by the Messages panel.
    pub(crate) fn handle_run_erc(&mut self) -> Task<Message> {
        let Some(snapshot) = self.active_render_snapshot() else {
            self.ui_state.erc_violations.clear();
            return Task::none();
        };
        let mut violations = signex_erc::run(snapshot);
        // Apply severity overrides from prefs.
        for v in &mut violations {
            if let Some(&sev) = self.ui_state.erc_severity_override.get(&v.rule) {
                v.severity = sev;
            }
        }
        // Drop violations the user disabled.
        violations.retain(|v| v.severity != signex_erc::Severity::Off);
        // Sort: errors first, then warnings, then info; within a bucket,
        // by rule kind so the order is stable.
        violations.sort_by_key(|v| {
            let bucket = match v.severity {
                signex_erc::Severity::Error => 0,
                signex_erc::Severity::Warning => 1,
                signex_erc::Severity::Info => 2,
                signex_erc::Severity::Off => 3,
            };
            (bucket, format!("{:?}", v.rule))
        });
        crate::diagnostics::log_info(&format!(
            "ERC: {} violations on active sheet",
            violations.len(),
        ));
        self.ui_state.erc_violations = violations;
        // Surface the Messages panel so the user can see the results.
        self.document_state.dock.add_panel(
            crate::dock::PanelPosition::Bottom,
            crate::panels::PanelKind::Messages,
        );
        Task::none()
    }

    /// Move the viewport to center on a world-space point and optionally
    /// replace the current selection. Used by the Messages panel's
    /// click-to-zoom and by future Find/Replace result navigation.
    pub(crate) fn handle_focus_at(
        &mut self,
        world_x: f64,
        world_y: f64,
        select: Option<signex_types::schematic::SelectedItem>,
    ) -> Task<Message> {
        if let Some(item) = select {
            self.interaction_state.canvas.selected = vec![item];
            self.update_selection_info();
            self.interaction_state.canvas.clear_overlay_cache();
        }
        // Stage a fit target around the violation point so the canvas's
        // next draw centers on it.
        let half = 20.0_f32;
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
        self.interaction_state.canvas.clear_content_cache();
        self.interaction_state.canvas.clear_overlay_cache();
        Task::none()
    }

    pub(crate) fn handle_annotate(
        &mut self,
        mode: signex_engine::AnnotateMode,
    ) -> Task<Message> {
        // Share one per-prefix counter across every open sheet so designators
        // don't collide across sheets of the same project.
        let mut next_by_prefix: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let tab_count = self.document_state.tabs.len();

        // Pass A: seed the shared counter from every sheet's already-
        // annotated symbols (cached + active). This happens inside
        // annotate_with_seed's phase 2, but running a separate seed pass
        // first ensures order-independence — without this, sheet B could
        // reuse numbers it considers free that sheet A actually claims.
        let mut all_existing: Vec<String> = Vec::new();
        if let Some(eng) = self.document_state.engine.as_ref() {
            for sym in &eng.document().symbols {
                if !sym.is_power && !sym.reference.starts_with('#') {
                    all_existing.push(sym.reference.clone());
                }
            }
        }
        for tab in &self.document_state.tabs {
            if let Some(TabDocument::Schematic(session)) = tab.cached_document.as_ref() {
                for sym in &session.document().symbols {
                    if !sym.is_power && !sym.reference.starts_with('#') {
                        all_existing.push(sym.reference.clone());
                    }
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
        let mut any_cached_changed = false;
        for (idx, tab) in self.document_state.tabs.iter_mut().enumerate() {
            if idx == self.document_state.active_tab {
                continue;
            }
            if let Some(TabDocument::Schematic(session)) = tab.cached_document.as_mut() {
                if let Ok(changed) = session
                    .engine_mut()
                    .annotate_with_seed(mode, &mut next_by_prefix)
                {
                    if changed {
                        session.set_dirty(true);
                        tab.dirty = true;
                        any_cached_changed = true;
                    }
                }
            }
        }

        // Pass C: apply to the active engine so the canvas, Properties
        // panel, and render cache all refresh. Run through the raw engine
        // method (not Command) so it shares the same counter.
        if let Some(engine) = self.document_state.engine.as_mut() {
            let _ = engine.annotate_with_seed(mode, &mut next_by_prefix);
        }
        // Force a render + panel refresh as if a command had fired.
        self.interaction_state.canvas.clear_content_cache();
        self.sync_canvas_from_visible_schematic(
            signex_render::schematic::RenderInvalidation::FULL,
        );
        self.update_selection_info();
        if any_cached_changed || self.document_state.engine.is_some() {
            self.refresh_panel_ctx();
        }

        self.ui_state.annotate_reset_confirm = false;
        crate::diagnostics::log_info(&format!(
            "Annotated symbols across {} sheet(s) ({:?})",
            tab_count.max(1),
            mode,
        ));
        Task::none()
    }

    pub(crate) fn handle_open_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = true;
        self.interaction_state.context_menu = None;
        Task::none()
    }

    pub(crate) fn handle_close_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = false;
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
        Task::none()
    }

    pub(crate) fn handle_close_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = false;
        Task::none()
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
        Task::none()
    }

    pub(crate) fn handle_open_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = true;
        Task::none()
    }

    pub(crate) fn handle_close_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = false;
        Task::none()
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
        Task::none()
    }
}
