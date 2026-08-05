use iced::Task;

mod keymap;

use super::super::*;

impl Signex {
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
        self.ui_state.panel_list_open = false;
        self.interaction_state.context_menu = None;
        // Open Preferences as a separate OS window so the user can move
        // it around / drag it onto a second monitor. Same pattern as
        // Print Preview, BOM Preview, Annotate, ERC.
        self.handle_detach_modal(super::super::state::ModalId::Preferences)
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
        self.ui_state.preferences_draft_power_port_style = self.ui_state.power_port_style;
        self.ui_state.preferences_draft_label_style = self.ui_state.label_style;
        self.ui_state.preferences_draft_multisheet_style = self.ui_state.multisheet_style;
        self.ui_state.preferences_draft_component_classes = self.ui_state.component_classes.clone();
        self.ui_state.preferences_draft_grid_style = self.ui_state.grid_style;
        self.ui_state.preferences_draft_pcb_gpu_render = self.ui_state.pcb_gpu_render;
        // Fresh working copy of the live profile set for the Keyboard
        // Shortcuts pane; drop any stale recorder / status with it.
        self.ui_state.preferences_keymap_editor =
            crate::keymap::KeymapEditorModel::new(self.ui_state.keymap_profiles.clone());
        self.ui_state.preferences_keymap_status.clear();
        self.ui_state.preferences_keymap_recorder = None;
        // The Appearance status line is transient feedback about one
        // export, not a draft — a fresh dialog (or a Discard) must not
        // reopen showing the last session's result.
        self.ui_state.preferences_theme_status.clear();
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
        self.ui_state.preferences_dirty_sticky = false;
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

    /// Re-read `prefs.json`'s health into the banner flag (#602).
    ///
    /// One cheap file read, so it can run on every Preferences open and
    /// after every burst of preference writes rather than being trusted
    /// from boot. That second call site is what catches a file broken
    /// *while* the dialog is open: `handle_preferences_open_requested`
    /// early-returns when the dialog is already up, so without a probe on
    /// the Save path a user who hand-edits the file beside the running app
    /// — the very workflow the banner's "repair it by hand" advice invites
    /// — gets every write refused with the dialog still reporting success.
    fn refresh_prefs_load_error(&mut self) {
        self.ui_state.prefs_load_error = crate::fonts::check_prefs_file()
            .err()
            .map(|error| error.to_string());
    }

    /// Recompute the cached dirty flag from the full unsaved-state
    /// predicate. Every Preferences draft mutation routes through this —
    /// never assign `preferences_dirty` from an ad-hoc comparison; that
    /// drift class is exactly what let imperative edits be clobbered back
    /// to "clean" (review #308 finding 1).
    fn recompute_preferences_dirty(&mut self) {
        self.ui_state.preferences_dirty = self.ui_state.preferences_has_unsaved_changes();
    }

    pub(crate) fn handle_preferences_navigation_requested(
        &mut self,
        nav: crate::preferences::PrefNav,
    ) -> Task<Message> {
        self.ui_state.preferences_nav = nav;
        Task::none()
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
        self.interaction_state.pcb_canvas.gpu_render = self.ui_state.pcb_gpu_render;
        // Revert the render_config globals used for schematic live preview.
        let tokens = if self.ui_state.theme_id == ThemeId::Custom {
            self.ui_state
                .custom_theme
                .as_ref()
                .map(|c| c.tokens)
                .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
        } else {
            signex_types::theme::theme_tokens(self.ui_state.theme_id)
        };
        self.document_state.panel_ctx.tokens = tokens;
        crate::render_config::set_power_port_style(self.ui_state.power_port_style);
        crate::render_config::set_label_style(self.ui_state.label_style);
        crate::render_config::set_multisheet_style(self.ui_state.multisheet_style);
        crate::render_config::set_grid_style(self.ui_state.grid_style);
        // `update_canvas_theme` re-derives the canvas colours from the saved
        // theme and clears BOTH content caches (schematic + PCB) — don't
        // clear them again here. Only the bg caches (grid style / dot layer)
        // are ours to invalidate.
        self.update_canvas_theme();
        self.interaction_state.pcb_canvas.clear_bg_cache();
        self.interaction_state.active_canvas_mut().clear_bg_cache();
    }

    pub(crate) fn handle_preferences_message(
        &mut self,
        msg: crate::preferences::PrefMsg,
    ) -> Task<Message> {
        use crate::preferences::PrefMsg;

        match msg {
            PrefMsg::Nav(nav) => {
                self.ui_state.preferences_nav = nav;
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
            PrefMsg::Save => {
                // Refuse to commit while any keymap trigger draft is
                // unparseable — surface the reason and keep the dialog
                // dirty/open rather than persisting a broken profile.
                if self
                    .ui_state
                    .preferences_keymap_editor
                    .has_invalid_trigger_drafts()
                {
                    self.ui_state.preferences_keymap_status =
                        "Fix invalid keyboard shortcuts before saving.".to_string();
                    self.recompute_preferences_dirty();
                    return Task::none();
                }
                self.ui_state.theme_id = self.ui_state.preferences_draft_theme;
                self.ui_state.ui_font_name = self.ui_state.preferences_draft_font.clone();
                self.ui_state.power_port_style = self.ui_state.preferences_draft_power_port_style;
                self.ui_state.label_style = self.ui_state.preferences_draft_label_style;
                self.ui_state.multisheet_style = self.ui_state.preferences_draft_multisheet_style;
                self.ui_state.grid_style = self.ui_state.preferences_draft_grid_style;
                self.ui_state.pcb_gpu_render = self.ui_state.preferences_draft_pcb_gpu_render;
                // The live-preview already pushed the draft into the widget;
                // re-assert it so the saved and effective flags can't diverge.
                self.interaction_state.pcb_canvas.gpu_render = self.ui_state.pcb_gpu_render;
                self.update_canvas_theme();
                let tokens = if self.ui_state.theme_id == ThemeId::Custom {
                    self.ui_state
                        .custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(self.ui_state.theme_id)
                };
                self.document_state.panel_ctx.tokens = tokens;
                self.document_state.panel_ctx.ui_font_name = self.ui_state.ui_font_name.clone();
                crate::render_config::set_power_port_style(self.ui_state.power_port_style);
                crate::render_config::set_label_style(self.ui_state.label_style);
                crate::render_config::set_multisheet_style(self.ui_state.multisheet_style);
                crate::render_config::set_grid_style(self.ui_state.grid_style);
                crate::fonts::write_ui_font_pref(&self.ui_state.ui_font_name);
                crate::fonts::write_power_port_style_pref(self.ui_state.power_port_style);
                crate::fonts::write_label_style_pref(self.ui_state.label_style);
                crate::fonts::write_multisheet_style_pref(self.ui_state.multisheet_style);
                crate::fonts::write_grid_style_pref(self.ui_state.grid_style);
                crate::fonts::write_pcb_gpu_render_pref(self.ui_state.pcb_gpu_render);
                crate::fonts::write_theme_pref(self.ui_state.theme_id);
                // Component classes — keep entries with non-empty keys
                // and labels, dedupe by key (last write wins) so the
                // dropdown never shows blanks or duplicates.
                let mut sanitised: Vec<crate::fonts::ComponentClassEntry> = Vec::new();
                for entry in &self.ui_state.preferences_draft_component_classes {
                    let key = entry.key.trim();
                    let label = entry.label.trim();
                    if key.is_empty() || label.is_empty() {
                        continue;
                    }
                    if let Some(existing) = sanitised.iter_mut().find(|e| e.key == key) {
                        existing.label = label.to_string();
                    } else {
                        sanitised.push(crate::fonts::ComponentClassEntry {
                            key: key.to_string(),
                            label: label.to_string(),
                        });
                    }
                }
                // Persist first (the prefs writer borrows by ref so
                // it can run before any moves), then move `sanitised`
                // into the live registry. The draft + panel-ctx
                // mirrors clone from the field rather than from
                // `sanitised` so we drop one redundant clone.
                crate::fonts::write_component_classes_pref(&sanitised);
                self.ui_state.component_classes = sanitised;
                self.ui_state.preferences_draft_component_classes =
                    self.ui_state.component_classes.clone();
                self.document_state.panel_ctx.component_classes =
                    self.ui_state.component_classes.clone();
                // Commit the keymap working copy: persist to disk first,
                // then swap it into the live set and recompile so the
                // running dispatch bridge (`ui_state.active_keymap`)
                // picks up the rebinds immediately. A save failure is
                // surfaced in the status line and leaves the dialog
                // dirty so nothing is silently lost.
                let keymap_profiles = self
                    .ui_state
                    .preferences_keymap_editor
                    .clone()
                    .into_profiles();
                // The running set is the built-in fallback whenever the
                // saved file could not be honoured at boot, and only
                // custom profiles are serialised — so saving now would
                // land a custom-profile-free document over profiles the
                // user still has on disk. Copy the file aside first, and
                // refuse to save at all if that copy cannot be made:
                // overwriting after a failed backup is the bug (#595).
                let mut backup_note = String::new();
                if self.ui_state.keymap_load_error.is_some() {
                    match crate::keymap::back_up_profile_file() {
                        Ok(Some(bak)) => {
                            backup_note =
                                format!(" The previous file was kept as {}.", bak.display());
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.ui_state.preferences_keymap_status = format!(
                                "Could not save keyboard shortcuts: the existing file could \
                                 not be backed up first, so it was left untouched rather than \
                                 overwritten ({error})."
                            );
                            // The prefs writes above already ran, so this
                            // early return is still a path that has to
                            // report a file that broke since the dialog
                            // opened (#602).
                            self.refresh_prefs_load_error();
                            self.recompute_preferences_dirty();
                            return Task::none();
                        }
                    }
                }
                match crate::keymap::save_profile_set(&keymap_profiles) {
                    Ok(()) => {
                        self.ui_state.keymap_profiles = keymap_profiles;
                        self.ui_state.active_keymap =
                            self.ui_state.keymap_profiles.compile_active();
                        self.ui_state.preferences_keymap_recorder = None;
                        self.ui_state.preferences_keymap_status =
                            format!("Keyboard shortcuts saved.{backup_note}");
                        // The file on disk is now valid and written by this
                        // process, so the boot-time failure no longer holds:
                        // drop the banner and let the next Apply be an
                        // ordinary save.
                        self.ui_state.keymap_load_error = None;
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not save keyboard shortcuts: {error}");
                        // Same reason as the backup-failure return above:
                        // the prefs writes already ran (#602).
                        self.refresh_prefs_load_error();
                        self.recompute_preferences_dirty();
                        return Task::none();
                    }
                }
                // Every prefs write in this arm has now been attempted, so
                // ask the file whether they landed (#602). This is the
                // only probe on the path a user takes when they break
                // `prefs.json` beside the running app with Preferences
                // already open — the open handler early-returns in that
                // case, so nothing else would ever raise the banner and
                // the dialog would keep reporting a save that was refused.
                // One extra file read per Save.
                self.refresh_prefs_load_error();
                self.ui_state.preferences_dirty = false;
                self.ui_state.preferences_dirty_sticky = false;
            }
            PrefMsg::DraftTheme(id) => {
                self.ui_state.preferences_draft_theme = id;
                let tokens = if id == ThemeId::Custom {
                    self.ui_state
                        .custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(id)
                };
                self.document_state.panel_ctx.tokens = tokens;
                let canvas_colors = if id == ThemeId::Custom {
                    self.ui_state
                        .custom_theme
                        .as_ref()
                        .map(|c| c.canvas)
                        .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
                } else {
                    signex_types::theme::canvas_colors(id)
                };
                self.interaction_state.active_canvas_mut().set_theme_colors(
                    crate::render_config::to_iced(&canvas_colors.background),
                    crate::render_config::to_iced(&canvas_colors.grid),
                    crate::render_config::to_iced(&canvas_colors.paper),
                );
                self.interaction_state.active_canvas_mut().canvas_colors = canvas_colors;
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.recompute_preferences_dirty();
            }
            PrefMsg::DraftFont(name) => {
                self.ui_state.preferences_draft_font = name;
                self.recompute_preferences_dirty();
            }
            PrefMsg::DraftPowerPortStyle(style) => {
                self.ui_state.preferences_draft_power_port_style = style;
                crate::render_config::set_power_port_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.recompute_preferences_dirty();
            }
            PrefMsg::DraftLabelStyle(style) => {
                self.ui_state.preferences_draft_label_style = style;
                crate::render_config::set_label_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.recompute_preferences_dirty();
            }
            PrefMsg::DraftMultisheetStyle(style) => {
                self.ui_state.preferences_draft_multisheet_style = style;
                crate::render_config::set_multisheet_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.recompute_preferences_dirty();
            }
            PrefMsg::DraftGridStyle(style) => {
                self.ui_state.preferences_draft_grid_style = style;
                crate::render_config::set_grid_style(style);
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.recompute_preferences_dirty();
            }
            PrefMsg::DraftPcbGpuRender(enabled) => {
                self.ui_state.preferences_draft_pcb_gpu_render = enabled;
                // Live preview: push the draft into the widget's effective flag
                // and repaint the PCB canvas so the toggle shows immediately.
                // Persisted only on Save; reverted on Discard.
                self.interaction_state.pcb_canvas.gpu_render = enabled;
                self.interaction_state.pcb_canvas.clear_content_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                self.recompute_preferences_dirty();
            }
            PrefMsg::DraftSymbolGridSize(size) => {
                self.ui_state.preferences_draft_symbol_grid_size_mm = size;
                crate::fonts::write_symbol_grid_size_mm_pref(size);
            }
            PrefMsg::DraftSymbolGridStyle(style) => {
                self.ui_state.preferences_draft_symbol_grid_style = style;
                crate::render_config::set_symbol_grid_style(style);
                crate::fonts::write_symbol_grid_style_pref(style);
            }
            PrefMsg::DraftSymbolPinSelection(mode) => {
                self.ui_state.preferences_draft_symbol_pin_selection = mode;
                crate::fonts::write_symbol_pin_selection_pref(mode);
            }
            PrefMsg::ImportTheme => {
                return Task::future(async {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Import Signex Theme")
                        .add_filter("Signex Theme", &["json"])
                        .pick_file()
                        .await;
                    if let Some(f) = picked {
                        let bytes = f.read().await;
                        let s = String::from_utf8_lossy(&bytes).to_string();
                        Message::Preferences(PreferencesMsg::Inner(PrefMsg::ThemeFileLoaded(s)))
                    } else {
                        Message::Noop
                    }
                });
            }
            PrefMsg::ExportTheme => {
                let id = self.ui_state.preferences_draft_theme;
                let name = if id == ThemeId::Custom {
                    self.ui_state
                        .custom_theme
                        .as_ref()
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "Custom".to_string())
                } else {
                    id.label().to_string()
                };
                let tokens = if id == ThemeId::Custom {
                    self.ui_state
                        .custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(id)
                };
                let canvas = if id == ThemeId::Custom {
                    self.ui_state
                        .custom_theme
                        .as_ref()
                        .map(|c| c.canvas)
                        .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
                } else {
                    signex_types::theme::canvas_colors(id)
                };
                let export = signex_types::theme::CustomThemeFile {
                    name,
                    tokens,
                    canvas,
                };
                let json = serde_json::to_string_pretty(&export).unwrap_or_default();
                // Drop any status from a previous attempt so the line the
                // user sees always describes the export they just started.
                self.ui_state.preferences_theme_status.clear();
                return Task::future(async move {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Export Signex Theme")
                        .add_filter("Signex Theme", &["json"])
                        .set_file_name("custom-theme.json")
                        .save_file()
                        .await;
                    let Some(f) = picked else {
                        // Cancelled the save dialog — nothing was
                        // attempted, so there is nothing to report.
                        return Message::Noop;
                    };
                    let path = f.path().to_path_buf();
                    // `rfd::FileHandle::write` is a whole-file
                    // `std::fs::write` on a worker thread and yields
                    // `io::Result<()>` — no short write to handle, but a
                    // real failure (read-only volume, full disk, a path
                    // that vanished between pick and write) used to be
                    // discarded here, leaving the user believing the
                    // theme had been exported (#533).
                    let outcome = match f.write(json.as_bytes()).await {
                        Ok(()) => Ok(path),
                        Err(error) => Err(format!("{path}: {error}", path = path.display())),
                    };
                    Message::Preferences(PreferencesMsg::Inner(PrefMsg::ThemeExportFinished(
                        outcome,
                    )))
                });
            }
            PrefMsg::ThemeExportFinished(result) => {
                self.ui_state.preferences_theme_status = match result {
                    Ok(path) => format!("Exported theme to {}.", path.display()),
                    Err(detail) => format!("Could not export theme: {detail}"),
                };
            }
            PrefMsg::ThemeFileLoaded(content) => {
                if let Ok(custom) =
                    serde_json::from_str::<signex_types::theme::CustomThemeFile>(&content)
                {
                    self.ui_state.custom_theme = Some(custom);
                    self.ui_state.preferences_draft_theme = ThemeId::Custom;
                    // Import swaps the live `custom_theme` in place — the
                    // draft comparator can't see it, so pin the dirty flag
                    // sticky until Save or a draft reseed clears it. Sticky
                    // MUST be set before the recompute below so the flag
                    // lands `true` even when every draft compares equal.
                    // (Discard & Close cannot restore the previous custom
                    // theme content; the sticky flag at least guarantees
                    // the user is asked before the dialog closes. Note the
                    // imported theme is session-only either way — nothing
                    // persists `custom_theme` to disk yet, so it is lost on
                    // restart even after Save; pre-existing gap.)
                    self.ui_state.preferences_dirty_sticky = true;
                    self.recompute_preferences_dirty();
                }
            }
            PrefMsg::DraftErcSeverity(rule, severity) => {
                let default_sev = rule.default_severity();
                if severity == default_sev {
                    self.ui_state.erc_severity_override.remove(&rule);
                } else {
                    self.ui_state.erc_severity_override.insert(rule, severity);
                }
                crate::fonts::write_erc_severity_overrides(&self.ui_state.erc_severity_override);
            }
            PrefMsg::ResetErcSeverities => {
                self.ui_state.erc_severity_override.clear();
                crate::fonts::write_erc_severity_overrides(&self.ui_state.erc_severity_override);
            }
            PrefMsg::ResetPrefsFile => {
                // Never move a healthy file aside. The user may have
                // repaired it by hand between this banner being painted
                // and the click, and renaming a good prefs.json away
                // would be #594 again, by hand.
                // Every outcome below is reported to the Messages panel as
                // well as to the dialog. The status string dies with the
                // dialog and is invisible to anyone reading a bug report;
                // a recovery action that touches the user's config files
                // has to leave a trace that outlives the window.
                match crate::fonts::check_prefs_file() {
                    Ok(()) => {
                        self.ui_state.prefs_load_error = None;
                        tracing::info!(
                            target = "signex::prefs",
                            path = %crate::fonts::prefs_file_path().display(),
                            "preferences reset was requested but the file now loads cleanly, \
                             so nothing was moved"
                        );
                        self.ui_state.preferences_prefs_status =
                            "Your preferences file is readable again — nothing was moved."
                                .to_string();
                    }
                    Err(_) => match crate::fonts::move_prefs_file_aside() {
                        Ok(Some(aside)) => {
                            self.ui_state.prefs_load_error = None;
                            tracing::info!(
                                target = "signex::prefs",
                                path = %crate::fonts::prefs_file_path().display(),
                                kept_as = %aside.display(),
                                "the unreadable preferences file was moved aside on the user's \
                                 request and a fresh empty one put in its place; preferences \
                                 save again from now on"
                            );
                            // This arm deliberately writes no drafts, so
                            // the fresh file is empty and anything the
                            // user "saved" while writes were being refused
                            // is still only in memory. Saying "Preferences
                            // reset." alone would let them quit believing
                            // it persisted. Re-persisting live state from
                            // here is a different feature and a different
                            // decision; telling the truth is not.
                            self.ui_state.preferences_prefs_status = format!(
                                "Preferences reset. The unreadable file was kept as {}. \
                                 Change and save a setting to write a new one.",
                                aside.display()
                            );
                        }
                        Ok(None) => {
                            self.ui_state.prefs_load_error = None;
                            // `warn!`, not `info!`: the probe a moment ago
                            // said this file was unloadable and the rename
                            // then found nothing there, so something
                            // outside this process removed it mid-action.
                            tracing::warn!(
                                target = "signex::prefs",
                                path = %crate::fonts::prefs_file_path().display(),
                                "the unreadable preferences file was gone before it could be \
                                 moved aside; something outside this process removed it"
                            );
                            self.ui_state.preferences_prefs_status =
                                "There was no preferences file to move aside.".to_string();
                        }
                        Err(error) => {
                            // The flag deliberately stays set: nothing
                            // changed on disk, so the banner must stay up
                            // and keep offering the action.
                            tracing::error!(
                                target = "signex::prefs",
                                path = %crate::fonts::prefs_file_path().display(),
                                error = %error,
                                "the unreadable preferences file could not be moved aside, so \
                                 it was left exactly as it is and preferences still cannot be \
                                 saved"
                            );
                            self.ui_state.preferences_prefs_status = format!(
                                "Could not start a fresh preferences file: the unreadable one \
                                 could not be moved aside, so it was left untouched ({error})."
                            );
                        }
                    },
                }
                // No draft was touched, so the dirty flag is not
                // recomputed — same as `ResetErcSeverities` above.
            }
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
            PrefMsg::ComponentClassEditKey { index, key } => {
                if let Some(entry) = self
                    .ui_state
                    .preferences_draft_component_classes
                    .get_mut(index)
                {
                    entry.key = key;
                    self.recompute_preferences_dirty();
                }
            }
            PrefMsg::ComponentClassEditLabel { index, label } => {
                if let Some(entry) = self
                    .ui_state
                    .preferences_draft_component_classes
                    .get_mut(index)
                {
                    entry.label = label;
                    self.recompute_preferences_dirty();
                }
            }
            PrefMsg::ComponentClassAdd => {
                self.ui_state.preferences_draft_component_classes.push(
                    crate::fonts::ComponentClassEntry {
                        key: String::new(),
                        label: String::new(),
                    },
                );
                self.recompute_preferences_dirty();
            }
            PrefMsg::ComponentClassRemove { index } => {
                if index < self.ui_state.preferences_draft_component_classes.len() {
                    self.ui_state
                        .preferences_draft_component_classes
                        .remove(index);
                    self.recompute_preferences_dirty();
                }
            }
            PrefMsg::ComponentClassResetDefaults => {
                self.ui_state.preferences_draft_component_classes =
                    crate::fonts::default_component_classes();
                self.recompute_preferences_dirty();
            }
            // Keyboard Shortcuts pane — the arms live in the sibling
            // `keymap` module. Listed one by one rather than caught by a
            // `_ =>` so this match stays exhaustive over `PrefMsg`: a new
            // non-keymap variant is a compile error here, not a message
            // silently routed to a handler that does not own it.
            msg @ (PrefMsg::KeymapSearchChanged(..)
            | PrefMsg::KeymapProfileSelected(..)
            | PrefMsg::KeymapCreateCustomProfile
            | PrefMsg::KeymapDeleteActiveProfile
            | PrefMsg::KeymapRestoreFromBackup
            | PrefMsg::KeymapDiscardBackup
            | PrefMsg::KeymapImportProfile
            | PrefMsg::KeymapProfileLoaded(..)
            | PrefMsg::KeymapExportProfile
            | PrefMsg::KeymapExportFinished(..)
            | PrefMsg::KeymapBindingChanged { .. }
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
