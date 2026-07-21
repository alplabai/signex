use super::*;

/// Load the 256×256 PNG bundled by `installer/build-icons.sh` into an
/// [`iced::window::Icon`]. When `has_bundled_icon` isn't set (i.e. the PNG
/// hasn't been generated yet) this returns `None` and the window opens with
/// the platform default icon.
fn bundled_window_icon() -> Option<iced::window::Icon> {
    #[cfg(has_bundled_icon)]
    {
        let bytes: &[u8] = include_bytes!("../../../assets/brand/generated/signex-256.png");
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
    }
    #[cfg(not(has_bundled_icon))]
    {
        None
    }
}

fn selection_slot_from_key(key: &str) -> Option<usize> {
    match key {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        "5" => Some(4),
        "6" => Some(5),
        "7" => Some(6),
        "8" => Some(7),
        _ => None,
    }
}

mod new;
mod subscription;

impl Signex {
    pub(super) const CONTEXT_MENU_WIDTH: f32 = 248.0;
    /// Default size of the unified Export PDF / Print Preview modal.
    /// Both `view_print_preview` (in-window) and the detached-window
    /// path read from these so resizing the modal happens in one
    /// place instead of two duplicated literals.
    pub(super) const PDF_MODAL_W: f32 = 1180.0;
    pub(super) const PDF_MODAL_H: f32 = 760.0;

    pub fn title(&self, _id: iced::window::Id) -> String {
        let version = env!("CARGO_PKG_VERSION");
        let dirty_count = self.document_state.dirty_paths.len();
        if dirty_count == 0 {
            format!("Signex {version}")
        } else {
            format!("• Signex {version} — {dirty_count} unsaved")
        }
    }

    pub fn theme(&self, _id: iced::window::Id) -> Option<Theme> {
        Some(self.resolve_theme())
    }

    fn resolve_theme(&self) -> Theme {
        let id = if self.ui_state.preferences_open {
            self.ui_state.preferences_draft_theme
        } else {
            self.ui_state.theme_id
        };
        Self::id_to_iced_theme(id, self.ui_state.custom_theme.as_ref())
    }

    /// Map a ThemeId to an iced::Theme with a properly tuned palette.
    fn id_to_iced_theme(
        id: ThemeId,
        custom: Option<&signex_types::theme::CustomThemeFile>,
    ) -> Theme {
        use crate::render_config::to_iced;
        match id {
            ThemeId::Custom => {
                if let Some(c) = custom {
                    let t = &c.tokens;
                    Theme::custom(
                        c.name.clone(),
                        iced::theme::Palette {
                            background: to_iced(&t.bg),
                            text: to_iced(&t.text),
                            primary: to_iced(&t.accent),
                            success: to_iced(&t.success),
                            danger: to_iced(&t.error),
                            warning: to_iced(&t.warning),
                        },
                    )
                } else {
                    Theme::CatppuccinMocha
                }
            }
            ThemeId::CatppuccinMocha => Theme::CatppuccinMocha,
            ThemeId::VsCodeDark => Theme::custom(
                "VS Code Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.118, 0.118, 0.118),
                    text: iced::Color::from_rgb(0.831, 0.831, 0.831),
                    primary: iced::Color::from_rgb(0.000, 0.478, 0.800),
                    success: iced::Color::from_rgb(0.416, 0.600, 0.333),
                    danger: iced::Color::from_rgb(0.957, 0.267, 0.278),
                    warning: iced::Color::from_rgb(1.000, 0.549, 0.000),
                },
            ),
            ThemeId::Signex => Theme::custom(
                "Altium Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.18, 0.18, 0.19),
                    text: iced::Color::from_rgb(0.86, 0.86, 0.86),
                    primary: iced::Color::from_rgb(0.91, 0.57, 0.18),
                    success: iced::Color::from_rgb(0.34, 0.65, 0.29),
                    danger: iced::Color::from_rgb(0.96, 0.31, 0.31),
                    warning: iced::Color::from_rgb(0.91, 0.57, 0.18),
                },
            ),
            ThemeId::Alplab => Theme::custom(
                "Alp Lab".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.18, 0.18, 0.19),
                    text: iced::Color::from_rgb(0.86, 0.86, 0.86),
                    // Alp Lab cyan #0891b2 as primary accent.
                    primary: iced::Color::from_rgb(0.031, 0.569, 0.698),
                    success: iced::Color::from_rgb(0.34, 0.65, 0.29),
                    danger: iced::Color::from_rgb(0.96, 0.31, 0.31),
                    warning: iced::Color::from_rgb(0.91, 0.57, 0.18),
                },
            ),
            ThemeId::GitHubDark => Theme::custom(
                "GitHub Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.051, 0.067, 0.090),
                    text: iced::Color::from_rgb(0.902, 0.929, 0.953),
                    primary: iced::Color::from_rgb(0.345, 0.651, 1.000),
                    success: iced::Color::from_rgb(0.247, 0.725, 0.314),
                    danger: iced::Color::from_rgb(1.000, 0.482, 0.447),
                    warning: iced::Color::from_rgb(0.824, 0.604, 0.133),
                },
            ),
            ThemeId::SolarizedLight => Theme::Light,
            ThemeId::Nord => Theme::Nord,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::selection_slot_from_key;

    #[test]
    fn selection_slot_only_matches_digits_one_through_eight() {
        // Digits 1-8 map to selection-memory slots 0-7.
        for (i, key) in ["1", "2", "3", "4", "5", "6", "7", "8"].iter().enumerate() {
            assert_eq!(selection_slot_from_key(key), Some(i));
        }
        // Regression guard for issue #103: the Ctrl+1-8 / Alt+1-8
        // catch-all arms are gated on `selection_slot_from_key(c).is_some()`.
        // These letters returning None is exactly what lets the
        // Ctrl+C/X/V/D and Shift+Ctrl+V/G arms below fire instead of
        // being shadowed into a no-op.
        for key in ["c", "x", "v", "d", "g", "s", "a", "z", "0", "9"] {
            assert_eq!(
                selection_slot_from_key(key),
                None,
                "{key} must not resolve to a selection slot"
            );
        }
    }
}
