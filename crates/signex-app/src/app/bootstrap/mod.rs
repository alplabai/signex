use super::*;

/// Load the 256×256 PNG bundled by `installer/build-icons.sh` into an
/// [`iced::window::Icon`]. When `has_bundled_icon` isn't set (i.e. the PNG
/// hasn't been generated yet) this returns `None` and the window opens with
/// the platform default icon.
pub(in crate::app) fn bundled_window_icon() -> Option<iced::window::Icon> {
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

// `selection_slot_from_key` moved to `app/dispatch/input.rs` with its
// only consumer when the keyboard branches left the subscription (#557).

#[cfg(test)]
mod boot_tests;
mod new;
#[cfg(test)]
pub(in crate::app) use new::first_run_tour_boot;
mod subscription;
pub(crate) use subscription::modal_overlay_id;
mod window_subscriptions;

impl Signex {
    /// Default size of the unified Export PDF / Print Preview modal.
    /// Both `view_print_preview` (in-window) and the detached-window
    /// path read from these so resizing the modal happens in one
    /// place instead of two duplicated literals.
    pub(super) const PDF_MODAL_W: f32 = 1180.0;
    pub(super) const PDF_MODAL_H: f32 = 760.0;

    pub fn title(&self, id: iced::window::Id) -> String {
        // A detached modal is its own OS window with a native title bar:
        // name it after the dialog so the taskbar / window switcher can
        // tell it from the main window.
        if let Some(super::state::WindowKind::DetachedModal(modal)) = self.ui_state.windows.get(&id)
        {
            // The dialog's own name, no application suffix: the notice
            // names itself, the rest read "Quit Signex", "Rename", ...
            if *modal == super::state::ModalId::ErrorNotice
                && let Some(notice) = self.document_state.error_notice.as_ref()
            {
                return notice.title.clone();
            }
            return self.detached_modal_title(*modal);
        }
        let version = env!("CARGO_PKG_VERSION");
        let dirty_count = self.document_state.dirty_document_count();
        if dirty_count == 0 {
            format!("Signex {version}")
        } else {
            format!("• Signex {version} — {dirty_count} unsaved")
        }
    }

    /// The interface scale every window lays out at, on top of the OS
    /// display scale (#1228). Read from the committed pref: the Preferences
    /// draft only takes effect on Save.
    pub fn scale_factor(&self, _id: iced::window::Id) -> f32 {
        self.ui_state.ui_scale
    }

    pub fn theme(&self, id: iced::window::Id) -> Option<Theme> {
        let theme = self.resolve_theme();
        // A dialog in its own window draws flat (the OS window is the frame).
        if matches!(
            self.ui_state.windows.get(&id),
            Some(super::state::WindowKind::DetachedModal(_))
        ) {
            let paper = crate::styles::ti(self.document_state.panel_ctx.tokens.paper);
            return Some(crate::styles::dialog_window_theme(&theme, paper));
        }
        Some(theme)
    }

    fn resolve_theme(&self) -> Theme {
        Self::id_to_iced_theme(self.ui_state.effective_theme_id())
    }

    /// Map a ThemeId to an iced::Theme with a properly tuned palette.
    pub(crate) fn id_to_iced_theme(id: ThemeId) -> Theme {
        match id {
            ThemeId::GraphiteLight => Theme::custom(
                "Graphite Light".to_string(),
                iced::theme::palette::Seed {
                    background: iced::Color::from_rgb8(0xF6, 0xF5, 0xF1),
                    text: iced::Color::from_rgb8(0x15, 0x17, 0x1A),
                    primary: iced::Color::from_rgb8(0xC2, 0x55, 0x2D),
                    success: iced::Color::from_rgb8(0x24, 0x61, 0x3A),
                    danger: iced::Color::from_rgb8(0xA8, 0x30, 0x1F),
                    warning: iced::Color::from_rgb8(0x8A, 0x5A, 0x10),
                },
            ),
            ThemeId::GraphiteDark => Theme::custom(
                "Graphite Dark".to_string(),
                iced::theme::palette::Seed {
                    background: iced::Color::from_rgb8(0x11, 0x12, 0x14),
                    text: iced::Color::from_rgb8(0xEC, 0xEA, 0xE5),
                    primary: iced::Color::from_rgb8(0xC2, 0x55, 0x2D),
                    success: iced::Color::from_rgb8(0x7F, 0xD1, 0xA0),
                    danger: iced::Color::from_rgb8(0xF2, 0xA0, 0x93),
                    warning: iced::Color::from_rgb8(0xE8, 0xC9, 0x7A),
                },
            ),
        }
    }
}
