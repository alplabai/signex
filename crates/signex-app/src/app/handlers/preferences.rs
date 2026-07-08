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
        self.close_detached_modal(super::super::state::ModalId::Preferences)
    }

    pub(crate) fn handle_preferences_navigation_requested(
        &mut self,
        nav: crate::preferences::PrefNav,
    ) -> Task<Message> {
        self.ui_state.preferences_nav = nav;
        Task::none()
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
                    return self.close_detached_modal(super::super::state::ModalId::Preferences);
                }
            }
            PrefMsg::DiscardAndClose => {
                self.ui_state.preferences_draft_theme = self.ui_state.theme_id;
                self.ui_state.preferences_draft_font = self.ui_state.ui_font_name.clone();
                self.ui_state.preferences_draft_power_port_style = self.ui_state.power_port_style;
                self.ui_state.preferences_draft_label_style = self.ui_state.label_style;
                self.ui_state.preferences_draft_multisheet_style = self.ui_state.multisheet_style;
                self.ui_state.preferences_draft_component_classes =
                    self.ui_state.component_classes.clone();
                self.ui_state.preferences_draft_grid_style = self.ui_state.grid_style;
                self.ui_state.preferences_dirty = false;
                self.ui_state.preferences_open = false;
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
                return self.close_detached_modal(super::super::state::ModalId::Preferences);
            }
            PrefMsg::Save => {
                self.ui_state.theme_id = self.ui_state.preferences_draft_theme;
                self.ui_state.ui_font_name = self.ui_state.preferences_draft_font.clone();
                self.ui_state.power_port_style = self.ui_state.preferences_draft_power_port_style;
                self.ui_state.label_style = self.ui_state.preferences_draft_label_style;
                self.ui_state.multisheet_style = self.ui_state.preferences_draft_multisheet_style;
                self.ui_state.grid_style = self.ui_state.preferences_draft_grid_style;
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
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme
                    != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style
                        != self.ui_state.power_port_style
                    || self.ui_state.preferences_draft_label_style != self.ui_state.label_style;
            }
            PrefMsg::DraftFont(name) => {
                self.ui_state.preferences_draft_font = name;
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme
                    != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style
                        != self.ui_state.power_port_style
                    || self.ui_state.preferences_draft_label_style != self.ui_state.label_style;
            }
            PrefMsg::DraftPowerPortStyle(style) => {
                self.ui_state.preferences_draft_power_port_style = style;
                crate::render_config::set_power_port_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme
                    != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style
                        != self.ui_state.power_port_style
                    || self.ui_state.preferences_draft_label_style != self.ui_state.label_style;
            }
            PrefMsg::DraftLabelStyle(style) => {
                self.ui_state.preferences_draft_label_style = style;
                crate::render_config::set_label_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme
                    != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style
                        != self.ui_state.power_port_style
                    || self.ui_state.preferences_draft_label_style != self.ui_state.label_style
                    || self.ui_state.preferences_draft_multisheet_style
                        != self.ui_state.multisheet_style;
            }
            PrefMsg::DraftMultisheetStyle(style) => {
                self.ui_state.preferences_draft_multisheet_style = style;
                crate::render_config::set_multisheet_style(style);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme
                    != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style
                        != self.ui_state.power_port_style
                    || self.ui_state.preferences_draft_label_style != self.ui_state.label_style
                    || self.ui_state.preferences_draft_multisheet_style
                        != self.ui_state.multisheet_style;
            }
            PrefMsg::DraftGridStyle(style) => {
                self.ui_state.preferences_draft_grid_style = style;
                crate::render_config::set_grid_style(style);
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme
                    != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style
                        != self.ui_state.power_port_style
                    || self.ui_state.preferences_draft_label_style != self.ui_state.label_style
                    || self.ui_state.preferences_draft_multisheet_style
                        != self.ui_state.multisheet_style
                    || self.ui_state.preferences_draft_grid_style != self.ui_state.grid_style;
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
        }

        Task::none()
    }
}
