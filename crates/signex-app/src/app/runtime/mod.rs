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
    #[expect(
        dead_code,
        reason = "reserved for callers outside the Components Panel that need a flat library-path slice"
    )]
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

    /// Republish the diagnostics ring buffer into the Messages panel.
    ///
    /// `finish_update` calls this, but not every dispatcher calls
    /// `finish_update` — `dispatch_preferences_message` deliberately does
    /// not, because reloading the History panel and draining git commits
    /// on every keystroke in a modal is not what that dispatcher is for.
    /// Those dispatchers call this directly instead, so a record they
    /// emit is on screen in the same frame rather than whenever some
    /// unrelated later message happens to run `finish_update`.
    pub(in crate::app) fn sync_diagnostics_panel_ctx(&mut self) {
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

    /// The active canvas colour set, derived from the saved theme.
    ///
    /// One derivation for both readers — `update_canvas_theme` (which
    /// still pushes them into the PCB canvas) and
    /// [`Self::canvas_view_prefs`] (which hands them to the schematic
    /// `Program` each frame). Two copies of this `if` was how the two
    /// could disagree.
    /// Canvas colours for a given theme id. `Custom` reads the loaded
    /// custom theme and falls back to Signex when none is loaded.
    pub(crate) fn canvas_colors_for(&self, id: ThemeId) -> signex_types::theme::CanvasColors {
        if id == ThemeId::Custom {
            self.ui_state
                .custom_theme
                .as_ref()
                .map(|custom_theme| custom_theme.canvas)
                .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
        } else {
            signex_types::theme::canvas_colors(id)
        }
    }

    /// Settings the schematic canvas renders with, read fresh from app
    /// state on every frame (#631).
    ///
    /// Every field here used to be a copy on the canvas struct, pushed by
    /// hand from whichever handler changed it. Most of those sites wrote
    /// the *active* canvas only, so an undocked window kept rendering the
    /// value it was created with — the staleness the issue predicted.
    /// Reading them here means every window draws from one source.
    ///
    /// `grid_style` comes from the draft field on purpose: that is the
    /// effective value, carrying the Preferences live preview while the
    /// dialog is open and equal to the committed one otherwise (#630).
    pub(crate) fn canvas_view_prefs(&self) -> crate::canvas::CanvasViewPrefs<'_> {
        // The DRAFT theme, not the committed one: picking a theme in
        // Preferences previews it on the canvas immediately, which is
        // what the `PrefMsg::DraftTheme` arm used to push by hand.
        let colors = self.canvas_colors_for(self.ui_state.preferences_draft_theme);
        crate::canvas::CanvasViewPrefs {
            grid_visible: self.ui_state.grid_visible,
            theme_bg: crate::render_config::to_iced(&colors.background),
            theme_grid: crate::render_config::to_iced(&colors.grid),
            theme_paper: crate::render_config::to_iced(&colors.paper),
            canvas_colors: colors,
            snap_enabled: self.ui_state.snap_enabled,
            snap_grid_mm: self.ui_state.grid_size_mm as f64,
            visible_grid_mm: self.ui_state.visible_grid_mm as f64,
            grid_style: self.ui_state.preferences_draft_grid_style,
            auto_focus: self.ui_state.auto_focus,
            draw_mode: self.interaction_state.draw_mode,
            wire_color_overrides: &self.ui_state.wire_color_overrides,
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
        self.interaction_state.pcb_canvas.set_theme_colors(
            crate::render_config::to_iced(&colors.background),
            crate::render_config::to_iced(&colors.grid),
        );
        self.interaction_state.pcb_canvas.canvas_colors = colors;
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #631 — the canvas used to own copies of these settings, written by
    /// whichever handler changed them. Most of those sites wrote
    /// `active_canvas_mut()` only, so an undocked window kept rendering
    /// the value it was created with. `canvas_view_prefs` is now the only
    /// path from `UiState` to the renderer, so a change has to show up
    /// there with nothing else called in between.
    #[test]
    fn grid_settings_reach_the_renderer_with_no_sync_step() {
        // Arrange
        let (mut app, _t) = Signex::new();
        // Copy the scalars out so the prefs borrow of `app` ends here —
        // it borrows `wire_color_overrides`, so holding it would block the
        // mutations below.
        let (visible_before, grid_visible_before, snap_before) = {
            let before = app.canvas_view_prefs();
            (
                before.visible_grid_mm,
                before.grid_visible,
                before.snap_enabled,
            )
        };
        let wanted_visible = visible_before + 1.0;

        // Act — mutate app state directly, exactly as a handler would,
        // and call nothing else.
        app.ui_state.visible_grid_mm = wanted_visible as f32;
        app.ui_state.grid_visible = !grid_visible_before;
        app.ui_state.snap_enabled = !snap_before;

        // Assert
        let after = app.canvas_view_prefs();
        assert_eq!(after.visible_grid_mm, wanted_visible);
        assert_eq!(after.grid_visible, !grid_visible_before);
        assert_eq!(after.snap_enabled, !snap_before);
    }

    /// Picking a theme in Preferences previews it on the canvas before
    /// Save. That preview used to be a hand-written push of the computed
    /// colours onto the canvas from the `DraftTheme` arm; it is now the
    /// draft field itself feeding `canvas_view_prefs`. If this reverted to
    /// reading the committed `theme_id`, the picker would look dead.
    #[test]
    fn the_canvas_previews_the_draft_theme_not_the_committed_one() {
        // Arrange
        let (mut app, _t) = Signex::new();
        let committed = app.ui_state.theme_id;
        let other = if committed == ThemeId::Nord {
            ThemeId::SolarizedLight
        } else {
            ThemeId::Nord
        };

        // Act
        app.ui_state.preferences_draft_theme = other;

        // Assert
        assert_eq!(
            app.canvas_view_prefs().canvas_colors,
            signex_types::theme::canvas_colors(other),
            "the canvas must render the previewed theme"
        );
        assert_eq!(
            app.ui_state.theme_id, committed,
            "a preview must not commit the theme"
        );
    }

    /// #631 — `preferences_draft_theme` was hardcoded to `Signex` at boot
    /// rather than seeded from the saved preference. That was invisible
    /// while nothing read it before the Preferences dialog first opened
    /// (`seed_preferences_drafts_from_live` repaired it there). The canvas
    /// reads it now, so a user whose saved theme is not Signex would have
    /// opened to the wrong canvas colours.
    #[test]
    fn boot_seeds_the_draft_theme_from_the_saved_theme() {
        // Arrange / Act
        let (app, _t) = Signex::new();

        // Assert
        assert_eq!(
            app.ui_state.preferences_draft_theme, app.ui_state.theme_id,
            "the previewed theme must start equal to the saved one"
        );
        assert_eq!(
            app.canvas_view_prefs().canvas_colors,
            app.canvas_colors_for(app.ui_state.theme_id),
            "so the first frame renders the saved theme's canvas colours"
        );
    }

    /// The wire-colour overrides are borrowed, not cloned onto the canvas.
    /// The old copy was cleared in parallel with `ui_state`'s at each
    /// mutation site — one missed site and the canvas kept painting a
    /// colour the user had cleared.
    #[test]
    fn wire_colour_overrides_are_read_from_ui_state() {
        // Arrange
        let (mut app, _t) = Signex::new();
        let uuid = uuid::Uuid::new_v4();
        let color = signex_types::theme::Color {
            r: 1,
            g: 2,
            b: 3,
            a: 255,
        };

        // Act
        app.ui_state.wire_color_overrides.insert(uuid, color);

        // Assert
        assert_eq!(
            app.canvas_view_prefs().wire_color_overrides.get(&uuid),
            Some(&color),
            "the renderer must see the override without a copy step"
        );

        // Act — and clearing `ui_state` is the whole operation.
        app.ui_state.wire_color_overrides.clear();

        // Assert
        assert!(
            app.canvas_view_prefs().wire_color_overrides.is_empty(),
            "a cleared override must not survive on a canvas copy"
        );
    }
}
