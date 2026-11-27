use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_ui_message(&mut self, message: UiMsg) -> Task<Message> {
        match message {
            UiMsg::Noop => Task::none(),
            UiMsg::PrefWriteFinished => {
                self.sync_diagnostics_panel_ctx();
                Task::none()
            }
            UiMsg::KeymapModPromptAnswered { use_command } => {
                self.handle_keymap_mod_prompt_answered(use_command)
            }
            UiMsg::KeymapModMigrationWritten(result) => {
                self.handle_keymap_mod_migration_written(result)
            }
            UiMsg::KeymapModStaleResaved(result) => self.handle_keymap_mod_stale_resaved(result),
            UiMsg::MenuInWindow { window, msg } => {
                let Some(id) = msg.window_command() else {
                    return self.update(Message::Menu(msg));
                };
                self.run_menu_command(&id, Some(window))
            }
            UiMsg::HistoryLoaded {
                window: None,
                generation,
                path,
                result,
            } => self.apply_history_loaded(generation, path, result),
            UiMsg::HistoryLoaded {
                window: Some(window),
                generation,
                path,
                result,
            } => {
                if !self.is_document_window(window) {
                    crate::diagnostics::log_info(format!(
                        "History for {} was dropped: the window that asked for it has closed.",
                        path.display()
                    ));
                    return Task::none();
                }
                self.run_in_window(window, false, |app| {
                    app.apply_history_loaded(generation, path, result)
                })
            }
            UiMsg::HistoryRepo {
                window: None,
                event,
            } => self.apply_history_repo_event(event),
            UiMsg::HistoryRepo {
                window: Some(window),
                event,
            } => {
                if !self.is_document_window(window) {
                    return Task::none();
                }
                self.run_in_window(window, false, |app| app.apply_history_repo_event(event))
            }
            UiMsg::ThemeChanged(id) => {
                self.ui_state.theme_id = id;
                self.ui_state.preferences_draft_theme = id;
                self.sync_theme();
                let save = crate::fonts::background_pref_write("theme", move || {
                    crate::fonts::write_theme_pref(id)
                });
                Task::batch([save, self.finish_update()])
            }
            UiMsg::LanguageChanged(pref) => {
                let save = self.set_language(pref);
                Task::batch([save, self.finish_update()])
            }
            UiMsg::UnitCycled | UiMsg::StatusBar(StatusBarRequest::CycleUnit) => {
                let save = self.handle_unit_cycle_request();
                Task::batch([save, self.finish_update()])
            }
            UiMsg::GridToggle | UiMsg::StatusBar(StatusBarRequest::ToggleGrid) => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                for canvas in self.interaction_state.all_schematic_canvases_mut() {
                    canvas.clear_bg_cache();
                }
                for canvas in self.interaction_state.all_pcb_canvases_mut() {
                    canvas.clear_bg_cache();
                }
                let visible = self.ui_state.grid_visible;
                let save = crate::fonts::background_pref_write("grid_visible", move || {
                    crate::fonts::write_grid_visible_pref(visible)
                });
                Task::batch([save, self.finish_update()])
            }
            UiMsg::DragStart(target) => {
                self.handle_layout_drag_started(target);
                self.finish_update()
            }
            UiMsg::DragMovedIn { window, x, y } => {
                self.interaction_state.pointer_window = Some(window);
                self.dispatch_update(Message::Ui(UiMsg::DragMove(x, y)))
            }
            UiMsg::DragMove(x, y) => {
                self.handle_layout_drag_moved(x, y);
                // Altium parity: cursor leaving the main window during a
                // modal, floating-panel, or tab drag hands the content
                // off to the OS by spawning a detached window.
                let modal_detach = self.check_modal_auto_detach(x, y);
                let panel_detach = self.check_floating_panel_auto_detach(x, y);
                let tab_detach = self.check_tab_auto_detach(x, y);
                let arm = self.arm_components_row_drag();
                let finish = Task::batch([arm, self.finish_update()]);
                if let Some(modal) = modal_detach {
                    Task::batch([
                        finish,
                        Task::done(Message::Window(WindowMsg::DetachModal(modal))),
                    ])
                } else if let Some(idx) = panel_detach {
                    Task::batch([
                        finish,
                        Task::done(Message::Window(WindowMsg::DetachFloatingPanel(idx))),
                    ])
                } else if let Some(idx) = tab_detach {
                    Task::batch([
                        finish,
                        Task::done(Message::Window(WindowMsg::UndockTab(idx))),
                    ])
                } else {
                    finish
                }
            }
            UiMsg::WindowResized(w, h) => {
                let before = self.canvas_bounds();
                self.ui_state.window_size = (w, h);
                self.follow_main_canvas_resize(before);
                self.finish_update()
            }
            UiMsg::DragEnd(released_in) => {
                let dropped = self.finish_components_row_drag(released_in);
                self.handle_layout_drag_finished();
                self.interaction_state.pointer_window = None;
                // A dock drag changed a preferred width the layout saves.
                let persist = self.persist_dock_layout_soon();
                Task::batch([dropped, persist, self.finish_update()])
            }
            UiMsg::GridCycle => {
                let cycled = self.cycle_grid_size(true);
                Task::batch([cycled, self.finish_update()])
            }
            UiMsg::GridCycleBackward => {
                let cycled = self.cycle_grid_size(false);
                Task::batch([cycled, self.finish_update()])
            }
            UiMsg::FastGridPicked(index) => {
                let picked = self.pick_fast_grid(index);
                Task::batch([picked, self.finish_update()])
            }
            UiMsg::FastGridCycled => {
                let cycled = self.cycle_fast_grid();
                Task::batch([cycled, self.finish_update()])
            }
            UiMsg::GridPickerOpen => {
                // v0.18.10 — only mount the picker when the active
                // tab is a Footprint editor; the schematic / PCB
                // grid systems aren't wired through this picker yet.
                let footprint = self.active_footprint_editor_path();
                tracing::info!(
                    target: "signex::ui",
                    footprint_active = footprint.is_some(),
                    last_mouse_pos = ?self.interaction_state.last_mouse_pos,
                    "GridPickerOpen received",
                );
                if let Some(target) = footprint {
                    let (x, y) = self.interaction_state.last_mouse_pos;
                    let owner = self.interaction_state.acting_window;
                    self.interaction_state.grid_picker = Some(crate::app::GridPickerState {
                        x,
                        y,
                        target,
                        owner,
                        hovered_step_mm: None,
                        opens_upward: false,
                    });
                }
                self.finish_update()
            }
            UiMsg::GridPickerClose => {
                self.interaction_state.grid_picker = None;
                self.finish_update()
            }
            // Hover only changes what the preview draws: no refresh owed.
            UiMsg::GridPickerStepHovered(step_mm) => {
                if let Some(picker) = self.interaction_state.grid_picker.as_mut()
                    && step_mm > 0.0
                    && step_mm.is_finite()
                {
                    picker.hovered_step_mm = Some(step_mm);
                }
                Task::none()
            }
            // A leave of a row no longer hovered (the next row's enter came
            // first) keeps the newer hover.
            UiMsg::GridPickerStepLeft(step_mm) => {
                if let Some(picker) = self.interaction_state.grid_picker.as_mut()
                    && picker.hovered_step_mm == Some(step_mm)
                {
                    picker.hovered_step_mm = None;
                }
                Task::none()
            }
            UiMsg::GridPickerSelect(step_mm) => {
                let picker = self.interaction_state.grid_picker.take();
                let target = picker
                    .map(|picker| picker.target)
                    .or_else(|| self.footprint_dialog_target());
                let editor = match target.as_deref() {
                    Some(target) => self.footprint_editor_at_mut(target),
                    None => None,
                };
                if let Some(editor) = editor
                    && step_mm > 0.0
                    && step_mm.is_finite()
                {
                    editor.state.snap_options.grid_step_mm = step_mm;
                    // v0.18.21 — mirror onto active grid row.
                    let idx = editor.state.active_grid_idx;
                    if let Some(row) = editor.state.grids.get_mut(idx) {
                        row.step_mm = step_mm;
                    }
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                self.finish_update()
            }
            UiMsg::StatusBar(StatusBarRequest::ToggleSnap) => {
                self.ui_state.snap_enabled = !self.ui_state.snap_enabled;
                let enabled = self.ui_state.snap_enabled;
                let save = crate::fonts::background_pref_write("snap_enabled", move || {
                    crate::fonts::write_snap_enabled_pref(enabled)
                });
                // v0.18.25.1 — mirror the global toggle into every open
                // footprint editor so the .snxfpt snap chain (guides /
                // grid / point-hit / angle) short-circuits in lockstep
                // with the schematic + PCB canvases.
                let disabled = !self.ui_state.snap_enabled;
                for editor in self.document_state.footprint_editors.values_mut() {
                    editor.state.global_snap_disabled = disabled;
                    editor.canvas_cache.clear();
                }
                Task::batch([save, self.finish_update()])
            }
            UiMsg::StatusBar(StatusBarRequest::TogglePanelList) => {
                self.dispatch_overlay_message(OverlayMsg::TogglePanelList)
            }
            UiMsg::StatusBar(StatusBarRequest::OpenPropertiesForSelection) => {
                self.handle_menu_message(MenuMessage::OpenPropertiesPanel)
            }
            // The board counters and their popovers (#2460); a bare click is
            // the main window's.
            UiMsg::StatusBar(StatusBarRequest::BoardCounterClicked { window, counter }) => {
                match window.or(self.ui_state.main_window_id) {
                    Some(window) => self.board_counter_clicked(window, counter),
                    None => Task::none(),
                }
            }
            UiMsg::StatusBar(StatusBarRequest::BoardPopover(event)) => {
                self.board_popover_event(event)
            }
            UiMsg::StatusBar(StatusBarRequest::BoardClockTicked) => {
                self.ui_state.board_status.now = Some(std::time::SystemTime::now());
                Task::none()
            }
            // `status_bar_message` routes these to the window's board; a bare
            // one acts on the main window's.
            UiMsg::StatusBar(StatusBarRequest::ViewAngleClicked) => {
                self.dispatch_pcb_message(PcbMsg::ViewRotation(ViewRotationMsg::AngleEntryToggled))
            }
            UiMsg::StatusBar(StatusBarRequest::ViewAngleEdited(text)) => self.dispatch_pcb_message(
                PcbMsg::ViewRotation(ViewRotationMsg::AngleEntryEdited(text)),
            ),
            UiMsg::StatusBar(StatusBarRequest::ViewAngleSubmitted) => self
                .dispatch_pcb_message(PcbMsg::ViewRotation(ViewRotationMsg::AngleEntrySubmitted)),
            UiMsg::StatusBar(StatusBarRequest::CancelBoardImport) => {
                self.handle_altium_import_cancel();
                Task::none()
            }
            UiMsg::StatusBar(StatusBarRequest::LibraryMissingChipClicked) => {
                self.reopen_skipped_library();
                Task::none()
            }
            // `status_bar_message` names the window; a bare one is the main
            // window's.
            UiMsg::StatusBar(StatusBarRequest::GridReadoutRightClicked) => {
                match self.ui_state.main_window_id {
                    Some(main) => {
                        self.dispatch_overlay_message(OverlayMsg::GridSnapPopoverToggled(main))
                    }
                    None => Task::none(),
                }
            }
            // The caret opens the step list as the open_grid_picker key
            // does in that window (an undocked one runs it on its
            // footprint), then sets it above the bar.
            UiMsg::StatusBar(StatusBarRequest::GridCaretClicked { window }) => {
                let open = Message::Ui(UiMsg::GridPickerOpen);
                let opened = match window {
                    Some(window) if self.is_document_window(window) => {
                        self.run_command_in_window(window, open)
                    }
                    _ => self.dispatch_update(open),
                };
                let (x, y) = self.grid_caret_anchor(window);
                if let Some(picker) = self.interaction_state.grid_picker.as_mut() {
                    (picker.x, picker.y, picker.opens_upward) = (x, y, true);
                }
                opened
            }
            // On a board Shift+E is `cycle_board_snapping` (#2879).
            UiMsg::ToggleSnapHotspots => {
                self.ui_state.snap_hotspots = !self.ui_state.snap_hotspots;
                self.document_view.panel_ctx.snap_hotspots = self.ui_state.snap_hotspots;
                self.finish_update()
            }
            UiMsg::KeymapStroke { window, stroke } => self.resolve_keymap_stroke(window, stroke),
            UiMsg::KeymapChordTimedOut { generation } => self.resolve_chord_timeout(generation),
            UiMsg::KeymapChordCancelled => {
                self.cancel_pending_chord();
                Task::none()
            }
            UiMsg::InstalledFontsScanned => {
                self.handle_installed_fonts_scanned();
                self.finish_update()
            }
            UiMsg::NetClassStylesResolved(result) => {
                self.apply_net_class_styles(&result);
                Task::none()
            }
            UiMsg::NetNav(msg) => self.dispatch_net_nav_message(msg),
            UiMsg::ZoomPresetsToggled => {
                self.interaction_state.zoom_presets_open =
                    !self.interaction_state.zoom_presets_open;
                Task::none()
            }
            UiMsg::ZoomPresetsDismissed => {
                self.interaction_state.zoom_presets_open = false;
                Task::none()
            }
            UiMsg::ZoomPresetPicked(percent) => {
                self.interaction_state.zoom_presets_open = false;
                if self.has_active_schematic() {
                    let canvas = self.interaction_state.active_canvas_mut();
                    if canvas.zoom_to_percent_about_centre(percent) {
                        canvas.clear_bg_cache();
                        canvas.clear_content_cache();
                    }
                }
                Task::none()
            }
            UiMsg::ZoomPresetCommandPicked(command) => {
                self.interaction_state.zoom_presets_open = false;
                self.run_menu_command(&command, None)
            }
            UiMsg::HomeViewRequested => self.go_home_view(),
        }
    }

    /// Custom Selection Filter modal handler (namespaced family,
    /// ADR-0001 D3). Drives the footprint editor's selection-filter
    /// customization modal.
    pub(crate) fn dispatch_selection_filter_message(
        &mut self,
        msg: SelectionFilterMsg,
    ) -> Task<Message> {
        match msg {
            SelectionFilterMsg::OpenCustom => {
                let filter = self
                    .active_footprint_editor()
                    .map(|editor| editor.state.selection_filter);
                if let (Some(filter), Some(target)) = (filter, self.footprint_dialog_target()) {
                    self.ui_state.selection_filter_custom = Some(Box::new(
                        crate::app::SelectionFilterCustomState::from_filter(filter, target),
                    ));
                }
                self.finish_update()
            }
            SelectionFilterMsg::CloseCustom => {
                self.ui_state.selection_filter_custom = None;
                self.finish_update()
            }
            SelectionFilterMsg::ToggleCustomKind(kind) => {
                if let Some(state) = self.ui_state.selection_filter_custom.as_mut() {
                    state.toggle(kind);
                }
                self.finish_update()
            }
            SelectionFilterMsg::CustomKindsAllSet(on) => {
                if let Some(state) = self.ui_state.selection_filter_custom.as_mut() {
                    state.set_all_shown(on);
                }
                self.finish_update()
            }
            SelectionFilterMsg::CustomPresetNameEdited(name) => {
                if let Some(state) = self.ui_state.selection_filter_custom.as_mut() {
                    state.preset_name = name;
                }
                self.finish_update()
            }
            SelectionFilterMsg::ApplyCustom => {
                let save = self.apply_custom_selection_filter();
                self.refresh_panel_ctx();
                Task::batch([save, self.finish_update()])
            }
        }
    }

    /// Apply: the draft becomes the filter of the document it was opened on
    /// (closed by now: nothing applies, and Messages says so) and is
    /// saved as a preset under its typed name, or "Filter N" when none is
    /// typed. The filter always applies; no preset is added when one already
    /// holds the same kinds, or at the preset limit (a typed name dropped
    /// there is said in Messages). The `prefs.json` write rides the ordered
    /// background writer.
    fn apply_custom_selection_filter(&mut self) -> Task<Message> {
        use crate::library::editor::footprint::filter_presets::{capture_preset, existing_twin};
        use signex_i18n::{ids, t};
        let Some(draft) = self.ui_state.selection_filter_custom.take() else {
            return Task::none();
        };
        let presets = self.interaction_state.footprint_filter_presets.clone();
        let typed = !draft.preset_name.trim().is_empty();
        let name = draft.preset_name_or_numbered(presets.len());
        let Some(editor) = self.footprint_editor_at_mut(&draft.target) else {
            crate::diagnostics::log_warning(format!(
                "Selection filter not applied: {} is no longer open for editing.",
                draft.target.display()
            ));
            return Task::none();
        };
        editor.state.selection_filter = draft.to_filter();
        editor.canvas_cache.clear();
        let preset = capture_preset(&editor.state, name);
        let i18n = &self.ui_state.i18n;
        if let Some(twin) = existing_twin(&presets, &preset, typed) {
            crate::diagnostics::log_info(
                t!(i18n, ids::DIALOG_SELECTION_FILTER_PRESET_EXISTS, "name" => twin.name.as_str())
                    .into_string(),
            );
            return Task::none();
        }
        let limit = crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT;
        if presets.len() >= limit {
            if typed {
                crate::diagnostics::log_warning(
                    t!(i18n, ids::DIALOG_SELECTION_FILTER_PRESET_FULL,
                        "name" => preset.name.as_str(), "limit" => limit)
                    .into_string(),
                );
            }
            return Task::none();
        }
        let mut next = presets;
        next.push(preset);
        self.interaction_state.footprint_filter_presets = next.clone();
        crate::fonts::background_pref_write("footprint_filter_presets", move || {
            crate::fonts::write_footprint_filter_presets(&next)
        })
    }

    /// Open Grid Properties on `target`, seeded from its current step. No-op
    /// for anything but a footprint (the modal would have nothing to drive).
    fn open_grid_properties_on(&mut self, target: Option<std::path::PathBuf>) {
        let editor_snap = target
            .as_deref()
            .and_then(|target| self.footprint_editor_at(target))
            .map(|e| e.state.snap_options);
        tracing::info!(
            target: "signex::ui",
            has_snap = editor_snap.is_some(),
            "GridPropertiesOpen received",
        );
        if let (Some(opts), Some(target)) = (editor_snap, target) {
            let step = opts.grid_step_mm;
            self.ui_state.grid_properties = Some(Box::new(crate::app::GridPropertiesState {
                step_mm: format!("{step:.4}"),
                fine_display: opts.fine_grid_display,
                coarse_display: opts.coarse_grid_display,
                multiplier: opts.coarse_multiplier,
                target,
                ..crate::app::GridPropertiesState::default()
            }));
        }
    }

    /// Grid Properties dialog handler (namespaced family, ADR-0001 D3).
    pub(crate) fn dispatch_grid_properties_message(
        &mut self,
        msg: GridPropertiesMsg,
    ) -> Task<Message> {
        match msg {
            // The command: the footprint in view of the window it ran in.
            // An open picker closes behind it and lends it nothing.
            GridPropertiesMsg::Open => {
                self.interaction_state.grid_picker = None;
                let target = self.footprint_dialog_target();
                self.open_grid_properties_on(target);
                self.finish_update()
            }
            // The picker's own row: the footprint the picker was for.
            GridPropertiesMsg::OpenFromPicker => {
                let picked = self.interaction_state.grid_picker.take();
                self.open_grid_properties_on(picked.map(|picker| picker.target));
                self.finish_update()
            }
            GridPropertiesMsg::SetFineDisplay(d) => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.fine_display = d;
                }
                self.finish_update()
            }
            GridPropertiesMsg::SetCoarseDisplay(d) => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.coarse_display = d;
                }
                self.finish_update()
            }
            GridPropertiesMsg::SetMultiplier(m) => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.drop_open_label();
                    state.multiplier = m.max(1);
                }
                self.finish_update()
            }
            GridPropertiesMsg::Close => {
                // Esc with a preview label open closes the label only.
                if let Some(state) = self.ui_state.grid_properties.as_mut()
                    && state.preview_labels.editing_index().is_some()
                {
                    state.drop_open_label();
                    return self.finish_update();
                }
                self.ui_state.grid_properties = None;
                self.finish_update()
            }
            GridPropertiesMsg::CancelPressed => {
                self.ui_state.grid_properties = None;
                self.finish_update()
            }
            GridPropertiesMsg::PreviewExpandToggled => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.preview_expanded = !state.preview_expanded;
                }
                self.finish_update()
            }
            GridPropertiesMsg::PreviewLabelEdited(event) => {
                let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
                let focus = match self.ui_state.grid_properties.as_mut() {
                    Some(state) => {
                        if state.label_edited(event, &i18n) {
                            state.preview_labels.focus_input()
                        } else {
                            Task::none()
                        }
                    }
                    None => Task::none(),
                };
                Task::batch([focus, self.finish_update()])
            }
            GridPropertiesMsg::SetStep(value) => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.drop_open_label();
                    state.step_mm = value;
                }
                self.finish_update()
            }
            GridPropertiesMsg::Apply => {
                // A preview label still open commits its draft first, as
                // Enter (or the blur of a click on OK) would. Text that is
                // not a length refuses OK: the dialog stays open with the
                // label outlined red until it is fixed or Esc closes it.
                let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
                if let Some(state) = self.ui_state.grid_properties.as_mut()
                    && state.preview_labels.editing_index().is_some()
                {
                    if state.open_label_refused(&i18n) {
                        return self.finish_update();
                    }
                    let _ = state.label_edited(
                        signex_widgets::dimension_label::DimensionLabelMessage::DraftSubmitted,
                        &i18n,
                    );
                }
                // `SnapOptions` stores one grid step for both axes, so the
                // dialog has a single Step field.
                let draft = self.ui_state.grid_properties.clone();
                // A step that is not a finite number above zero is refused
                // with its reason in the Messages panel; the previous
                // value is kept and the dialog stays open so the user can
                // correct it (#972).
                let step = match draft
                    .as_ref()
                    .map(|s| crate::numeric_field::validated_grid_step(&s.step_mm))
                {
                    Some(Ok(step)) => Some(step),
                    Some(Err(reason)) => {
                        crate::diagnostics::log_warning(format!(
                            "Grid step refused, previous value kept: {reason}"
                        ));
                        return self.finish_update();
                    }
                    None => None,
                };
                let editor = match &draft {
                    Some(d) => self.footprint_editor_at_mut(&d.target),
                    None => None,
                };
                if let (Some(d), Some(editor)) = (&draft, editor) {
                    let opts = &mut editor.state.snap_options;
                    if let Some(step) = step {
                        opts.grid_step_mm = step;
                    }
                    opts.fine_grid_display = d.fine_display;
                    opts.coarse_grid_display = d.coarse_display;
                    opts.coarse_multiplier = d.multiplier.max(1);
                    // v0.18.21 — mirror onto the active grid row so
                    // multi-grid CRUD stays consistent. The Manager
                    // displays per-row values from `grids[idx]`, so a
                    // commit through the legacy modal must update the
                    // matching row too.
                    let active_idx = editor.state.active_grid_idx;
                    if let Some(row) = editor.state.grids.get_mut(active_idx) {
                        row.step_mm = opts.grid_step_mm;
                        row.fine_display = opts.fine_grid_display;
                        row.coarse_display = opts.coarse_grid_display;
                        row.coarse_multiplier = opts.coarse_multiplier;
                    }
                    editor.canvas_cache.clear();
                } else if let Some(d) = &draft {
                    crate::diagnostics::log_warning(format!(
                        "Grid not applied: {} is no longer open for editing.",
                        d.target.display()
                    ));
                }
                self.ui_state.grid_properties = None;
                self.refresh_panel_ctx();
                self.finish_update()
            }
        }
    }

    /// Handle a `CanvasEvent` that originated in a non-main window.
    ///
    /// The canvas event handlers assume `self.interaction_state.canvas`
    /// is the live target. To avoid rewriting hundreds of call sites,
    /// we temporarily swap the per-window canvas into the main slot
    /// (and point `document_state.active_path` at the window's tab
    /// path so engine lookups resolve correctly), run the handler, and
    /// swap back. Writes to other sub-fields of `interaction_state` /
    /// `document_state` still occur — the user experience is that the
    /// non-main window behaves like "the active window" for the
    /// duration of its event.
    ///
    /// **Invariant:** any `Task<Message>` returned by the handler runs
    /// AFTER the swap unwinds. If a handler ever chains
    /// `Task::done(Message::CanvasEvent(..))` expecting to land back
    /// in the window that produced the original event, it must use
    /// `Message::CanvasEventInWindow { window_id, .. }` instead — the
    /// plain `CanvasEvent` form always targets the main window.
    ///
    /// **Panic safety:** the swap is guarded with `catch_unwind` +
    /// `resume_unwind` so a panicking handler still restores both the
    /// canvas slot and `active_path` before the panic propagates.
    pub(super) fn handle_canvas_event_in_window(
        &mut self,
        window_id: iced::window::Id,
        event: crate::canvas::CanvasEvent,
    ) -> iced::Task<Message> {
        self.run_in_window(window_id, false, |app| {
            app.handle_canvas_interaction_event(event)
        })
    }

    /// A click on the status bar of `window`. A board counter or the layer
    /// pill opens its popover over that window, for the board it shows
    /// (#2272, #2460). On a symbol tab
    /// (`symbol`, from [`Self::status_symbol_path`]) the Grid and unit
    /// buttons toggle that symbol's library settings, the ones the bar shows.
    pub fn status_bar_message(
        window: iced::window::Id,
        symbol: Option<&std::path::Path>,
        request: StatusBarRequest,
    ) -> Message {
        use crate::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};
        let symbol_edit = |path: &std::path::Path, msg| {
            Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: path.to_path_buf(),
                msg: PrimitiveEdit::Symbol(msg),
            })
        };
        // The angle readout turns the board that window shows (#2316).
        let view_angle =
            |window, event| Message::Pcb(PcbMsg::ViewRotationInWindow { window, event });
        match (request, symbol) {
            (StatusBarRequest::BoardCounterClicked { counter, .. }, _) => {
                Message::Ui(UiMsg::StatusBar(StatusBarRequest::BoardCounterClicked {
                    window: Some(window),
                    counter,
                }))
            }
            (StatusBarRequest::ViewAngleClicked, _) => {
                view_angle(window, ViewRotationMsg::AngleEntryToggled)
            }
            (StatusBarRequest::ViewAngleEdited(text), _) => {
                view_angle(window, ViewRotationMsg::AngleEntryEdited(text))
            }
            (StatusBarRequest::ViewAngleSubmitted, _) => {
                view_angle(window, ViewRotationMsg::AngleEntrySubmitted)
            }
            // The Grid & Snap popover opens over the window that asked.
            (StatusBarRequest::GridReadoutRightClicked, _) => {
                Message::Overlay(OverlayMsg::GridSnapPopoverToggled(window))
            }
            (StatusBarRequest::GridCaretClicked { .. }, _) => {
                Message::Ui(UiMsg::StatusBar(StatusBarRequest::GridCaretClicked {
                    window: Some(window),
                }))
            }
            (StatusBarRequest::ToggleGrid, Some(path)) => {
                symbol_edit(path, SymbolEditorMsg::ToggleGrid)
            }
            (StatusBarRequest::CycleUnit, Some(path)) => {
                symbol_edit(path, SymbolEditorMsg::CycleUnit)
            }
            (request, _) => Message::Ui(UiMsg::StatusBar(request)),
        }
    }

    /// Run `handler` with `window_id`'s canvas and document swapped into
    /// the main slots, as [`Self::handle_canvas_event_in_window`] describes.
    ///
    /// A PCB window always points `active_tab` at its tab. `retarget_tab`
    /// does the same for a schematic window, for a handler that picks its
    /// target by the active tab's kind (undo, #1546); a canvas event leaves
    /// it alone, so a sheet it opens stays the main window's active tab.
    pub(crate) fn run_in_window(
        &mut self,
        window_id: iced::window::Id,
        retarget_tab: bool,
        handler: impl FnOnce(&mut Self) -> iced::Task<Message>,
    ) -> iced::Task<Message> {
        use crate::app::state::WindowKind;
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

        // Main window → run the handler directly on the legacy canvas.
        if self.ui_state.main_window_id == Some(window_id) {
            return handler(self);
        }

        // Unknown window (closed mid-queue, or never had a canvas) —
        // drop the event. Previously fell through to the main canvas,
        // which could apply a stale undocked click to the main window
        // during the race between `SecondaryWindowClosed` and a queued
        // CanvasEvent.
        if !self.interaction_state.canvases.contains_key(&window_id) {
            return iced::Task::none();
        }

        // Resolve the target tab path. Non-tab windows (detached
        // modals, detached panels) can't host a canvas today, so an
        // event from one is nonsensical — drop it.
        let target_path = match self.ui_state.windows.get(&window_id) {
            Some(WindowKind::UndockedTab { path, .. }) => path.clone(),
            _ => {
                debug_assert!(
                    false,
                    "CanvasEventInWindow for a non-UndockedTab window: {window_id:?}"
                );
                return iced::Task::none();
            }
        };

        // A PCB window whose tab has just been closed: a queued event
        // would otherwise fall through to the main window's active board.
        let pcb_tab = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.path == target_path);
        if pcb_tab.is_none() && self.interaction_state.pcb_canvases.contains_key(&window_id) {
            return iced::Task::none();
        }

        // Swap the per-window canvas into the main slot and retarget
        // `active_path` so the handler's engine + canvas accesses hit
        // the window's tab.
        let mut swapped_canvas = self
            .interaction_state
            .canvases
            .remove(&window_id)
            .expect("canvases entry checked above");
        std::mem::swap(&mut self.interaction_state.canvas, &mut swapped_canvas);
        // A PCB window swaps its PCB canvas in the same way, and retargets
        // `active_tab` too: the PCB handlers resolve the board through the
        // active tab, not `active_path` (#494). The main window's tab is
        // remembered by path and kind (a project's Project and Rules tabs
        // share the `.snxprj` path), so it is found again even if the
        // handler closed or reordered tabs.
        let saved_active_tab = self.document_state.active_tab;
        let saved_active_tab_key = self
            .document_state
            .tabs
            .get(saved_active_tab)
            .map(|tab| (tab.path.clone(), std::mem::discriminant(&tab.kind)));
        let mut swapped_pcb =
            pcb_tab.and_then(|_| self.interaction_state.pcb_canvases.remove(&window_id));
        if let Some(pcb) = swapped_pcb.as_mut() {
            std::mem::swap(&mut self.interaction_state.pcb_canvas, pcb);
        }
        // Its board selection goes with it (#2240).
        let mut swapped_pcb_selection = swapped_pcb.as_ref().map(|_| {
            self.interaction_state
                .pcb_selections
                .remove(&window_id)
                .unwrap_or_default()
        });
        if let Some(selection) = swapped_pcb_selection.as_mut() {
            std::mem::swap(&mut self.interaction_state.pcb_selection, selection);
        }
        let board_revision_before = self.document_state.pcb_revisions.get(&target_path).copied();
        let retargeted_tab = pcb_tab.filter(|_| retarget_tab || swapped_pcb.is_some());
        if let Some(index) = retargeted_tab {
            self.document_state.active_tab = index;
        }
        // The window's panel context goes into the main slot with its canvas,
        // so the panels and the handlers see the sheet the window shows.
        let fresh_ctx = !self.document_view.window_panel_ctx.contains_key(&window_id);
        let mut swapped_ctx = self
            .document_view
            .window_panel_ctx
            .remove(&window_id)
            .unwrap_or_else(|| self.new_window_panel_ctx(window_id));
        std::mem::swap(&mut self.document_view.panel_ctx, &mut swapped_ctx);
        // So does its History panel state (generation, pending restore).
        let mut swapped_history = self
            .document_view
            .window_history
            .remove(&window_id)
            .unwrap_or_default();
        std::mem::swap(&mut self.document_view.history, &mut swapped_history);
        let saved_active_path = self.document_state.active_path.take();
        let revision_before = self
            .document_state
            .engines
            .get(&target_path)
            .map(signex_engine::Engine::revision);
        self.document_state.active_path = Some(target_path.clone());

        // Run the handler. `AssertUnwindSafe` is needed because
        // `&mut self` isn't `UnwindSafe` by default — we're accepting
        // that a panicking handler may leave `self` in a partially
        // mutated state, which is fine as long as the swap itself
        // unwinds deterministically below.
        let main_work = self.main_canvas_work();
        let saved_acting = self.interaction_state.acting_window.replace(window_id);
        let task_result = catch_unwind(AssertUnwindSafe(|| {
            if fresh_ctx {
                self.init_window_panel_ctx();
            }
            handler(self)
        }));
        self.interaction_state.acting_window = saved_acting;
        std::mem::swap(&mut self.document_view.panel_ctx, &mut swapped_ctx);
        std::mem::swap(&mut self.document_view.history, &mut swapped_history);
        // The handler may have closed the window (Close Document): its
        // context and canvas are then dropped, not put back; its typed note
        // boxes are still owed to their sheets (#2649).
        let window_alive = self.ui_state.windows.contains_key(&window_id);
        if window_alive {
            self.document_view
                .window_history
                .insert(window_id, swapped_history);
            self.document_view
                .window_panel_ctx
                .insert(window_id, swapped_ctx);
        } else {
            self.owe_note_drafts_of(swapped_ctx);
        }

        // Always-run cleanup (runs on success, error return, or panic
        // via resume_unwind below): swap the canvases back and restore
        // the saved active_path. The per-window canvas (now in the
        // main slot) returns to the HashMap; the main canvas (held in
        // `swapped_canvas`) returns to the main slot.
        std::mem::swap(&mut self.interaction_state.canvas, &mut swapped_canvas);
        if window_alive {
            self.interaction_state
                .canvases
                .insert(window_id, swapped_canvas);
        }
        if let Some(mut pcb) = swapped_pcb {
            std::mem::swap(&mut self.interaction_state.pcb_canvas, &mut pcb);
            if window_alive {
                self.interaction_state.pcb_canvases.insert(window_id, pcb);
            }
            // The main canvas sat in a local while the handler ran, out of
            // reach of a change to the right-release hold.
            let swallow = self.interaction_state.swallow_right_release;
            self.interaction_state
                .pcb_canvas
                .set_swallow_right_release(swallow);
        }
        if let Some(mut selection) = swapped_pcb_selection {
            std::mem::swap(&mut self.interaction_state.pcb_selection, &mut selection);
            if window_alive {
                self.interaction_state
                    .pcb_selections
                    .insert(window_id, selection);
            }
        }
        if retargeted_tab.is_some() {
            let tabs = &self.document_state.tabs;
            self.document_state.active_tab = saved_active_tab_key
                .and_then(|(path, kind)| {
                    tabs.iter().position(|tab| {
                        tab.path == path && std::mem::discriminant(&tab.kind) == kind
                    })
                })
                // The main window's tab itself was closed: stay at its
                // position, as closing a tab does.
                .unwrap_or_else(|| saved_active_tab.min(tabs.len().saturating_sub(1)));
        }
        // The main slot's cache missed any edit made while the window's
        // canvas was swapped in. Hover and other read-only events leave the
        // revision alone and cost nothing here.
        let revision_after = self
            .document_state
            .engines
            .get(&target_path)
            .map(signex_engine::Engine::revision);
        let main_shows_target = saved_active_path.as_ref() == Some(&target_path);
        // Save As re-keyed the tab the main window also had active: follow
        // the new path rather than restore the old key.
        let saved_active_path = match (saved_active_path, retargeted_tab) {
            (Some(path), Some(index)) if path == target_path && !self.has_tab_for(&path) => self
                .document_state
                .tabs
                .get(index)
                .map(|tab| tab.path.clone()),
            (path, _) => path,
        };
        self.document_state.active_path = saved_active_path;
        // What the handler ended of main's, while main's slot sat in a local.
        self.end_main_canvas_work(&main_work);
        if main_shows_target && revision_before != revision_after {
            self.refresh_main_canvas_from_engine(&target_path);
        }
        // The main window showing the board the window just edited repaints
        // its own selection over the new geometry.
        let board_moved =
            self.document_state.pcb_revisions.get(&target_path).copied() != board_revision_before;
        let main_shows_board = self.active_tab_path().as_ref() == Some(&target_path);
        if board_moved && main_shows_board {
            self.refresh_pcb_highlight();
        } else if main_shows_board {
            // A net highlighted from the window is board-wide (#2514).
            self.sync_board_net_focus();
        }

        match task_result {
            Ok(task) => task,
            Err(payload) => resume_unwind(payload),
        }
    }
}
