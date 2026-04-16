use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_preferences_open_requested(&mut self) -> Task<Message> {
        self.ui_state.preferences_open = true;
        self.ui_state.preferences_draft_theme = self.ui_state.theme_id;
        self.ui_state.preferences_draft_font = self.ui_state.ui_font_name.clone();
        self.ui_state.preferences_draft_power_port_style = self.ui_state.power_port_style;
        self.ui_state.preferences_dirty = false;
        self.ui_state.panel_list_open = false;
        Task::none()
    }

    pub(crate) fn handle_preferences_close_requested(&mut self) -> Task<Message> {
        self.ui_state.preferences_open = false;
        self.ui_state.preferences_dirty = false;
        Task::none()
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
                }
            }
            PrefMsg::DiscardAndClose => {
                self.ui_state.preferences_draft_theme = self.ui_state.theme_id;
                self.ui_state.preferences_draft_font = self.ui_state.ui_font_name.clone();
                self.ui_state.preferences_draft_power_port_style = self.ui_state.power_port_style;
                self.ui_state.preferences_dirty = false;
                self.ui_state.preferences_open = false;
                let tokens = if self.ui_state.theme_id == ThemeId::Custom {
                    self.ui_state.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(self.ui_state.theme_id)
                };
                self.document_state.panel_ctx.tokens = tokens;
                self.update_canvas_theme();
                signex_render::set_power_port_style(self.ui_state.power_port_style);
                self.interaction_state.canvas.clear_content_cache();
            }
            PrefMsg::Save => {
                self.ui_state.theme_id = self.ui_state.preferences_draft_theme;
                self.ui_state.ui_font_name = self.ui_state.preferences_draft_font.clone();
                self.ui_state.power_port_style = self.ui_state.preferences_draft_power_port_style;
                self.update_canvas_theme();
                let tokens = if self.ui_state.theme_id == ThemeId::Custom {
                    self.ui_state.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(self.ui_state.theme_id)
                };
                self.document_state.panel_ctx.tokens = tokens;
                self.document_state.panel_ctx.ui_font_name = self.ui_state.ui_font_name.clone();
                signex_render::set_power_port_style(self.ui_state.power_port_style);
                crate::fonts::write_ui_font_pref(&self.ui_state.ui_font_name);
                crate::fonts::write_power_port_style_pref(self.ui_state.power_port_style);
                self.ui_state.preferences_dirty = false;
            }
            PrefMsg::DraftTheme(id) => {
                self.ui_state.preferences_draft_theme = id;
                let tokens = if id == ThemeId::Custom {
                    self.ui_state.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(id)
                };
                self.document_state.panel_ctx.tokens = tokens;
                let canvas_colors = if id == ThemeId::Custom {
                    self.ui_state.custom_theme
                        .as_ref()
                        .map(|c| c.canvas)
                        .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
                } else {
                    signex_types::theme::canvas_colors(id)
                };
                self.interaction_state.canvas.set_theme_colors(
                    signex_render::colors::to_iced(&canvas_colors.background),
                    signex_render::colors::to_iced(&canvas_colors.grid),
                    signex_render::colors::to_iced(&canvas_colors.paper),
                );
                self.interaction_state.canvas.canvas_colors = canvas_colors;
                self.interaction_state.canvas.clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style != self.ui_state.power_port_style;
            }
            PrefMsg::DraftFont(name) => {
                self.ui_state.preferences_draft_font = name;
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style != self.ui_state.power_port_style;
            }
            PrefMsg::DraftPowerPortStyle(style) => {
                self.ui_state.preferences_draft_power_port_style = style;
                signex_render::set_power_port_style(style);
                self.interaction_state.canvas.clear_content_cache();
                self.ui_state.preferences_dirty = self.ui_state.preferences_draft_theme != self.ui_state.theme_id
                    || self.ui_state.preferences_draft_font != self.ui_state.ui_font_name
                    || self.ui_state.preferences_draft_power_port_style != self.ui_state.power_port_style;
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
                        Message::PreferencesMsg(PrefMsg::ThemeFileLoaded(s))
                    } else {
                        Message::Noop
                    }
                });
            }
            PrefMsg::ExportTheme => {
                let id = self.ui_state.preferences_draft_theme;
                let name = if id == ThemeId::Custom {
                    self.ui_state.custom_theme
                        .as_ref()
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "Custom".to_string())
                } else {
                    id.label().to_string()
                };
                let tokens = if id == ThemeId::Custom {
                    self.ui_state.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(id)
                };
                let canvas = if id == ThemeId::Custom {
                    self.ui_state.custom_theme
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
        }

        Task::none()
    }
}
