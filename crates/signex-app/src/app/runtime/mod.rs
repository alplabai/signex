use super::*;

mod footprint_ctx;
mod footprint_summaries;
mod history;
mod panel_ctx;
mod symbol_ctx;

impl Signex {
    /// Resolved `.snxlib` paths referenced by every loaded project's
    /// `Project.libraries` list — reserved for callers outside the
    /// Components Panel that still need a flat slice (the panel itself
    /// derives the same Vec from `ctx.projects[].libraries[].root`).
    #[allow(dead_code)]
    pub(crate) fn collect_project_library_paths(&self) -> Vec<std::path::PathBuf> {
        let mut out: Vec<std::path::PathBuf> = Vec::new();
        for p in &self.document_state.projects {
            for entry in &p.data.libraries {
                let resolved = p.data.resolve_library_path(entry);
                if !out.contains(&resolved) {
                    out.push(resolved);
                }
            }
        }
        out
    }

    pub(crate) fn finish_update(&mut self) -> Task<Message> {
        self.document_state.panel_ctx.unit = self.ui_state.unit;
        self.document_state.panel_ctx.grid_visible = self.ui_state.grid_visible;
        self.document_state.panel_ctx.snap_enabled = self.ui_state.snap_enabled;
        self.document_state.panel_ctx.grid_size_mm = self.ui_state.grid_size_mm;
        self.document_state.panel_ctx.visible_grid_mm = self.ui_state.visible_grid_mm;
        self.document_state.panel_ctx.snap_hotspots = self.ui_state.snap_hotspots;
        self.sync_diagnostics_panel_ctx();

        // Re-resolve the History panel's active path; on change, bump
        // the generation counter and kick off an async load. Stale
        // results check `generation == history.generation` and drop
        // themselves on mismatch.
        let history = self.refresh_history_panel();
        // v0.23 — Drain any queued git commits onto the iced task
        // pool so they run off the update thread. The "Saving…" pill
        // in the status bar reads from `inflight_git_commits` until
        // each Task::perform completion fires `ProjectGitCommitDone`.
        let commits = self.drain_pending_git_commits();
        Task::batch([history, commits])
    }

    fn sync_diagnostics_panel_ctx(&mut self) {
        self.document_state.panel_ctx.diagnostics_level =
            crate::diagnostics::configured_level_label().to_string();
        self.document_state.panel_ctx.diagnostics = crate::diagnostics::recent_entries();
    }

    pub(crate) fn sync_active_tab(&mut self) {
        // Follow the focused tab into its project: the Projects-panel
        // accent and active_project-scoped handlers (ERC / annotate /
        // save-all) should track the user's tab focus, not the most
        // recently opened project. Tabs with no `project_id` (loose
        // schematics opened without a `.standard_pro`) leave the pointer
        // alone so the panel keeps showing whichever project was last
        // active. (#54 phase 2.4)
        if let Some(pid) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.project_id)
        {
            self.document_state.active_project = Some(pid);
        }

        self.sync_visible_document_from_active_tab();
        // ERC results are cached per-sheet. On tab switch, repoint the visible
        // list/markers at the newly active sheet instead of dropping results.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.path.clone());
        self.refresh_active_erc_from_cache(active_path.as_ref());
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        // Always rebuild the panel context so the active-row highlight
        // and active-project accent track the focused tab even when
        // sync_visible_document_from_active_tab took the empty-doc
        // branch (which suppresses the implicit refresh).
        self.refresh_panel_ctx();
    }

    /// Refresh `panel_ctx` selection fields from the active canvas.
    ///
    /// NOTE: `panel_ctx` is shared across every window — the dock
    /// panels, Properties panel, and status bar all read these
    /// fields. When an undocked window handles a canvas event via
    /// the swap trick, "active canvas" refers to the undocked
    /// window's canvas for the duration of the event, so this
    /// function writes THAT window's selection into the shared
    /// panel_ctx. End result: main-window panels reflect the
    /// most-recently-interacted-with window's selection. This is
    /// intentional "last-touched wins" behaviour.
    pub(crate) fn update_selection_info(&mut self) {
        // AutoFocus dims every item not in the current selection, so any
        // selection change must invalidate the cached content layer to
        // reflect the new focus set.
        if self.ui_state.auto_focus {
            self.interaction_state
                .active_canvas_mut()
                .clear_content_cache();
        }
        let selected = &self.interaction_state.active_canvas_mut().selected;
        self.document_state.panel_ctx.selection_count = selected.len();
        self.document_state.panel_ctx.selection_info.clear();
        self.document_state.panel_ctx.selected_uuid = None;
        self.document_state.panel_ctx.selected_kind = None;
        self.document_state.panel_ctx.selected_drawing = None;
        self.document_state.panel_ctx.selected_child_sheet = None;

        if selected.len() != 1 {
            if !selected.is_empty() {
                self.document_state
                    .panel_ctx
                    .selection_info
                    .push(("Selected".into(), format!("{} items", selected.len())));
            }
            return;
        }

        // Borrow `engines` + `panel_ctx` as disjoint fields so the
        // compiler can split the mutation below. Going through
        // `active_engine()` would keep the whole `DocumentState`
        // borrowed for the duration of the block.
        let active_path = self.document_state.active_path.clone();
        if let Some(path) = active_path
            && let Some(engine) = self.document_state.engines.get(&path)
            && let Some(details) = engine.describe_single_selection(selected)
        {
            self.document_state.panel_ctx.selected_uuid = Some(details.selected_uuid);
            self.document_state.panel_ctx.selected_kind = Some(details.selected_kind);
            self.document_state.panel_ctx.selection_info = details.info;
            // Cache the live SchDrawing for the Properties preview
            // widget — only when the single selection is a drawing.
            if matches!(
                details.selected_kind,
                signex_types::schematic::SelectedKind::Drawing
            ) {
                use signex_types::schematic::SchDrawing;
                self.document_state.panel_ctx.selected_drawing = engine
                    .document()
                    .drawings
                    .iter()
                    .find(|d| {
                        let u = match d {
                            SchDrawing::Line { uuid, .. }
                            | SchDrawing::Rect { uuid, .. }
                            | SchDrawing::Circle { uuid, .. }
                            | SchDrawing::Arc { uuid, .. }
                            | SchDrawing::Polyline { uuid, .. } => *uuid,
                        };
                        u == details.selected_uuid
                    })
                    .cloned();
            }
            if matches!(
                details.selected_kind,
                signex_types::schematic::SelectedKind::ChildSheet
            ) {
                self.document_state.panel_ctx.selected_child_sheet = engine
                    .document()
                    .child_sheets
                    .iter()
                    .find(|cs| cs.uuid == details.selected_uuid)
                    .cloned();
            }
        }
    }

    pub(crate) fn update_canvas_theme(&mut self) {
        let colors = if self.ui_state.theme_id == ThemeId::Custom {
            self.ui_state
                .custom_theme
                .as_ref()
                .map(|custom_theme| custom_theme.canvas)
                .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
        } else {
            signex_types::theme::canvas_colors(self.ui_state.theme_id)
        };
        self.interaction_state.active_canvas_mut().set_theme_colors(
            crate::render_config::to_iced(&colors.background),
            crate::render_config::to_iced(&colors.grid),
            crate::render_config::to_iced(&colors.paper),
        );
        self.interaction_state.pcb_canvas.set_theme_colors(
            crate::render_config::to_iced(&colors.background),
            crate::render_config::to_iced(&colors.grid),
        );
        self.interaction_state.active_canvas_mut().canvas_colors = colors;
        self.interaction_state.pcb_canvas.canvas_colors = colors;
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();
    }
}
