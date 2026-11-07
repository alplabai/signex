use super::*;

mod body_style_info;
mod designators;
mod footprint_card;
#[cfg(test)]
pub(crate) mod designators_tests;
mod footprint_ctx;
mod footprint_summaries;
mod history;
mod label_class_pick;
pub(super) mod navigator_ctx;
pub(in crate::app) mod net_class_styles;
mod panel_ctx;
mod pin_focus;
mod sheet_frame;
mod sheet_tree;
#[cfg(test)]
pub(crate) mod sheet_tree_tests;
mod symbol_ctx;
mod window_panel_ctx;
pub(crate) use window_panel_ctx::WindowCtxKey;
mod window_hierarchies;
#[cfg(test)]
mod window_placement_tests;

impl Signex {
    /// Copy the UI-state grid/unit/snap fields into the panel context.
    pub(in crate::app) fn sync_panel_ctx_mirrors(&mut self) {
        self.document_state.panel_ctx.unit = self.ui_state.unit;
        self.document_state.panel_ctx.grid_visible = self.ui_state.grid_visible;
        self.document_state.panel_ctx.snap_enabled = self.ui_state.snap_enabled;
        self.document_state.panel_ctx.grid_size_mm = self.ui_state.grid_size_mm;
        self.document_state.panel_ctx.visible_grid_mm = self.ui_state.visible_grid_mm;
        self.document_state.panel_ctx.snap_hotspots = self.ui_state.snap_hotspots;
        let (visible, visible_grid_mm) = (
            self.ui_state.grid_visible,
            self.ui_state.visible_grid_mm as f64,
        );
        for canvas in self.interaction_state.all_pcb_canvases_mut() {
            canvas.set_grid(visible, visible_grid_mm);
        }
        self.sync_zoom_step();
        self.sync_fp_layer_palette();
    }

    /// Give every footprint editor the layer colour set of the active
    /// theme, and redraw an editor whose set changed.
    fn sync_fp_layer_palette(&mut self) {
        let palette = crate::library::editor::footprint::layers::LayerPalette::for_tokens(
            &self.document_state.panel_ctx.tokens,
        );
        for editor in self.document_state.footprint_editors.values_mut() {
            if editor.state.layer_palette != palette {
                editor.state.layer_palette = palette;
                editor.canvas_cache.clear();
            }
        }
    }

    /// Push the effective "Zoom speed" into the canvases that hold their
    /// own copy: every PCB canvas and the symbol / footprint editor tabs
    /// (the schematic reads it per frame through `CanvasViewPrefs`). Also
    /// called from the Preferences draft / discard paths, which do not run
    /// `finish_update`.
    pub(in crate::app) fn sync_zoom_step(&mut self) {
        let zoom_step = self.ui_state.effective_zoom_step();
        let trackpad_pan = self.ui_state.trackpad_pan;
        for canvas in self.interaction_state.all_pcb_canvases_mut() {
            canvas.zoom_step = zoom_step;
            canvas.trackpad_pan = trackpad_pan;
        }
        for editor in self.document_state.symbol_editors.values_mut() {
            editor.zoom_step = zoom_step;
            editor.trackpad_pan = trackpad_pan;
        }
        for editor in self.document_state.footprint_editors.values_mut() {
            editor.zoom_step = zoom_step;
            editor.trackpad_pan = trackpad_pan;
        }
    }

    pub(crate) fn finish_update(&mut self) -> Task<Message> {
        self.refresh_placement_reference();
        self.sync_panel_ctx_mirrors();
        let shown_browser = match self.document_state.tabs.get(self.document_state.active_tab) {
            Some(tab) => match &tab.kind {
                crate::app::TabKind::LibraryBrowser(path) => Some(path.clone()),
                _ => None,
            },
            None => None,
        };
        self.library.note_shown_browser(shown_browser.as_deref());
        self.library.sync_view_caches();
        let library_tabs = self.close_tabs_of_unmounted_libraries();
        self.sync_component_tab_titles();
        let class_styles = self.refresh_net_class_styles();
        let net_nav = self.refresh_net_nav();
        let footprint_card = self.refresh_footprint_card();
        self.sync_label_class_pick();
        self.sync_diagnostics_panel_ctx();
        self.sync_window_panel_ctx_shared();
        self.refresh_command_palette_if_stale();

        // Re-resolve the History panel's active path; on change, bump
        // the generation counter and kick off an async load. Stale
        // results check `generation == history.generation` and drop
        // themselves on mismatch.
        let history = Task::batch([
            self.refresh_history_panel(),
            self.refresh_window_histories(),
        ]);
        let dir_probes = self.warm_document_dir_probes();
        // v0.23 — Drain any queued git commits onto the iced task
        // pool so they run off the update thread. The "Saving…" pill
        // in the status bar reads from `inflight_git_commits` until
        // each Task::perform completion fires `ProjectGitCommitDone`.
        let commits = self.drain_pending_git_commits();
        let drift = self.drain_pending_drift_check();
        // Reload the Annotate preview if the documents changed under an open
        // dialog (a no-op while it is closed or already current).
        let annotate_preview = self.refresh_annotate_preview();
        let reprefix_window = self.drain_reprefix_window_close();
        let instance_note = if std::mem::take(&mut self.document_state.instance_note_due) {
            self.begin_instance_note()
        } else {
            Task::none()
        };
        Task::batch([
            history,
            dir_probes,
            commits,
            drift,
            annotate_preview,
            reprefix_window,
            instance_note,
            library_tabs,
            class_styles,
            net_nav,
            footprint_card,
        ])
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
        // Ids are monotonic and eviction only happens on push, so an
        // unchanged newest id means the ring is unchanged: skip the clone.
        let held = self
            .document_state
            .panel_ctx
            .diagnostics
            .last()
            .map(|e| e.id);
        if held.is_some() && held == crate::diagnostics::newest_entry_id() {
            return;
        }
        self.document_state.panel_ctx.diagnostics = crate::diagnostics::recent_entries();
    }

    pub(crate) fn sync_active_tab(&mut self) {
        // A tab switch ends every Project tab's typing run: what is typed
        // after coming back is its own undo step.
        for tab in self.document_state.project_tabs.values_mut() {
            tab.general_history.close_run();
        }
        // Follow the focused tab into its project: the Projects-panel
        // accent and active_project-scoped handlers (ERC / annotate /
        // save-all) should track the user's tab focus, not the most
        // recently opened project. Tabs with no `project_id` (loose
        // schematics opened without a `.snxprj`) leave the pointer
        // alone so the panel keeps showing whichever project was last
        // active. (#54 phase 2.4)
        if let Some(pid) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.project_id)
        {
            if self.document_state.active_project != Some(pid) {
                // A Re-prefix request is for one project (#1315).
                self.invalidate_reprefix();
            }
            self.document_state.active_project = Some(pid);
            self.drop_pending_profile_update_for_other_project();
        }

        self.sync_visible_document_from_active_tab();
        // The sheet now in view is the history's current one, and the net
        // highlight follows the tab (#1271, #1272).
        self.note_sheet_focused();
        // A move armed or floating on the previous tab must not drop here.
        self.cancel_move_pick();
        // A half-placed two-click shape belongs to the tab it was started in.
        self.clear_shape_anchors();
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

    /// Refresh `panel_ctx` selection fields from the active canvas. While an
    /// undocked window's action runs, both are that window's (see
    /// `run_in_window`), so its Properties panel follows its own selection.
    pub(crate) fn update_selection_info(&mut self) {
        // AutoFocus dims every item not in the current selection, so any
        // selection change must invalidate the cached content layer to
        // reflect the new focus set.
        if self.ui_state.auto_focus {
            self.interaction_state
                .active_canvas_mut()
                .clear_content_cache();
        }
        // The clicked pin stays focused through refreshes (an edit, the net
        // index landing) for as long as its part is the one selected part; any
        // other selection drops it.
        let keeps_focus = self
            .interaction_state
            .clicked_pin
            .as_ref()
            .is_some_and(|c| {
                let same_sheet = self.document_state.active_path.as_deref()
                    == Some(c.sheet.as_path())
                    && placements_agree(self.shown_placement(&c.sheet), &c.placement);
                same_sheet
                    && crate::canvas::pin_focus::is_sole_selected_symbol(
                        &self.interaction_state.active_canvas().selected,
                        c.symbol,
                    )
            });
        if !keeps_focus {
            self.interaction_state.clicked_pin = None;
        }
        let clicked_pin = self
            .interaction_state
            .clicked_pin
            .as_ref()
            .map(|c| (c.symbol, c.pin.clone()));
        self.sync_focused_pin(clicked_pin.clone());
        let selected = &self.interaction_state.active_canvas_mut().selected;
        self.document_state.panel_ctx.selection_count = selected.len();
        self.document_state.panel_ctx.selection_info.clear();
        self.document_state.panel_ctx.symbol_choices.clear();
        self.document_state.panel_ctx.footprint_choices.clear();
        self.document_state.panel_ctx.footprint_card = None;
        self.document_state.panel_ctx.selected_uuid = None;
        self.document_state.panel_ctx.selected_kind = None;
        self.document_state.panel_ctx.selected_drawing = None;
        self.document_state.panel_ctx.selected_drawing_placement = Default::default();
        self.document_state.panel_ctx.selected_child_sheet = None;
        self.document_state.panel_ctx.multi_draft = None;
        self.document_state.panel_ctx.multi_groups = match self.document_state.active_engine() {
            Some(engine) if selected.len() > 1 => {
                signex_engine::selection_groups(engine.document(), selected)
            }
            _ => Default::default(),
        };

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
            let pins = pin_focus::pin_keys(&self.document_state.panel_ctx.selection_info);
            let nets = self.pin_net_names(&path, details.selected_uuid, &pins);
            pin_focus::add_pin_rows(
                &mut self.document_state.panel_ctx.selection_info,
                details.selected_uuid,
                clicked_pin,
                nets,
            );
            // A tab opened through a placement shows, and edits, that
            // placement's designator (#1151).
            if let Some(designator) = self
                .interaction_state
                .active_canvas()
                .designators
                .get(&details.selected_uuid)
            {
                let is_ref_field =
                    details.selected_kind == signex_types::schematic::SelectedKind::SymbolRefField;
                for (key, value) in &mut self.document_state.panel_ctx.selection_info {
                    use signex_engine::info_keys;
                    if key == info_keys::REFERENCE || (is_ref_field && key == info_keys::TEXT) {
                        value.clone_from(designator);
                    }
                }
            }
            // The drawings a placed symbol's row offers (#1230).
            let choices = engine
                .document()
                .symbols
                .iter()
                .find(|s| s.uuid == details.selected_uuid)
                .map(|s| self.symbol_choices_for(s))
                .unwrap_or_default();
            self.document_state.panel_ctx.symbol_choices = choices;
            // The footprints it can be built with (#1234).
            let footprints = engine
                .document()
                .symbols
                .iter()
                .find(|s| s.uuid == details.selected_uuid)
                .map(|s| self.footprint_choices_for(s))
                .unwrap_or_default();
            self.document_state.panel_ctx.footprint_choices = footprints;
            // Library name + drift for a placed library symbol (#912).
            if let Some(lib_id) = self
                .document_state
                .panel_ctx
                .selection_info
                .iter()
                .find(|(k, _)| k == "Library ID")
                .map(|(_, v)| v.clone())
            {
                let drift = &self.library.drift;
                if let Some(name) = drift.names.get(&lib_id) {
                    self.document_state
                        .panel_ctx
                        .selection_info
                        .push(("Library".into(), name.clone()));
                }
                if drift.is_drifted(&path, &lib_id) {
                    self.document_state
                        .panel_ctx
                        .selection_info
                        .push(("Library Drift".into(), "Yes".into()));
                }
                if drift.is_missing(&path, &lib_id) {
                    self.document_state
                        .panel_ctx
                        .selection_info
                        .push(("Library Status".into(), "Symbol missing".into()));
                }
            }
            self.document_state
                .panel_ctx
                .selection_info
                .extend(body_style_info::body_style_info(
                    engine.document(),
                    details.selected_uuid,
                ));
            // The library row's documents for a placed symbol (#1231).
            if details.selected_kind == signex_types::schematic::SelectedKind::Symbol
                && let Some(symbol) = engine
                    .document()
                    .symbols
                    .iter()
                    .find(|s| s.uuid == details.selected_uuid)
            {
                // One equal to the symbol's own Datasheet field is already
                // shown as that field.
                let own = symbol.datasheet.trim();
                let rows = crate::library::row_documents::document_rows(
                    &self.library.open_libraries,
                    symbol.library_id,
                    symbol.row_id,
                )
                .into_iter()
                .filter(|(_, value)| value != own);
                self.document_state.panel_ctx.selection_info.extend(rows);
            }
            // Cache the live SchDrawing for the Properties preview
            // widget — only when the single selection is a drawing.
            if matches!(
                details.selected_kind,
                signex_types::schematic::SelectedKind::Drawing
            ) {
                self.document_state.panel_ctx.selected_drawing_placement =
                    engine.document().drawing_placement(details.selected_uuid);
                self.document_state.panel_ctx.selected_drawing = engine
                    .document()
                    .drawings
                    .iter()
                    .find(|d| d.uuid() == details.selected_uuid)
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
        self.document_state.panel_ctx.footprint_card = self.selected_footprint_card();
    }

    /// Canvas colours for a given theme id — the one derivation shared by
    /// `update_canvas_theme` (pushes them into the PCB canvas) and
    /// [`Self::canvas_view_prefs`] (hands them to the schematic `Program`
    /// each frame). `Custom` reads the loaded
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

    /// UI chrome tokens for a given theme id. `Custom` reads the loaded
    /// custom theme and falls back to Signex when none is loaded — the
    /// token twin of [`Self::canvas_colors_for`], so a panel refresh
    /// cannot reset a Custom theme to defaults (#110).
    pub(crate) fn tokens_for(&self, id: ThemeId) -> signex_types::theme::ThemeTokens {
        if id == ThemeId::Custom {
            self.ui_state
                .custom_theme
                .as_ref()
                .map(|custom_theme| custom_theme.tokens)
                .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
        } else {
            signex_types::theme::theme_tokens(id)
        }
    }

    /// Panel tokens and icon-tint theme for the effective theme: the
    /// Preferences draft while the dialog is open, the saved theme after.
    pub(crate) fn panel_theme(&self) -> (signex_types::theme::ThemeTokens, ThemeId) {
        let id = self.ui_state.effective_theme_id();
        (self.tokens_for(id), id)
    }

    /// Point the panel context at the effective theme. The one place that
    /// writes both fields, so tokens and icon tint cannot drift apart.
    pub(crate) fn sync_panel_theme(&mut self) {
        let (tokens, theme_id) = self.panel_theme();
        self.document_state.panel_ctx.tokens = tokens;
        self.document_state.panel_ctx.theme_id = theme_id;
        self.document_state.panel_ctx.canvas_colors = self.canvas_colors_for(theme_id);
    }

    /// The schematic's visible grid, for the symbol previews. Read from the
    /// draft style like `canvas_view_prefs`, so a Preferences live preview
    /// and a Discard reach the preview at once.
    pub(crate) fn sheet_grid(
        &self,
    ) -> crate::panels::components_panel::details::symbol_canvas::SheetGrid {
        crate::panels::components_panel::details::symbol_canvas::SheetGrid {
            visible: self.ui_state.grid_visible,
            pitch_mm: self.ui_state.visible_grid_mm,
            style: self.ui_state.preferences_draft_grid_style,
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
    pub(crate) fn canvas_view_prefs(
        &self,
        window: iced::window::Id,
    ) -> crate::canvas::CanvasViewPrefs<'_> {
        // The DRAFT theme, not the committed one: picking a theme in
        // Preferences previews it on the canvas immediately, which is
        // what the `PrefMsg::DraftTheme` arm used to push by hand.
        let colors = self.canvas_colors_for(self.ui_state.preferences_draft_theme);
        let placement = self
            .document_state
            .sheet_path_for_window(window, &self.ui_state)
            .and_then(|path| self.shown_placement(path));
        crate::canvas::CanvasViewPrefs {
            grid_visible: self.ui_state.grid_visible,
            theme_bg: crate::render_config::to_iced(&colors.background),
            theme_grid: crate::render_config::to_iced(&colors.grid),
            theme_paper: crate::render_config::to_iced(&colors.paper),
            canvas_colors: colors,
            graphite: crate::canvas::GraphiteCanvasInk::from_tokens(
                &self.tokens_for(self.ui_state.preferences_draft_theme),
            ),
            snap_enabled: self.ui_state.snap_enabled,
            snap_grid_mm: crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm),
            snap_hotspots: self.ui_state.snap_hotspots,
            unit: self.ui_state.unit,
            wire_tool_active: self.interaction_state.current_tool == crate::app::Tool::Wire,
            visible_grid_mm: self.ui_state.visible_grid_mm as f64,
            grid_style: self.ui_state.preferences_draft_grid_style,
            auto_focus: self.ui_state.auto_focus,
            zoom_step: self.ui_state.effective_zoom_step(),
            trackpad_pan: self.ui_state.trackpad_pan,
            move_keeps_wires: self.ui_state.move_keeps_wires,
            draw_mode: self.interaction_state.draw_mode,
            current_tool: self.interaction_state.current_tool,
            window_width: self.ui_state.size_of_window(window).0,
            wire_color_overrides: &self.ui_state.wire_color_overrides,
            wire_class_styles: self.ui_state.net_class_styles.styles_in(placement.as_ref()),
            wire_class_revision: self
                .ui_state
                .net_class_styles
                .revision_in(placement.as_ref()),
            schematic_defaults: self
                .document_state
                .sheet_path_for_window(window, &self.ui_state)
                .map(|path| self.document_state.schematic_defaults_for(path))
                .unwrap_or_default(),
            selection_filters: &self.interaction_state.selection_filters,
            placement: None,
            power_port_pending: self.interaction_state.pending_power.is_some(),
            device_px_per_logical_px: self.ui_state.device_px_per_logical_px(window),
        }
    }

    /// Repaint every canvas with the committed theme.
    pub(crate) fn update_canvas_theme(&mut self) {
        self.apply_canvas_theme(self.ui_state.theme_id);
    }

    /// Push `id`'s canvas colours onto every PCB canvas and drop the caches
    /// of every PCB and schematic canvas, docked or undocked. Serves both
    /// the committed theme and the Preferences draft preview (#1749).
    pub(crate) fn apply_canvas_theme(&mut self, id: ThemeId) {
        let colors = self.canvas_colors_for(id);
        for canvas in self.interaction_state.all_pcb_canvases_mut() {
            canvas.set_theme_colors(
                crate::render_config::to_iced(&colors.background),
                crate::render_config::to_iced(&colors.grid),
            );
            canvas.canvas_colors = colors;
            canvas.clear_content_cache();
        }
        for canvas in self.interaction_state.all_schematic_canvases_mut() {
            // The paper and grid live in the background layer, so a new
            // theme repaints them only if that cache is dropped too.
            canvas.clear_bg_cache();
            canvas.clear_content_cache();
        }
    }
}

/// Whether two placements of a sheet are the same one. An unknown placement
/// (the hierarchy has not filled in yet, or has gone) agrees with any, so a
/// pin focus survives the hierarchy arriving; two known ones must be equal.
fn placements_agree(
    a: Option<signex_types::schematic::InstancePath>,
    b: &Option<signex_types::schematic::InstancePath>,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == *b,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1350 - theme, grid and net-colour changes reach undocked schematic
    /// canvases, not just the main one.
    #[test]
    fn appearance_changes_invalidate_undocked_schematic_canvases() {
        let (mut app, _t) = Signex::new();
        let win = iced::window::Id::unique();
        app.interaction_state
            .canvases
            .insert(win, crate::canvas::CanvasSlot::new());

        app.update_canvas_theme();
        // A theme change repaints the paper and grid too, not just the content.
        let slot = &app.interaction_state.canvases[&win];
        assert_eq!(slot.content_cache_clears, 1);
        assert_eq!(slot.bg_cache_clears, 1);

        let _ = app.update(Message::Ui(UiMsg::GridToggle));
        assert_eq!(app.interaction_state.canvases[&win].bg_cache_clears, 2);

        let _ = app.update(Message::NetColor(NetColorMsg::Set {
            net: "N1".into(),
            color: None,
        }));
        assert_eq!(app.interaction_state.canvases[&win].content_cache_clears, 2);
    }

    /// #682 — bootstrap seeds the PCB canvas from the loaded grid state, so
    /// a saved grid-off (or a non-default size) holds before any update.
    #[test]
    fn bootstrap_seeds_pcb_canvas_grid_from_ui_state() {
        let (app, _t) = Signex::new();

        assert_eq!(
            app.interaction_state.pcb_canvas.grid_visible,
            app.ui_state.grid_visible
        );
        assert_eq!(
            app.interaction_state.pcb_canvas.visible_grid_mm,
            app.ui_state.visible_grid_mm as f64
        );
    }

    /// #682 — the PCB canvas used to keep its own grid fields that nothing
    /// ever wrote after construction, so the Grid Toggle and Visible Grid
    /// Size never reached it. `finish_update` now hands them over.
    #[test]
    fn pcb_canvas_follows_grid_toggle_and_visible_size() {
        let (mut app, _t) = Signex::new();
        app.ui_state.grid_visible = false;
        app.ui_state.visible_grid_mm = 5.08;

        let _ = app.finish_update();

        assert!(!app.interaction_state.pcb_canvas.grid_visible);
        assert_eq!(
            app.interaction_state.pcb_canvas.visible_grid_mm,
            5.08_f32 as f64
        );
    }

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
            let before = app.canvas_view_prefs(iced::window::Id::unique());
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
        let after = app.canvas_view_prefs(iced::window::Id::unique());
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
            app.canvas_view_prefs(iced::window::Id::unique())
                .canvas_colors,
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
            app.canvas_view_prefs(iced::window::Id::unique())
                .canvas_colors,
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
        let uuid = uuid::Uuid::now_v7();
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
            app.canvas_view_prefs(iced::window::Id::unique())
                .wire_color_overrides
                .get(&uuid),
            Some(&color),
            "the renderer must see the override without a copy step"
        );

        // Act — and clearing `ui_state` is the whole operation.
        app.ui_state.wire_color_overrides.clear();

        // Assert
        assert!(
            app.canvas_view_prefs(iced::window::Id::unique())
                .wire_color_overrides
                .is_empty(),
            "a cleared override must not survive on a canvas copy"
        );
    }
}
