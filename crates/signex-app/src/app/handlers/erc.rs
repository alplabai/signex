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
        self.apply_engine_command(
            signex_engine::Command::AnnotateAll { mode },
            false,
            true,
        );
        // Close any open dialogs that triggered this action.
        self.ui_state.annotate_dialog_open = false;
        self.ui_state.annotate_reset_confirm = false;
        crate::diagnostics::log_info(&format!("Annotated symbols ({:?})", mode));
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
}
