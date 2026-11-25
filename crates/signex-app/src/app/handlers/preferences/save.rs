//! Preferences Save (Apply, and OK with unsaved changes): commit every
//! draft, start its `prefs.json` writes, switch the language, and persist
//! the keymap working copy. Split out of `mod.rs`.

use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_preferences_save(&mut self) -> Task<Message> {
        // A draft that cannot be committed stops Save before anything is
        // committed; the dialog stays dirty and open.
        if self.refuse_invalid_save_drafts() {
            return Task::none();
        }
        self.ui_state.theme_id = self.ui_state.preferences_draft_theme;
        self.ui_state.ui_font_name = self.ui_state.preferences_draft_font.clone();
        self.ui_state.grid_style = self.ui_state.preferences_draft_grid_style;
        self.ui_state.pcb_gpu_render = self.ui_state.preferences_draft_pcb_gpu_render;
        self.ui_state.antialiasing = self.ui_state.preferences_draft_antialiasing;
        self.ui_state.annotate_undo = self.ui_state.preferences_draft_annotate_undo;
        self.ui_state.move_keeps_wires = self.ui_state.preferences_draft_move_keeps_wires;
        self.ui_state.net_hover_enabled = self.ui_state.preferences_draft_net_hover_enabled;
        self.ui_state.edge_pan = self.ui_state.preferences_draft_edge_pan;
        // Repainted by `sync_theme` below: the canvas colours carry it.
        self.ui_state.symbol_body_fill = self.ui_state.preferences_draft_symbol_body_fill;
        // #1294: the canvases redraw their parts with or without the cross.
        self.ui_state.cross_out_not_fitted = self.ui_state.preferences_draft_cross_out_not_fitted;
        self.refresh_all_not_fitted();
        // #2868: the next dimension placed starts from these.
        self.ui_state.pcb_dimension_defaults =
            self.ui_state.preferences_pcb_defaults.defaults.clone();
        self.ui_state.preferences_pcb_defaults.texts.clear();
        // #2514: nets in focus redraw the board with the new amount; the
        // sync does nothing when no copper dims.
        self.ui_state.pcb_dim_others_percent =
            self.ui_state.preferences_draft_pcb_dim_others_percent;
        self.sync_board_net_focus();
        self.ui_state.trackpad_pan = self.ui_state.preferences_draft_trackpad_pan;
        self.ui_state.wheel_mode = self.ui_state.preferences_draft_wheel_mode;
        self.ui_state.layer_switcher = self.ui_state.preferences_draft_layer_switcher;
        // Reaches the chrome through `sync_theme` below: the tokens carry it.
        self.ui_state.selected_segment = self.ui_state.preferences_draft_selected_segment;
        self.ui_state.remember_window_geometry =
            self.ui_state.preferences_draft_remember_window_geometry;
        self.ui_state.drag_direction_select = self.ui_state.preferences_draft_drag_direction_select;
        self.ui_state.increment_step = self.ui_state.preferences_draft_increment_step;
        self.ui_state.paste_designators = self.ui_state.preferences_draft_paste_designators;
        self.refresh_paste_ghost();
        self.ui_state.window_controls_placement =
            self.ui_state.preferences_draft_window_controls_placement;
        self.ui_state.autosave_enabled = self.ui_state.preferences_draft_autosave_enabled;
        self.commit_agent_connections_pref();
        // #629 — the three Symbol Editor settings commit here like every
        // other appearance draft, instead of writing themselves to disk the
        // moment the picker moved.
        self.ui_state.symbol_grid_size_mm = self.ui_state.preferences_draft_symbol_grid_size_mm;
        self.ui_state.commit_fast_grid_drafts();
        self.ui_state.symbol_grid_style = self.ui_state.preferences_draft_symbol_grid_style;
        self.ui_state.symbol_pin_selection = self.ui_state.preferences_draft_symbol_pin_selection;
        self.ui_state.zoom_step_percent = self.ui_state.preferences_draft_zoom_step_percent;
        // Takes effect on every open window at once: the daemon's
        // `scale_factor` hook reads this field (#1228).
        let previous_ui_scale = self.ui_state.ui_scale;
        self.ui_state.ui_scale = crate::ui_scale::committed_scale(
            self.ui_state.ui_scale,
            self.ui_state.preferences_draft_ui_scale_percent,
        );
        let ui_scale_changed = self.ui_state.ui_scale != previous_ui_scale;
        // The live-preview already pushed the draft into the widget;
        // re-assert it so the saved and effective flags can't diverge.
        self.set_pcb_gpu_render(self.ui_state.pcb_gpu_render);
        self.sync_theme();
        self.document_state.panel_ctx.ui_font_name = self.ui_state.ui_font_name.clone();
        // #1351 — every write is a background task, collected in `writes`
        // and never run inline in `update`. Each keeps its per-key ticket,
        // so a later toggle of the same key still wins over this Save.
        let mut writes: Vec<Task<Message>> = Vec::new();
        // #629 — re-assert the symbol grid style from the committed value so
        // the saved and effective styles cannot diverge (same rule as the
        // PCB GPU flag above).
        self.document_state.panel_ctx.symbol_grid_style = self.ui_state.symbol_grid_style;
        self.commit_component_class_drafts();
        // Everything committed above is in `ui_state` now: queue its writes,
        // and hand new libraries the same values so none of them reads
        // `prefs.json` while those writes are in flight.
        writes.extend(self.committed_pref_writes());
        self.library.new_library_defaults = crate::library::NewLibraryDefaults {
            grid_size_mm: self.ui_state.symbol_grid_size_mm,
            pin_selection: self.ui_state.symbol_pin_selection,
            component_classes: self.ui_state.component_classes.clone(),
        };
        // Mockup row 144a — the drafted language: every window redraws in it from the
        // next frame, no restart. Its write is one more background task.
        // Before the keymap below, which compiles its labels in it.
        writes.push(self.set_language(self.ui_state.preferences_pages.draft_language.clone()));
        if !self.commit_keymap_draft() {
            // The prefs writes above are already started, so this early
            // return still has to report a file that broke since the dialog
            // opened (#602).
            self.recompute_preferences_dirty();
            return self.save_follow_up(writes, ui_scale_changed);
        }
        // Every prefs write in this arm has been started, and
        // `save_follow_up` asks the file whether they landed once they all
        // finish (#602, #1351). It is the only probe on the path a user takes
        // when they break `prefs.json` beside the running app with
        // Preferences open: the open handler early-returns then.
        //
        // Limit: the dialog reports "Saved" from the committed model, before
        // the background writes finish. A write that fails (full disk) is
        // logged and shown in the Messages panel when it completes, and a
        // file broken beyond use raises the banner through the probe.
        self.ui_state.preferences_dirty = false;
        self.ui_state.preferences_pages.dirty_pages.clear();
        self.save_follow_up(writes, ui_scale_changed)
    }

    /// Component classes: keep entries with non-empty keys and labels, and
    /// dedupe by key (last write wins), so the dropdown never shows blanks
    /// or duplicates; then commit them. The rule is the table's own
    /// (`preferences::saved_classes`).
    fn commit_component_class_drafts(&mut self) {
        self.ui_state.component_classes =
            crate::preferences::saved_classes(&self.ui_state.preferences_draft_component_classes);
        self.ui_state.preferences_draft_component_classes = self.ui_state.component_classes.clone();
        self.document_state.panel_ctx.component_classes = self.ui_state.component_classes.clone();
        // A saved row is no longer the added one: its key stops following
        // its label, and the dedupe above may have moved it (mockup row 150).
        self.ui_state.preferences_pages.classes_view.new_row = None;
    }

    /// Commit the keymap working copy: persist it first, then swap it into
    /// the live set and recompile so the running dispatch bridge
    /// (`ui_state.active_keymap`) picks up the rebinds at once. `false` when
    /// it was not saved; the reason is in the status line and the dialog
    /// stays dirty, so nothing is silently lost.
    fn commit_keymap_draft(&mut self) -> bool {
        let keymap_profiles = self
            .ui_state
            .preferences_keymap_editor
            .clone()
            .into_profiles();
        // The running set is the built-in fallback whenever the saved file
        // could not be honoured at boot, and only custom profiles are
        // serialised — so saving now would land a custom-profile-free
        // document over profiles the user still has on disk. Copy the file
        // aside first, and refuse to save at all if that copy cannot be
        // made: overwriting after a failed backup is the bug (#595).
        let mut backup_note = String::new();
        if self.ui_state.keymap_load_error.is_some() {
            match crate::keymap::back_up_profile_file() {
                Ok(Some(bak)) => {
                    self.ui_state.keymap_backup = Some(bak.display().to_string());
                    backup_note = format!(" The previous file was kept as {}.", bak.display());
                }
                Ok(None) => {}
                Err(error) => {
                    // Reason first, and short: Messages cuts at 157 bytes.
                    tracing::error!(
                        target: "signex::keymap",
                        "keymap backup failed, file left untouched: {error}"
                    );
                    self.ui_state.preferences_keymap_status = format!(
                        "Could not save keyboard shortcuts: the existing file could \
                         not be backed up first, so it was left untouched rather than \
                         overwritten ({error})."
                    );
                    return false;
                }
            }
        }
        match crate::keymap::save_profile_set(&keymap_profiles) {
            Ok(()) => {
                self.ui_state.set_keymap_profiles(keymap_profiles);
                self.ui_state.active_keymap = std::sync::Arc::new(
                    self.ui_state
                        .keymap_profiles
                        .compile_active_in(&self.ui_state.i18n),
                );
                self.ui_state.preferences_keymap_recorder = None;
                self.ui_state.preferences_keymap_status =
                    format!("Keyboard shortcuts saved.{backup_note}");
                // The file on disk is now valid and written by this process,
                // so the boot-time failure no longer holds: drop the banner
                // and let the next Apply be an ordinary save.
                self.ui_state.keymap_load_error = None;
                true
            }
            Err(error) => {
                self.ui_state.preferences_keymap_status =
                    format!("Could not save keyboard shortcuts: {error}");
                false
            }
        }
    }
}
