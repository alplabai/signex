//! The OS window a dialog opens in: borderless with the Graphite title bar
//! when the app draws its own chrome, the native frame otherwise, and the
//! Signex mark as the window icon either way.

use iced::Size;

use crate::app::state::{ModalId, Signex};
use crate::app::view::dialogs::DIALOG_TITLE_BAR_H;

impl Signex {
    /// Whether a dialog window opened now draws its own title bar: on a
    /// Graphite theme while the main window draws its own chrome. Decided
    /// once per window, when it opens (the OS frame cannot change later).
    pub(crate) fn dialog_windows_borderless(&self) -> bool {
        crate::styles::is_graphite(&self.document_state.panel_ctx.tokens)
            && !self.window_controls_layout().is_native()
    }

    /// Whether the dialog window `id` was opened borderless.
    pub(crate) fn dialog_window_is_borderless(&self, id: iced::window::Id) -> bool {
        self.ui_state.borderless_dialog_windows.contains(&id)
    }

    /// The size `modal`'s window should have: its content, plus the title
    /// bar when the window draws its own.
    pub(crate) fn dialog_window_size(&self, modal: ModalId, borderless: bool) -> Size {
        let content = self.modal_window_size(modal);
        if !borderless {
            return content;
        }
        // Whole logical px, so the window scales as the content does.
        let bar = (DIALOG_TITLE_BAR_H.ceil() * self.ui_state.ui_scale).ceil();
        Size::new(content.width, content.height + bar)
    }

    /// Settings of a new window for `modal`, and whether it is borderless.
    pub(crate) fn dialog_window_settings(&self, modal: ModalId) -> (iced::window::Settings, bool) {
        let borderless = self.dialog_windows_borderless();
        let size = self.dialog_window_size(modal, borderless);
        let mut settings = crate::app::handlers::erc::modal_window_settings(modal, size);
        settings.decorations = !borderless;
        settings.icon = crate::app::bootstrap::bundled_window_icon();
        settings.position = self.new_window_position(
            size,
            crate::app::handlers::window_placement::Anchor::MonitorCenter,
            settings.position,
        );
        (settings, borderless)
    }

    /// Record that `id` holds a dialog, and whether it was opened borderless.
    pub(crate) fn note_dialog_window(&mut self, id: iced::window::Id, borderless: bool) {
        if borderless {
            self.ui_state.borderless_dialog_windows.insert(id);
        } else {
            self.ui_state.borderless_dialog_windows.remove(&id);
        }
    }

    /// Forget the frame of every dialog window that is gone.
    pub(crate) fn forget_closed_dialog_windows(&mut self) {
        let windows = &self.ui_state.windows;
        self.ui_state
            .borderless_dialog_windows
            .retain(|id| windows.contains_key(id));
    }
}

#[cfg(test)]
mod tests {
    use crate::app::state::{ModalId, Signex, WindowKind};

    fn graphite_app() -> Signex {
        let (mut app, _task) = Signex::new();
        app.document_state.panel_ctx.tokens =
            signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        app
    }

    /// The window of a dialog shows the Signex mark, whatever its frame.
    #[test]
    fn a_dialog_window_carries_the_signex_mark_as_its_icon() {
        let app = graphite_app();
        let (settings, _) = app.dialog_window_settings(ModalId::LibraryStockCopy);
        assert_eq!(
            settings.icon.is_some(),
            crate::app::bootstrap::bundled_window_icon().is_some()
        );
    }

    /// Borderless on Graphite with drawn chrome: no OS frame, and the window
    /// grows by the title bar so the body keeps all of its height.
    #[test]
    fn a_borderless_dialog_window_adds_the_title_bar_to_its_height() {
        let app = graphite_app();
        let borderless = app.dialog_windows_borderless();
        let (settings, chosen) = app.dialog_window_settings(ModalId::RenameDialog);
        assert_eq!(chosen, borderless);
        assert_eq!(settings.decorations, !borderless);
        let content = app.modal_window_size(ModalId::RenameDialog);
        let framed = app.dialog_window_size(ModalId::RenameDialog, true);
        assert_eq!(framed.width, content.width);
        assert!(framed.height > content.height);
        assert_eq!(
            app.dialog_window_size(ModalId::RenameDialog, false),
            content
        );
    }

    /// A legacy theme keeps the native frame.
    #[test]
    fn a_legacy_theme_keeps_the_native_dialog_frame() {
        let (mut app, _task) = Signex::new();
        app.document_state.panel_ctx.tokens =
            signex_types::theme::theme_tokens(signex_types::theme::ThemeId::Signex);
        assert!(!app.dialog_windows_borderless());
        let (settings, borderless) = app.dialog_window_settings(ModalId::About);
        assert!(settings.decorations && !borderless);
    }

    #[test]
    fn a_closed_dialog_window_is_forgotten() {
        let mut app = graphite_app();
        let id = iced::window::Id::unique();
        app.ui_state
            .windows
            .insert(id, WindowKind::DetachedModal(ModalId::About));
        app.note_dialog_window(id, true);
        assert!(app.dialog_window_is_borderless(id));
        app.forget_closed_dialog_windows();
        assert!(app.dialog_window_is_borderless(id));
        app.ui_state.windows.remove(&id);
        app.forget_closed_dialog_windows();
        assert!(!app.dialog_window_is_borderless(id));
    }
}
