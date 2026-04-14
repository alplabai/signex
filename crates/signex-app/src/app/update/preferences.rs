use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_open_preferences(&mut self) -> Task<Message> {
        self.preferences_open = true;
        self.preferences_draft_theme = self.theme_id;
        self.preferences_draft_font = self.ui_font_name.clone();
        self.preferences_draft_power_port_style = self.power_port_style;
        self.preferences_dirty = false;
        self.panel_list_open = false;
        Task::none()
    }

    pub(crate) fn handle_close_preferences(&mut self) -> Task<Message> {
        self.preferences_open = false;
        self.preferences_dirty = false;
        Task::none()
    }

    pub(crate) fn handle_preferences_nav(
        &mut self,
        nav: crate::preferences::PrefNav,
    ) -> Task<Message> {
        self.preferences_nav = nav;
        Task::none()
    }

    pub(crate) fn handle_preferences_msg(
        &mut self,
        msg: crate::preferences::PrefMsg,
    ) -> Task<Message> {
        use crate::preferences::PrefMsg;

        match msg {
            PrefMsg::Nav(nav) => {
                self.preferences_nav = nav;
            }
            PrefMsg::Close => {
                if !self.preferences_dirty {
                    self.preferences_open = false;
                }
            }
            PrefMsg::DiscardAndClose => {
                self.preferences_draft_theme = self.theme_id;
                self.preferences_draft_font = self.ui_font_name.clone();
                self.preferences_draft_power_port_style = self.power_port_style;
                self.preferences_dirty = false;
                self.preferences_open = false;
                let tokens = if self.theme_id == ThemeId::Custom {
                    self.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(self.theme_id)
                };
                self.panel_ctx.tokens = tokens;
                self.update_canvas_theme();
                signex_render::set_power_port_style(self.power_port_style);
                self.canvas.clear_content_cache();
            }
            PrefMsg::Save => {
                self.theme_id = self.preferences_draft_theme;
                self.ui_font_name = self.preferences_draft_font.clone();
                self.power_port_style = self.preferences_draft_power_port_style;
                self.update_canvas_theme();
                let tokens = if self.theme_id == ThemeId::Custom {
                    self.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(self.theme_id)
                };
                self.panel_ctx.tokens = tokens;
                self.panel_ctx.ui_font_name = self.ui_font_name.clone();
                signex_render::set_power_port_style(self.power_port_style);
                crate::fonts::write_ui_font_pref(&self.ui_font_name);
                crate::fonts::write_power_port_style_pref(self.power_port_style);
                self.preferences_dirty = false;
            }
            PrefMsg::DraftTheme(id) => {
                self.preferences_draft_theme = id;
                let tokens = if id == ThemeId::Custom {
                    self.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(id)
                };
                self.panel_ctx.tokens = tokens;
                let canvas_colors = if id == ThemeId::Custom {
                    self.custom_theme
                        .as_ref()
                        .map(|c| c.canvas)
                        .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
                } else {
                    signex_types::theme::canvas_colors(id)
                };
                self.canvas.set_theme_colors(
                    signex_render::colors::to_iced(&canvas_colors.background),
                    signex_render::colors::to_iced(&canvas_colors.grid),
                    signex_render::colors::to_iced(&canvas_colors.paper),
                );
                self.canvas.canvas_colors = canvas_colors;
                self.canvas.clear_content_cache();
                self.preferences_dirty = self.preferences_draft_theme != self.theme_id
                    || self.preferences_draft_font != self.ui_font_name
                    || self.preferences_draft_power_port_style != self.power_port_style;
            }
            PrefMsg::DraftFont(name) => {
                self.preferences_draft_font = name;
                self.preferences_dirty = self.preferences_draft_theme != self.theme_id
                    || self.preferences_draft_font != self.ui_font_name
                    || self.preferences_draft_power_port_style != self.power_port_style;
            }
            PrefMsg::DraftPowerPortStyle(style) => {
                self.preferences_draft_power_port_style = style;
                signex_render::set_power_port_style(style);
                self.canvas.clear_content_cache();
                self.preferences_dirty = self.preferences_draft_theme != self.theme_id
                    || self.preferences_draft_font != self.ui_font_name
                    || self.preferences_draft_power_port_style != self.power_port_style;
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
                let id = self.preferences_draft_theme;
                let name = if id == ThemeId::Custom {
                    self.custom_theme
                        .as_ref()
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "Custom".to_string())
                } else {
                    id.label().to_string()
                };
                let tokens = if id == ThemeId::Custom {
                    self.custom_theme
                        .as_ref()
                        .map(|c| c.tokens)
                        .unwrap_or_else(|| signex_types::theme::theme_tokens(ThemeId::Signex))
                } else {
                    signex_types::theme::theme_tokens(id)
                };
                let canvas = if id == ThemeId::Custom {
                    self.custom_theme
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
                    self.custom_theme = Some(custom);
                    self.preferences_draft_theme = ThemeId::Custom;
                    self.preferences_dirty = true;
                }
            }
        }

        Task::none()
    }
}
