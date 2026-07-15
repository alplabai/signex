use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_preferences_open_requested(&mut self) -> Task<Message> {
        self.ui_state.preferences_open = true;
        self.ui_state.preferences_draft_theme = self.ui_state.theme_id;
        self.ui_state.preferences_draft_font = self.ui_state.ui_font_name.clone();
        self.ui_state.preferences_draft_power_port_style = self.ui_state.power_port_style;
        self.ui_state.preferences_draft_label_style = self.ui_state.label_style;
        self.ui_state.preferences_draft_multisheet_style = self.ui_state.multisheet_style;
        self.ui_state.preferences_draft_component_classes = self.ui_state.component_classes.clone();
        self.ui_state.preferences_draft_grid_style = self.ui_state.grid_style;
        self.ui_state.preferences_draft_pcb_gpu_render = self.ui_state.pcb_gpu_render;
        // Seed the Keyboard Shortcuts pane with a fresh working copy of
        // the live profile set, and clear any stale recorder / status.
        self.ui_state.preferences_keymap_editor =
            crate::keymap::KeymapEditorModel::new(self.ui_state.keymap_profiles.clone());
        self.ui_state.preferences_keymap_status.clear();
        self.ui_state.preferences_keymap_search.clear();
        self.ui_state.preferences_keymap_recorder = None;
        self.ui_state.preferences_dirty = false;
        self.ui_state.panel_list_open = false;
        self.interaction_state.context_menu = None;
        // Open Preferences as a separate OS window so the user can move
        // it around / drag it onto a second monitor. Same pattern as
        // Print Preview, BOM Preview, Annotate, ERC.
        self.handle_detach_modal(super::super::state::ModalId::Preferences)
    }

    pub(crate) fn handle_preferences_close_requested(&mut self) -> Task<Message> {
        self.ui_state.preferences_open = false;
        self.ui_state.preferences_dirty = false;
        self.ui_state.preferences_keymap_recorder = None;
        self.close_detached_modal(super::super::state::ModalId::Preferences)
    }

    pub(crate) fn handle_preferences_navigation_requested(
        &mut self,
        nav: crate::preferences::PrefNav,
    ) -> Task<Message> {
        self.ui_state.preferences_nav = nav;
        Task::none()
    }

    /// Revert every Preferences live-preview draft back to its saved value and
    /// repaint, so closing the dialog by ANY means — the Discard button, Esc,
    /// or the native window-close button — drops unsaved previews instead of
    /// leaving them silently active (e.g. the experimental PCB GPU render
    /// toggle would otherwise keep rendering on the GPU with the checkbox
    /// showing unchecked). Does not touch `preferences_open` or close the
    /// window; callers own that. Idempotent — safe to call more than once.
    pub(crate) fn revert_preferences_drafts(&mut self) {
        self.ui_state.preferences_draft_theme = self.ui_state.theme_id;
        self.ui_state.preferences_draft_font = self.ui_state.ui_font_name.clone();
        self.ui_state.preferences_draft_power_port_style = self.ui_state.power_port_style;
        self.ui_state.preferences_draft_label_style = self.ui_state.label_style;
        self.ui_state.preferences_draft_multisheet_style = self.ui_state.multisheet_style;
        self.ui_state.preferences_draft_component_classes =
            self.ui_state.component_classes.clone();
        self.ui_state.preferences_draft_grid_style = self.ui_state.grid_style;
        self.ui_state.preferences_draft_pcb_gpu_render = self.ui_state.pcb_gpu_render;
        // Revert the live-preview GPU flag to the saved value and repaint the
        // PCB canvas so a discarded toggle takes no effect.
        self.interaction_state.pcb_canvas.gpu_render = self.ui_state.pcb_gpu_render;
        self.interaction_state.pcb_canvas.clear_content_cache();
        self.interaction_state.pcb_canvas.clear_bg_cache();
        // Drop the keymap working copy back to the live set so unsaved rebinds
        // / new profiles are discarded.
        self.ui_state.preferences_keymap_editor =
            crate::keymap::KeymapEditorModel::new(self.ui_state.keymap_profiles.clone());
        self.ui_state.preferences_keymap_status.clear();
        self.ui_state.preferences_keymap_recorder = None;
        self.ui_state.preferences_dirty = false;
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
        self.update_canvas_theme();
        crate::render_config::set_power_port_style(self.ui_state.power_port_style);
        crate::render_config::set_label_style(self.ui_state.label_style);
        crate::render_config::set_multisheet_style(self.ui_state.multisheet_style);
        crate::render_config::set_grid_style(self.ui_state.grid_style);
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
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
                if !self.ui_state.preferences_dirty {
                    self.ui_state.preferences_open = false;
                    self.ui_state.preferences_keymap_recorder = None;
                    return self.close_detached_modal(super::super::state::ModalId::Preferences);
                }
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
                    self.ui_state.preferences_dirty = true;
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
                match crate::keymap::save_profile_set(&keymap_profiles) {
                    Ok(()) => {
                        self.ui_state.keymap_profiles = keymap_profiles;
                        self.ui_state.active_keymap =
                            self.ui_state.keymap_profiles.compile_active();
                        self.ui_state.preferences_keymap_recorder = None;
                        self.ui_state.preferences_keymap_status =
                            "Keyboard shortcuts saved.".to_string();
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not save keyboard shortcuts: {error}");
                        self.ui_state.preferences_dirty = true;
                        return Task::none();
                    }
                }
                self.ui_state.preferences_dirty = false;
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
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_differs();
            }
            PrefMsg::DraftFont(name) => {
                self.ui_state.preferences_draft_font = name;
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_differs();
            }
            PrefMsg::DraftPowerPortStyle(style) => {
                self.ui_state.preferences_draft_power_port_style = style;
                crate::render_config::set_power_port_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_differs();
            }
            PrefMsg::DraftLabelStyle(style) => {
                self.ui_state.preferences_draft_label_style = style;
                crate::render_config::set_label_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_differs();
            }
            PrefMsg::DraftMultisheetStyle(style) => {
                self.ui_state.preferences_draft_multisheet_style = style;
                crate::render_config::set_multisheet_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_differs();
            }
            PrefMsg::DraftGridStyle(style) => {
                self.ui_state.preferences_draft_grid_style = style;
                crate::render_config::set_grid_style(style);
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_differs();
            }
            PrefMsg::DraftPcbGpuRender(enabled) => {
                self.ui_state.preferences_draft_pcb_gpu_render = enabled;
                // Live preview: push the draft into the widget's effective flag
                // and repaint the PCB canvas so the toggle shows immediately.
                // Persisted only on Save; reverted on Discard.
                self.interaction_state.pcb_canvas.gpu_render = enabled;
                self.interaction_state.pcb_canvas.clear_content_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_differs();
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
                return Task::future(async move {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Export Signex Theme")
                        .add_filter("Signex Theme", &["json"])
                        .set_file_name("custom-theme.json")
                        .save_file()
                        .await;
                    if let Some(f) = picked {
                        let _ = f.write(json.as_bytes()).await;
                    }
                    Message::Noop
                });
            }
            PrefMsg::ThemeFileLoaded(content) => {
                if let Ok(custom) =
                    serde_json::from_str::<signex_types::theme::CustomThemeFile>(&content)
                {
                    self.ui_state.custom_theme = Some(custom);
                    self.ui_state.preferences_draft_theme = ThemeId::Custom;
                    self.ui_state.preferences_dirty = true;
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
                    self.ui_state.preferences_dirty = true;
                }
            }
            PrefMsg::ComponentClassEditLabel { index, label } => {
                if let Some(entry) = self
                    .ui_state
                    .preferences_draft_component_classes
                    .get_mut(index)
                {
                    entry.label = label;
                    self.ui_state.preferences_dirty = true;
                }
            }
            PrefMsg::ComponentClassAdd => {
                self.ui_state.preferences_draft_component_classes.push(
                    crate::fonts::ComponentClassEntry {
                        key: String::new(),
                        label: String::new(),
                    },
                );
                self.ui_state.preferences_dirty = true;
            }
            PrefMsg::ComponentClassRemove { index } => {
                if index < self.ui_state.preferences_draft_component_classes.len() {
                    self.ui_state
                        .preferences_draft_component_classes
                        .remove(index);
                    self.ui_state.preferences_dirty = true;
                }
            }
            PrefMsg::ComponentClassResetDefaults => {
                self.ui_state.preferences_draft_component_classes =
                    crate::fonts::default_component_classes();
                self.ui_state.preferences_dirty = true;
            }
            PrefMsg::KeymapSearchChanged(query) => {
                // Pure view filter — does not touch the editor model or the
                // dirty flag, so opening/searching never marks unsaved work.
                self.ui_state.preferences_keymap_search = query;
            }
            PrefMsg::KeymapProfileSelected(id) => {
                match self
                    .ui_state
                    .preferences_keymap_editor
                    .set_active_profile(id)
                {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status.clear();
                        self.ui_state.preferences_dirty = true;
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not switch profile: {error}");
                    }
                }
            }
            PrefMsg::KeymapCreateCustomProfile => {
                // Pick the first free `custom-N` id so repeated forks
                // don't collide with an existing draft.
                let profiles = self.ui_state.preferences_keymap_editor.profiles();
                let next_index = profiles
                    .iter()
                    .filter(|profile| profile.kind == crate::keymap::ShortcutProfileKind::Custom)
                    .count()
                    + 1;
                let mut candidate = next_index;
                loop {
                    let id = format!("custom-{candidate}");
                    if profiles.iter().all(|profile| profile.id != id) {
                        let name = format!("Custom {candidate}");
                        match self
                            .ui_state
                            .preferences_keymap_editor
                            .create_custom_from_active(id, name)
                        {
                            Ok(()) => {
                                self.ui_state.preferences_keymap_status =
                                    "Created a custom profile draft.".to_string();
                                self.ui_state.preferences_dirty = true;
                            }
                            Err(error) => {
                                self.ui_state.preferences_keymap_status =
                                    format!("Could not create profile: {error}");
                            }
                        }
                        break;
                    }
                    candidate += 1;
                }
            }
            PrefMsg::KeymapDeleteActiveProfile => {
                let active_id = self
                    .ui_state
                    .preferences_keymap_editor
                    .profiles()
                    .into_iter()
                    .find(|profile| profile.active)
                    .map(|profile| profile.id);
                if let Some(id) = active_id {
                    match self
                        .ui_state
                        .preferences_keymap_editor
                        .delete_custom_profile(&id)
                    {
                        Ok(()) => {
                            self.ui_state.preferences_keymap_status =
                                "Deleted the custom profile draft.".to_string();
                            self.ui_state.preferences_dirty = true;
                        }
                        Err(error) => {
                            self.ui_state.preferences_keymap_status =
                                format!("Could not delete profile: {error}");
                        }
                    }
                }
            }
            PrefMsg::KeymapImportProfile => {
                return Task::future(async {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Import Signex Keyboard Shortcuts")
                        .add_filter("Signex Keyboard Shortcuts", &["toml"])
                        .pick_file()
                        .await;
                    if let Some(f) = picked {
                        let bytes = f.read().await;
                        let source = String::from_utf8_lossy(&bytes).to_string();
                        Message::Preferences(PreferencesMsg::Inner(PrefMsg::KeymapProfileLoaded(
                            source,
                        )))
                    } else {
                        Message::Noop
                    }
                });
            }
            PrefMsg::KeymapProfileLoaded(source) => {
                match crate::keymap::import_custom_profile(&source).and_then(|profile| {
                    self.ui_state
                        .preferences_keymap_editor
                        .insert_custom_profile(profile)
                }) {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status =
                            "Imported a custom keyboard shortcut profile.".to_string();
                        self.ui_state.preferences_dirty = true;
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not import keyboard shortcuts: {error}");
                    }
                }
            }
            PrefMsg::KeymapExportProfile => {
                let profile = self
                    .ui_state
                    .preferences_keymap_editor
                    .active_profile()
                    .clone();
                match crate::keymap::export_custom_profile(&profile) {
                    Ok(source) => {
                        let filename = format!("{}.toml", profile.id);
                        return Task::future(async move {
                            let picked = rfd::AsyncFileDialog::new()
                                .set_title("Export Signex Keyboard Shortcuts")
                                .add_filter("Signex Keyboard Shortcuts", &["toml"])
                                .set_file_name(&filename)
                                .save_file()
                                .await;
                            if let Some(f) = picked {
                                let _ = f.write(source.as_bytes()).await;
                            }
                            Message::Noop
                        });
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not export keyboard shortcuts: {error}");
                    }
                }
            }
            PrefMsg::KeymapBindingChanged {
                command,
                context,
                trigger,
            } => {
                match self.ui_state.preferences_keymap_editor.edit_active_trigger(
                    command,
                    context,
                    trigger,
                ) {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status.clear();
                        self.ui_state.preferences_dirty = true;
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Invalid shortcut: {error}");
                        self.ui_state.preferences_dirty = true;
                    }
                }
            }
            PrefMsg::KeymapRecorderOpen {
                command,
                label,
                context,
                trigger,
            } => {
                self.ui_state.preferences_keymap_recorder = Some(
                    crate::app::KeymapRecorderState::new(command, label, context, trigger),
                );
                self.ui_state.preferences_keymap_status.clear();
            }
            PrefMsg::KeymapRecorderCancel => {
                self.ui_state.preferences_keymap_recorder = None;
            }
            PrefMsg::KeymapRecorderStart => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder {
                    recorder.recording = true;
                    recorder.strokes.clear();
                    recorder.modifiers = crate::keymap::Modifiers::default();
                }
            }
            PrefMsg::KeymapRecorderStop => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder {
                    recorder.recording = false;
                    recorder.modifiers = crate::keymap::Modifiers::default();
                }
            }
            PrefMsg::KeymapRecorderClear => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder {
                    recorder.strokes.clear();
                    recorder.modifiers = crate::keymap::Modifiers::default();
                    recorder.recording = true;
                }
            }
            PrefMsg::KeymapRecorderModifiersChanged(modifiers) => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder
                    && recorder.recording
                {
                    recorder.modifiers = modifiers;
                }
            }
            PrefMsg::KeymapRecorderKeyPressed(stroke) => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder
                    && recorder.recording
                {
                    // Wrap back to a single stroke once the chord is
                    // full so a fourth press starts a fresh capture.
                    if recorder.strokes.len() >= crate::app::KeymapRecorderState::MAX_STROKES {
                        recorder.strokes.clear();
                    }
                    recorder.strokes.push(stroke);
                    recorder.modifiers = crate::keymap::Modifiers::default();
                }
            }
            PrefMsg::KeymapRecorderApply => {
                let Some(recorder) = self.ui_state.preferences_keymap_recorder.clone() else {
                    return Task::none();
                };
                if recorder.strokes.is_empty() {
                    self.ui_state.preferences_keymap_status =
                        "Record at least one keystroke before applying.".to_string();
                    return Task::none();
                }
                let trigger = recorder.trigger_text();
                match self.ui_state.preferences_keymap_editor.edit_active_trigger(
                    recorder.command,
                    recorder.context,
                    trigger,
                ) {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status.clear();
                        self.ui_state.preferences_dirty = true;
                        self.ui_state.preferences_keymap_recorder = None;
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Invalid shortcut: {error}");
                        self.ui_state.preferences_dirty = true;
                    }
                }
            }
        }

        Task::none()
    }
}
