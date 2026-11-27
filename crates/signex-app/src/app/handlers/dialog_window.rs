//! The OS window a dialog opens in: borderless with the Graphite title bar
//! when the app draws its own chrome, the native frame otherwise, and the
//! Signex mark as the window icon either way.

use iced::Size;

use crate::app::state::{ModalId, Signex};
use crate::app::view::dialogs::DIALOG_TITLE_BAR_H;

/// Room a dialog window leaves on each side of its monitor, for the OS
/// taskbar, dock or menu bar: the app knows the monitor's size, not its
/// work area.
const MONITOR_RESERVE: f32 = 48.0;

/// `size`, no larger than `monitor` less [`MONITOR_RESERVE`] on each side;
/// `size` itself while the monitor is unknown. A clamped dialog scrolls its
/// body.
pub(crate) fn fit_to_monitor(size: Size, monitor: Option<Size>) -> Size {
    let Some(monitor) = monitor.filter(|m| m.width.is_finite() && m.height.is_finite()) else {
        return size;
    };
    let cap = |want: f32, room: f32| want.min((room - 2.0 * MONITOR_RESERVE).floor().max(1.0));
    Size::new(
        cap(size.width, monitor.width),
        cap(size.height, monitor.height),
    )
}

impl Signex {
    /// Whether a dialog window opened now draws its own title bar: while the
    /// main window draws its own chrome. Decided once per window, when it
    /// opens (the OS frame cannot change later).
    pub(crate) fn dialog_windows_borderless(&self) -> bool {
        !self.window_controls_layout().is_native()
    }

    /// Whether the dialog window `id` was opened borderless.
    pub(crate) fn dialog_window_is_borderless(&self, id: iced::window::Id) -> bool {
        self.ui_state.borderless_dialog_windows.contains(&id)
    }

    /// The size `modal`'s window should have: its content, plus the title
    /// bar when the window draws its own, fitted to the monitor
    /// ([`fit_to_monitor`]).
    #[cfg(test)]
    pub(crate) fn dialog_window_size(&self, modal: ModalId, borderless: bool) -> Size {
        self.dialog_window_size_of(modal, None, borderless)
    }

    /// [`Self::dialog_window_size`] for the window of the browser at `owner`.
    pub(crate) fn dialog_window_size_of(
        &self,
        modal: ModalId,
        owner: Option<&std::path::Path>,
        borderless: bool,
    ) -> Size {
        let content = self.modal_window_size_of(modal, owner);
        // Whole logical px, so the window scales as the content does.
        let bar = if borderless {
            (DIALOG_TITLE_BAR_H.ceil() * self.ui_state.ui_scale).ceil()
        } else {
            0.0
        };
        let window = Size::new(content.width, content.height + bar);
        fit_to_monitor(window, self.new_window_monitor_size())
    }

    /// Settings of a new window for `modal`, and whether it is borderless.
    pub(crate) fn dialog_window_settings(&self, modal: ModalId) -> (iced::window::Settings, bool) {
        self.dialog_window_settings_of(modal, None)
    }

    /// [`Self::dialog_window_settings`] for the window of the browser at `owner`.
    pub(crate) fn dialog_window_settings_of(
        &self,
        modal: ModalId,
        owner: Option<&std::path::Path>,
    ) -> (iced::window::Settings, bool) {
        let borderless = self.dialog_windows_borderless();
        let size = self.dialog_window_size_of(modal, owner, borderless);
        let mut settings = crate::app::handlers::erc::modal_window_settings(modal, size);
        settings.decorations = !borderless;
        // Same scale and title-bar allowance as the window size itself.
        let bar = if borderless {
            (DIALOG_TITLE_BAR_H.ceil() * self.ui_state.ui_scale).ceil()
        } else {
            0.0
        };
        // Never above the size it opens at, which a small monitor caps.
        settings.min_size = settings.min_size.map(|m| {
            Size::new(
                ((m.width * self.ui_state.ui_scale).ceil()).min(size.width),
                ((m.height * self.ui_state.ui_scale).ceil() + bar).min(size.height),
            )
        });
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
        app.document_view.panel_ctx.tokens =
            signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        app
    }

    /// The New Project window's settings on a monitor of `monitor`, with
    /// the size it opens at agreeing with the size it is kept at.
    fn new_project_settings(monitor: Option<iced::Size>) -> iced::window::Settings {
        let mut app = graphite_app();
        app.ui_state.main_window_monitor_size = monitor;
        let borderless = app.dialog_windows_borderless();
        let (settings, _) = app.dialog_window_settings(ModalId::NewProject);
        let kept = app.dialog_window_size(ModalId::NewProject, borderless);
        assert_eq!(settings.size, kept, "opened and kept sizes agree");
        settings
    }

    /// Decision 2026-11-24: on a small screen the New Project window opens
    /// no larger than the screen, and can still be dragged to that size.
    #[test]
    fn the_new_project_window_fits_a_small_screen() {
        for (w, h) in [(1366.0, 768.0), (1280.0, 720.0)] {
            let settings = new_project_settings(Some(iced::Size::new(w, h)));
            let size = settings.size;
            assert!(size.width <= w && size.height <= h, "{size:?} on {w}x{h}");
            assert!(size.height < 920.0, "clamped below the mockup height");
            let min = settings.min_size.expect("a min size");
            let fits = min.width <= size.width && min.height <= size.height;
            assert!(fits, "{min:?}");
        }
    }

    /// On a screen with room, the New Project window opens at the mockup
    /// size (mockup row 21), as with no monitor known.
    #[test]
    fn on_a_large_screen_the_new_project_window_opens_at_the_mockup_size() {
        let large = new_project_settings(Some(iced::Size::new(2560.0, 1440.0)));
        let unknown = new_project_settings(None);
        assert_eq!(large.size, unknown.size);
        let app = graphite_app();
        let (w, _) = crate::app::view::dialogs::NEW_PROJECT_SIZE;
        assert_eq!(large.size.width, (w * app.ui_state.ui_scale).ceil());
        let content = app.modal_window_size(ModalId::NewProject);
        assert!(large.size.height >= content.height, "{large:?}");
    }

    #[test]
    fn fit_to_monitor_caps_each_side_and_leaves_an_unknown_monitor_alone() {
        let want = iced::Size::new(1040.0, 920.0);
        assert_eq!(super::fit_to_monitor(want, None), want);
        let fitted = super::fit_to_monitor(want, Some(iced::Size::new(1366.0, 768.0)));
        assert_eq!(fitted.width, 1040.0);
        assert_eq!(fitted.height, 768.0 - 2.0 * super::MONITOR_RESERVE);
        let nan = Some(iced::Size::new(f32::NAN, 768.0));
        assert_eq!(super::fit_to_monitor(want, nan), want);
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

    /// A dialog keeps the native frame exactly when the main window does.
    #[test]
    fn a_dialog_window_has_the_native_frame_when_the_main_window_has_it() {
        let app = graphite_app();
        let native = app.window_controls_layout().is_native();
        assert_eq!(app.dialog_windows_borderless(), !native);
        let (settings, borderless) = app.dialog_window_settings(ModalId::About);
        assert_eq!(settings.decorations, native);
        assert_eq!(borderless, !native);
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

    /// A Graphite app whose About dialog is open in its own window, opened
    /// borderless whatever the platform's window controls are.
    fn about_in_a_borderless_window() -> (Signex, iced::window::Id) {
        let mut app = graphite_app();
        app.ui_state.main_window_id = Some(iced::window::Id::unique());
        app.ui_state.main_window_opened = true;
        app.ui_state.first_run_tour_open = false;
        app.ui_state.about_open = true;
        let _ = app.sync_detached_modals();
        let id = app.detached_window_of(ModalId::About).expect("window");
        app.note_dialog_window(id, true);
        (app, id)
    }

    /// The title bar sits on top of the window: the dialog's title is in
    /// it and every line of the body is laid out below it.
    #[test]
    fn a_borderless_dialog_window_draws_its_title_bar_above_the_body() {
        let (mut app, id) = about_in_a_borderless_window();
        let size = app.dialog_window_size(ModalId::About, true);
        let bar = crate::app::view::dialogs::DIALOG_TITLE_BAR_H;
        let close = "\u{00D7}";
        let texts = crate::layout_probe::lay_out_texts(app.view(id), size);
        let title = texts
            .iter()
            .find(|t| t.text == "About Signex")
            .expect("the title in the bar");
        assert!(title.bounds.y + title.bounds.height <= bar, "{title:?}");
        assert!(texts.iter().any(|t| t.text == close), "the bar's close");
        let body: Vec<_> = texts
            .iter()
            .filter(|t| t.text != "About Signex" && t.text != close)
            .collect();
        assert!(!body.is_empty());
        for t in body {
            assert!(t.bounds.y >= bar, "{t:?} is under the title bar");
        }
        // A window with the OS frame draws no bar of its own.
        app.note_dialog_window(id, false);
        let size = app.dialog_window_size(ModalId::About, false);
        let texts = crate::layout_probe::lay_out_texts(app.view(id), size);
        assert!(texts.iter().all(|t| t.text != "About Signex"));
    }

    /// The bar's close is the OS close: it runs the dialog's own close.
    #[test]
    fn the_title_bar_close_runs_the_dialogs_own_close() {
        let (mut app, id) = about_in_a_borderless_window();
        let _ = app.update(crate::app::Message::Window(
            crate::app::WindowMsg::DialogCloseClicked(id),
        ));
        assert!(!app.ui_state.about_open, "the dialog closed");
        assert!(app.detached_window_of(ModalId::About).is_none());
    }

    /// Pressing the bar asks the OS to move that window; an edge, to resize it.
    #[test]
    fn pressing_the_title_bar_or_an_edge_starts_a_window_move_or_resize() {
        let (mut app, id) = about_in_a_borderless_window();
        let task = app.update(crate::app::Message::Window(
            crate::app::WindowMsg::DialogTitleBarPressed(id),
        ));
        assert!(task.units() > 0, "a window drag was requested");
        let task = app.update(crate::app::Message::Window(
            crate::app::WindowMsg::DialogEdgePressed(id, iced::window::Direction::East),
        ));
        assert!(task.units() > 0, "a window resize was requested");
    }
}
