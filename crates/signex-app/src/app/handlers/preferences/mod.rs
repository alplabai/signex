use iced::Task;

mod body_fill;
mod committed_writes;
mod component_classes;
mod drafts;
mod keymap;
#[cfg(test)]
mod keymap_tests;
pub(crate) mod language;
mod pages_view;
mod pcb_defaults;
#[cfg(test)]
mod pcb_defaults_tests;
mod prefs_file;
mod profile_page;
mod profiles;
#[cfg(test)]
mod profiles_tests;
mod save;
mod save_guard;
#[cfg(test)]
mod selected_segment_tests;
#[cfg(test)]
mod theme_repaint_tests;

use super::super::*;

impl Signex {
    /// Rescaling changes the window's logical size without a resize event,
    /// so ask for it again: the dialog caps and the dock maths read
    /// `window_size`. Every exit of Save after the scale is committed
    /// returns this.
    fn rescale_follow_up(&self, ui_scale_changed: bool) -> Task<Message> {
        if ui_scale_changed {
            self.requery_main_window_size()
        } else {
            Task::none()
        }
    }

    /// What Save returns once its `prefs.json` writes are started (#1351):
    /// the writes, then, after the last one finishes, `SaveWritesFinished`
    /// so the file-health probe (#602) sees their outcome, plus the rescale
    /// re-query.
    fn save_follow_up(&self, writes: Vec<Task<Message>>, ui_scale_changed: bool) -> Task<Message> {
        let probe_after_writes = Task::batch(writes).chain(Task::done(Message::Preferences(
            PreferencesMsg::Inner(crate::preferences::PrefMsg::SaveWritesFinished),
        )));
        Task::batch([probe_after_writes, self.rescale_follow_up(ui_scale_changed)])
    }

    /// Ask winit for the main window's current logical size, which feeds
    /// `ui_state.window_size` through `WindowResizedFor`.
    fn requery_main_window_size(&self) -> Task<Message> {
        let Some(id) = self.ui_state.main_window_id else {
            return Task::none();
        };
        iced::window::size(id).map(move |size| {
            Message::Window(WindowMsg::WindowResizedFor(id, size.width, size.height))
        })
    }

    pub(crate) fn handle_preferences_open_requested(&mut self) -> Task<Message> {
        // Re-invoking Preferences while it's already open must NOT reseed:
        // `handle_detach_modal` no-ops on an already-detached modal, so a
        // reseed here would wipe the dirty flag + drafts while the live
        // previews (`pcb_canvas.gpu_render`, the `render_config` globals,
        // `panel_ctx.tokens`) keep the draft values — silently reinstating
        // the lingering-preview divergence the dirty-close guards exist to
        // prevent. Keep the session exactly as the user left it (the detach
        // call below retries the in-window fallback's detach, else no-ops).
        if self.ui_state.preferences_open {
            return self.handle_detach_modal(super::super::state::ModalId::Preferences);
        }
        self.ui_state.preferences_open = true;
        self.seed_preferences_drafts_from_live();
        self.ui_state.preferences_keymap_search.clear();
        self.ui_state.preferences_nav_search.clear();
        // The pages' view state (search, open sections, ...) is per session.
        self.ui_state.preferences_pages.pages_view = Default::default();
        self.ui_state.preferences_pages.keymap_view = Default::default();
        self.ui_state.preferences_pages.classes_view = Default::default();
        self.ui_state.panel_list_open = false;
        self.interaction_state.context_menu = None;
        // Open Preferences as a separate OS window so the user can move
        // it around / drag it onto a second monitor. Same pattern as
        // Print Preview, BOM Preview, Annotate, ERC.
        let detach = self.handle_detach_modal(super::super::state::ModalId::Preferences);
        Task::batch([detach, self.profiles_pane_task()])
    }

    /// App-level close request (Esc ladder, `PreferencesMsg::Close`). Same
    /// dirty-close guard as the in-dialog X button (`PrefMsg::Close`): while
    /// unsaved changes exist the dialog stays open — the footer's
    /// "Unsaved changes" bar with Save / Discard & Close is the only way to
    /// resolve them, so no dismiss route can silently drop work. The guard
    /// also covers the detach-failed in-window fallback, where no
    /// `SecondaryWindowClosed` backstop ever fires to revert a lingering
    /// live preview (e.g. the experimental PCB GPU render toggle).
    pub(crate) fn handle_preferences_close_requested(&mut self) -> Task<Message> {
        if self.ui_state.preferences_dirty {
            // The footer's "Unsaved changes" bar is already visible; the log
            // line says why a dismiss (OS close, X, Esc) did nothing, once
            // per attempt.
            crate::diagnostics::log_warning(
                "Preferences has unsaved changes; Save or Discard & Close before closing",
            );
            return Task::none();
        }
        self.ui_state.preferences_open = false;
        self.ui_state.preferences_keymap_recorder = None;
        self.close_detached_modal(super::super::state::ModalId::Preferences)
    }

    /// Seed every Preferences draft from its saved live value and reset the
    /// dialog's transient keymap state + dirty flags. Shared by open (fresh
    /// dialog) and [`Self::revert_preferences_drafts`] (discard) so a future
    /// draft field can't be added to one block and silently missed by the
    /// other — the drift class `preferences_draft_differs` exists to repair.
    fn seed_preferences_drafts_from_live(&mut self) {
        self.ui_state.preferences_draft_theme = self.ui_state.theme_id;
        self.ui_state.preferences_draft_font = self.ui_state.ui_font_name.clone();
        self.ui_state.preferences_draft_component_classes = self.ui_state.component_classes.clone();
        self.ui_state.preferences_draft_grid_style = self.ui_state.grid_style;
        self.ui_state.preferences_draft_pcb_gpu_render = self.ui_state.pcb_gpu_render;
        self.ui_state.preferences_draft_antialiasing = self.ui_state.antialiasing;
        self.ui_state.preferences_draft_annotate_undo = self.ui_state.annotate_undo;
        self.ui_state.preferences_draft_move_keeps_wires = self.ui_state.move_keeps_wires;
        self.ui_state.preferences_draft_net_hover_enabled = self.ui_state.net_hover_enabled;
        self.ui_state.preferences_draft_edge_pan = self.ui_state.edge_pan;
        self.ui_state.preferences_draft_symbol_body_fill = self.ui_state.symbol_body_fill;
        self.ui_state.preferences_draft_cross_out_not_fitted = self.ui_state.cross_out_not_fitted;
        self.ui_state.preferences_body_fill_palette_open = false;
        self.ui_state.preferences_body_fill_picker_open = false;
        *self.ui_state.preferences_pcb_defaults =
            crate::pcb_defaults_prefs::PcbDefaultsDraft::of(&self.ui_state.pcb_dimension_defaults);
        self.ui_state.preferences_draft_trackpad_pan = self.ui_state.trackpad_pan;
        self.ui_state.preferences_draft_wheel_mode = self.ui_state.wheel_mode;
        self.ui_state.preferences_draft_layer_switcher = self.ui_state.layer_switcher;
        self.ui_state.preferences_draft_selected_segment = self.ui_state.selected_segment;
        self.ui_state.preferences_draft_remember_window_geometry =
            self.ui_state.remember_window_geometry;
        self.ui_state.preferences_draft_drag_direction_select = self.ui_state.drag_direction_select;
        self.ui_state.preferences_draft_increment_step = self.ui_state.increment_step;
        self.ui_state.preferences_draft_paste_designators = self.ui_state.paste_designators;
        self.ui_state.preferences_draft_window_controls_placement =
            self.ui_state.window_controls_placement;
        self.ui_state.preferences_draft_autosave_enabled = self.ui_state.autosave_enabled;
        self.ui_state.preferences_draft_agent_connections_enabled =
            self.ui_state.agent_connections_enabled;
        // #629 — the three Symbol Editor settings. Before they had
        // committed fields there was nothing to seed them from, so a
        // Discard left them showing (and rendering) the abandoned value.
        self.ui_state.preferences_draft_symbol_grid_size_mm = self.ui_state.symbol_grid_size_mm;
        self.ui_state.seed_fast_grid_drafts();
        self.ui_state.preferences_draft_symbol_grid_style = self.ui_state.symbol_grid_style;
        self.ui_state.preferences_draft_symbol_pin_selection = self.ui_state.symbol_pin_selection;
        self.ui_state.preferences_draft_zoom_step_percent = self.ui_state.zoom_step_percent;
        self.ui_state.preferences_draft_pcb_dim_others_percent =
            self.ui_state.pcb_dim_others_percent;
        self.ui_state.preferences_draft_ui_scale_percent =
            crate::ui_scale::scale_to_percent(self.ui_state.ui_scale);
        // Mockup row 144a — the language in force, its preview and the system name.
        self.seed_language_draft();
        // Fresh working copy of the live profile set for the Keyboard
        // Shortcuts pane; drop any stale recorder / status with it.
        self.ui_state.preferences_keymap_editor =
            crate::keymap::KeymapEditorModel::new(self.ui_state.keymap_profiles.clone());
        self.ui_state.preferences_keymap_status.clear();
        self.ui_state.preferences_keymap_recorder = None;
        // The Rule Profiles status line is transient feedback about one
        // action, not a draft — a fresh dialog (or a Discard) must not
        // reopen showing the last session's result.
        self.ui_state.profile_manager.clear_transient();
        // Same rule for the prefs-file recovery result — one action's
        // outcome, not a draft.
        self.ui_state.preferences_prefs_status.clear();
        // Re-probe the prefs file itself (#602). `keymap_load_error` is
        // deliberately NOT cleared here (see `UiState`): it can only be
        // trusted again once this process has written that file. This one
        // can, because the check is a single file read with no state
        // behind it — so a user who repairs `prefs.json` in an editor and
        // reopens Preferences sees the banner gone instead of being told
        // to restart, and one who breaks it mid-session sees it appear.
        self.refresh_prefs_load_error();
        // #603 — and the shortcuts backup, for the same reason: the file
        // can appear or be removed between two openings of this dialog.
        self.refresh_keymap_backup();
        self.ui_state.preferences_dirty = false;
        self.ui_state.preferences_pages.dirty_pages.clear();
    }

    /// Re-check whether `keyboard_shortcuts.toml.bak` is on disk (#603).
    ///
    /// Deliberately not derived from `keymap_load_error`: the backup
    /// outlives the failure that produced it, so the Restore and Discard
    /// controls have to stay reachable long after the banner is gone.
    fn refresh_keymap_backup(&mut self) {
        self.ui_state.keymap_backup =
            crate::keymap::existing_backup_profiles_path().map(|bak| bak.display().to_string());
    }

    /// The backup the Preferences row currently shows: what Restore and
    /// Discard act on. They never re-scan the directory, since a newer
    /// backup (the one Apply just wrote) is a different file from the one
    /// the user is looking at. When the shown file is gone the action is
    /// refused, the row is refreshed, and the status line says so; with no
    /// backup shown it says `none_shown`.
    fn displayed_keymap_backup(
        &mut self,
        none_shown: signex_i18n::MessageId,
    ) -> Option<std::path::PathBuf> {
        let Some(shown) = self.ui_state.keymap_backup.clone() else {
            self.refresh_keymap_backup();
            self.ui_state
                .preferences_keymap_status
                .set(signex_i18n::Translatable::new(none_shown));
            return None;
        };
        let path = std::path::PathBuf::from(shown);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => Some(path),
            Err(error) => {
                tracing::warn!(
                    target: "signex::keymap",
                    path = %path.display(),
                    error = %error,
                    "the keyboard-shortcuts backup shown in Preferences is no longer there; \
                     nothing was changed"
                );
                self.refresh_keymap_backup();
                self.ui_state
                    .preferences_keymap_status
                    .set(keymap::keymap_path_status(
                        signex_i18n::shortcut_ids::PREF_KEYMAP_STATUS_BACKUP_GONE,
                        &path,
                    ));
                None
            }
        }
    }

    /// Re-read `prefs.json`'s health into the banner flag (#602).
    ///
    /// One cheap file read, so it can run on every Preferences open and
    /// after every burst of preference writes rather than being trusted
    /// from boot. That second call site is what catches a file broken
    /// *while* the dialog is open: `handle_preferences_open_requested`
    /// early-returns when the dialog is already up, so without a probe on
    /// the Save path a user who hand-edits the file beside the running app
    /// gets every write refused with the dialog still reporting success.
    fn refresh_prefs_load_error(&mut self) {
        self.ui_state.prefs_load_error = crate::fonts::check_prefs_file()
            .err()
            .map(|error| error.to_string());
    }

    pub(crate) fn handle_preferences_navigation_requested(
        &mut self,
        nav: crate::preferences::PrefNav,
    ) -> Task<Message> {
        self.ui_state.preferences_nav = nav;
        self.profiles_pane_task()
    }

    /// Revert every Preferences live-preview draft back to its saved value and
    /// repaint, so discarding — the Discard & Close button, or the backstop
    /// for a dirty window destroyed outside the guarded close paths — drops
    /// unsaved previews instead of leaving them silently active (e.g. the
    /// experimental PCB GPU render toggle would otherwise keep rendering on
    /// the GPU with the checkbox showing unchecked). Does not touch
    /// `preferences_open` or close the window; callers own that. Idempotent —
    /// safe to call more than once.
    pub(crate) fn revert_preferences_drafts(&mut self) {
        self.seed_preferences_drafts_from_live();
        // Revert the live-preview GPU flag to the saved value; the content
        // cache clear below repaints the PCB canvas so a discarded toggle
        // takes no effect.
        self.set_pcb_gpu_render(self.ui_state.pcb_gpu_render);
        // #630 — the schematic grid style previewed live, so push the
        // committed value back to every window's canvas.
        // #629 — the symbol grid style is the only one of the three
        // Symbol Editor settings with a live preview, so it is the only
        // one to push back. Grid size and pin selection reach the editor
        // through `LibraryDisplaySettings` when a library is opened, so
        // restoring their draft fields above is enough.
        self.document_view.panel_ctx.symbol_grid_style = self.ui_state.symbol_grid_style;
        // `sync_theme` re-derives the canvas colours from the saved
        // theme, clears BOTH content caches (schematic + PCB) and every
        // schematic and PCB bg cache — don't clear them again here.
        self.sync_theme();
    }

    /// Set the effective PCB GPU-render flag on every PCB canvas (#494).
    fn set_pcb_gpu_render(&mut self, enabled: bool) {
        for canvas in self.interaction_state.all_pcb_canvases_mut() {
            canvas.gpu_render = enabled;
        }
    }

    pub(crate) fn handle_preferences_message(
        &mut self,
        msg: crate::preferences::PrefMsg,
    ) -> Task<Message> {
        use crate::preferences::PrefMsg;

        match msg {
            PrefMsg::Noop => {}
            PrefMsg::SaveWritesFinished => self.refresh_prefs_load_error(),
            PrefMsg::NavSearchChanged(query) => {
                self.ui_state.preferences_nav_search = query;
            }
            PrefMsg::Nav(nav) => {
                self.ui_state.preferences_nav = nav;
                return self.profiles_pane_task();
            }
            PrefMsg::Close => {
                // One close path for every dismiss route (X button here,
                // Esc / app-level requests via `PreferencesMsg::Close`) —
                // all share the same dirty-close guard.
                return self.handle_preferences_close_requested();
            }
            PrefMsg::DiscardAndClose => {
                self.revert_preferences_drafts();
                self.ui_state.preferences_open = false;
                return self.close_detached_modal(super::super::state::ModalId::Preferences);
            }
            PrefMsg::SaveAndClose => {
                // Save as the Save button does; a refused save leaves the
                // dialog dirty, so the close guard keeps it open. A refused
                // pitch label is not a dirty draft: it keeps it open itself.
                let held = self.snap_label_holds_refused_text();
                let saved = self.handle_preferences_save();
                if held {
                    return saved;
                }
                return Task::batch([saved, self.handle_preferences_close_requested()]);
            }
            PrefMsg::Save => return self.handle_preferences_save(),
            // The drafts and the component-class edits live in `drafts`.
            // Listed one by one rather than caught by a `_ =>`, so this
            // match stays exhaustive over `PrefMsg`: a new variant is a
            // compile error here, not a message routed to the wrong owner.
            msg @ (PrefMsg::DraftTheme(..)
            | PrefMsg::DraftFont(..)
            | PrefMsg::DraftGridStyle(..)
            | PrefMsg::DraftPcbGpuRender(..)
            | PrefMsg::DraftAntialiasing(..)
            | PrefMsg::DraftAnnotateUndo(..)
            | PrefMsg::DraftMoveKeepsWires(..)
            | PrefMsg::DraftDragDirectionSelect(..)
            | PrefMsg::DraftNetHoverEnabled(..)
            | PrefMsg::DraftEdgePanEnabled(..)
            | PrefMsg::DraftEdgePanSpeed(..)
            | PrefMsg::SymbolBodyFill(..)
            | PrefMsg::DraftCrossOutNotFitted(..)
            | PrefMsg::DraftTrackpadPan(..)
            | PrefMsg::DraftWheelMode(..)
            | PrefMsg::DraftLayerSwitcher(..)
            | PrefMsg::DraftSelectedSegment(..)
            | PrefMsg::DraftRememberWindowGeometry(..)
            | PrefMsg::DraftIncrementStep(..)
            | PrefMsg::DraftPasteDesignators(..)
            | PrefMsg::DraftWindowControlsPlacement(..)
            | PrefMsg::DraftAutosaveEnabled(..)
            | PrefMsg::DraftAgentConnectionsEnabled(..)
            | PrefMsg::DraftSymbolGridSize(..)
            | PrefMsg::DraftFastGrid(..)
            | PrefMsg::DraftSymbolGridStyle(..)
            | PrefMsg::DraftZoomStepPercent(..)
            | PrefMsg::DraftPcbDimOthersPercent(..)
            | PrefMsg::DraftUiScalePercent(..)
            | PrefMsg::DraftSymbolPinSelection(..)
            | PrefMsg::DraftLanguage(..)
            | PrefMsg::ComponentClassEditKey { .. }
            | PrefMsg::ComponentClassEditLabel { .. }
            | PrefMsg::ComponentClassAdd
            | PrefMsg::ComponentClassRemove { .. }
            | PrefMsg::ComponentClassResetDefaults) => {
                return self.handle_preferences_draft(msg);
            }
            PrefMsg::PcbDefaults(msg) => self.handle_pcb_defaults_message(msg),
            PrefMsg::Profiles(msg) => return self.handle_profiles_message(msg),
            PrefMsg::ResetPrefsFile => self.handle_prefs_file_reset(),
            PrefMsg::PrefsFolderOpenRequested => return self.open_prefs_folder(),
            PrefMsg::PagesView(msg) => return self.handle_preferences_pages_view(msg),
            PrefMsg::KeymapView(msg) => return self.handle_preferences_keymap_view(msg),
            PrefMsg::ClassesView(msg) => return self.handle_preferences_classes_view(msg),
            PrefMsg::LibrarySettings(settings_msg) => {
                // Route the Distributor APIs panel's SettingsMsg back
                // through the library dispatcher so the canonical
                // state (`LibraryState.settings`) and any async tasks
                // (OAuth flow, Mouser test) live in one place.
                // Returning the dispatch task lets long-running flows
                // like the OAuth handshake settle on the iced runtime.
                return self.dispatch_library_message(
                    crate::library::messages::LibraryMessage::Settings(settings_msg),
                );
            }
            // Keyboard Shortcuts pane — the arms live in the sibling
            // `keymap` module, listed one by one for the same reason as
            // the drafts above.
            msg @ (PrefMsg::KeymapSearchChanged(..)
            | PrefMsg::KeymapProfileSelected(..)
            | PrefMsg::KeymapCreateCustomProfile
            | PrefMsg::KeymapDeleteActiveProfile
            | PrefMsg::KeymapRestoreFromBackup
            | PrefMsg::KeymapDiscardBackup
            | PrefMsg::KeymapImportProfile
            | PrefMsg::KeymapProfileLoaded(..)
            | PrefMsg::KeymapProfileUnreadable(..)
            | PrefMsg::KeymapExportProfile
            | PrefMsg::KeymapExportFinished(..)
            | PrefMsg::KeymapRecorderOpen { .. }
            | PrefMsg::KeymapRecorderCancel
            | PrefMsg::KeymapRecorderStart
            | PrefMsg::KeymapRecorderStop
            | PrefMsg::KeymapRecorderClear
            | PrefMsg::KeymapRecorderModifiersChanged(..)
            | PrefMsg::KeymapRecorderKeyPressed(..)
            | PrefMsg::KeymapRecorderApply) => {
                return self.handle_keymap_pref_message(msg);
            }
        }

        Task::none()
    }
}

/// The text of a file a dialog picked to import. A read failure is logged
/// and returned as the reason, for the pane's status line.
async fn read_picked(path: &std::path::Path) -> Result<String, String> {
    match tokio::fs::read(path).await {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(error) => {
            let reason = format!("{} could not be read: {error}", path.display());
            crate::diagnostics::log_warning(format!("Import: {reason}"));
            Err(reason)
        }
    }
}
