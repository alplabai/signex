//! Window-control placement: resolving the strip layout from state, the
//! Linux desktop probe, and the Preferences restart note.

use iced::Task;

use crate::app::{Message, Signex, WindowMsg};
use crate::chrome::controls::{
    ChromePlatform, WindowControlsLayout, effective_placement, is_effectively_fullscreen,
    needs_restart, resolve,
};
use crate::chrome::desktop::{DesktopButtonLayout, DesktopFamily, current_family, detect};

/// How long winit's fullscreen exit animation takes to finish on macOS,
/// with margin.
const FULLSCREEN_EXIT_SETTLE: std::time::Duration = std::time::Duration::from_millis(900);

/// Extra deferred re-queries after the first, for a slow exit animation.
const FULLSCREEN_REQUERY_RETRIES: u8 = 3;

fn query_mode(id: iced::window::Id) -> Task<Message> {
    Task::batch([
        iced::window::mode(id).map(|m| Message::Window(WindowMsg::MainWindowModeRead(m))),
        iced::window::monitor_size(id)
            .map(|s| Message::Window(WindowMsg::MainWindowMonitorSizeRead(s))),
    ])
}

/// Whether a deferred re-query that just read these values should run once
/// more: the window still looks fullscreen and retries remain.
fn should_requery_again(
    mode: iced::window::Mode,
    window_size: iced::Size,
    monitor: Option<iced::Size>,
    retries_left: u8,
) -> bool {
    retries_left > 0
        && is_effectively_fullscreen(mode, (window_size.width, window_size.height), monitor)
}

/// Sleep out winit's exit animation, read mode, monitor and size, publish
/// the first two, and chain another round while the window still reads as
/// fullscreen. A `Task` chain, so `update` never sleeps.
fn deferred_requery(id: iced::window::Id, retries_left: u8) -> Task<Message> {
    Task::future(async { tokio::time::sleep(FULLSCREEN_EXIT_SETTLE).await }).then(move |()| {
        iced::window::mode(id).then(move |mode| {
            iced::window::monitor_size(id).then(move |monitor| {
                iced::window::size(id).then(move |size| {
                    let mut steps = vec![
                        Task::done(Message::Window(WindowMsg::MainWindowModeRead(mode))),
                        Task::done(Message::Window(WindowMsg::MainWindowMonitorSizeRead(
                            monitor,
                        ))),
                    ];
                    if should_requery_again(mode, size, monitor, retries_left) {
                        steps.push(deferred_requery(id, retries_left - 1));
                    }
                    Task::batch(steps)
                })
            })
        })
    })
}

impl Signex {
    /// The layout the strip draws right now. Pure over `ui_state`.
    pub(crate) fn window_controls_layout(&self) -> WindowControlsLayout {
        let ui = &self.ui_state;
        let platform = ChromePlatform::current();
        let placement = effective_placement(
            platform,
            ui.window_controls_placement,
            ui.window_controls_placement_at_boot,
        );
        resolve(platform, placement, ui.desktop_button_layout.as_ref())
    }

    /// Whether the drafted placement only takes effect after a restart.
    pub(crate) fn window_controls_needs_restart(&self) -> bool {
        let ui = &self.ui_state;
        needs_restart(
            ChromePlatform::current(),
            ui.window_controls_placement_at_boot,
            ui.preferences_draft_window_controls_placement,
        )
    }

    /// Startup probe for the desktop's button layout. Linux only; the probe
    /// spawns `gsettings` and reads a file, so it runs off the update thread.
    pub(crate) fn detect_window_controls_task() -> Task<Message> {
        if ChromePlatform::current() != ChromePlatform::Linux {
            return Task::none();
        }
        if current_family() == DesktopFamily::Other {
            crate::diagnostics::log_info(
                "Window controls: this desktop has no known button-layout source; \
                 using minimize, maximize and close on the right",
            );
            return Task::none();
        }
        Task::perform(
            async {
                tokio::task::spawn_blocking(detect)
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|probe| {
                        probe.map_err(|_| "gsettings timed out and was killed".to_string())
                    })
            },
            |layout| Message::Window(WindowMsg::ControlsLayoutDetected(layout)),
        )
    }

    /// Read the main window's mode now and, while it is stored as
    /// fullscreen, again after the exit animation (see [`deferred_requery`]).
    /// Only the native macOS style reads the result (it hides the inset in
    /// fullscreen); every other style skips the queries.
    pub(crate) fn main_window_mode_task(&self, id: iced::window::Id) -> Task<Message> {
        if !self.window_controls_layout().is_native() {
            return Task::none();
        }
        if self.ui_state.main_window_mode != iced::window::Mode::Fullscreen {
            return query_mode(id);
        }
        Task::batch([
            query_mode(id),
            deferred_requery(id, FULLSCREEN_REQUERY_RETRIES),
        ])
    }

    /// The probe landed. No answer is normal on desktops that expose
    /// neither GNOME's setting nor KDE's `kwinrc`; the strip then uses its
    /// default, and the Messages panel says so.
    pub(crate) fn handle_controls_layout_detected(
        &mut self,
        probe: Result<Option<DesktopButtonLayout>, String>,
    ) -> Task<Message> {
        let layout = match probe {
            Ok(layout) => layout,
            Err(reason) => {
                crate::diagnostics::log_warning(format!(
                    "Window controls: the desktop probe failed ({reason}); \
                     using minimize, maximize and close on the right"
                ));
                self.ui_state.desktop_button_layout = None;
                return Task::none();
            }
        };
        match &layout {
            Some(found) => crate::diagnostics::log_info(format!(
                "Window controls: using the desktop's layout ({} left, {} right)",
                found.leading.len(),
                found.trailing.len()
            )),
            None => crate::diagnostics::log_info(
                "Window controls: the desktop's button layout could not be read; \
                 using minimize, maximize and close on the right",
            ),
        }
        self.ui_state.desktop_button_layout = layout;
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::controls::WindowControlsPlacement;

    /// A saved placement is live at once on Windows and Linux; on macOS the
    /// window keeps the style it was created with until a restart.
    #[test]
    fn a_saved_placement_changes_the_layout_except_on_macos() {
        let (mut app, _task) = Signex::new();
        app.ui_state.window_controls_placement_at_boot = WindowControlsPlacement::Auto;
        app.ui_state.window_controls_placement = WindowControlsPlacement::Leading;

        let layout = app.window_controls_layout();

        if ChromePlatform::current() == ChromePlatform::MacOs {
            assert!(layout.is_native(), "macOS keeps its boot-time style");
        } else {
            assert!(!layout.leading.is_empty());
            assert!(layout.trailing.is_empty());
        }
    }

    /// Only a native-style window stored as fullscreen schedules the
    /// deferred re-query; a drawn or non-mac style runs no queries at all.
    #[test]
    fn mode_queries_run_only_for_the_native_style() {
        use crate::chrome::controls::WindowControlsPlacement::{Auto, Trailing};
        let (mut app, _task) = Signex::new();
        let id = iced::window::Id::unique();

        app.ui_state.window_controls_placement_at_boot = Trailing;
        app.ui_state.window_controls_placement = Trailing;
        assert_eq!(app.main_window_mode_task(id).units(), 0);

        app.ui_state.window_controls_placement_at_boot = Auto;
        app.ui_state.window_controls_placement = Auto;
        if app.window_controls_layout().is_native() {
            app.ui_state.main_window_mode = iced::window::Mode::Windowed;
            let windowed = app.main_window_mode_task(id).units();
            app.ui_state.main_window_mode = iced::window::Mode::Fullscreen;
            let fullscreen = app.main_window_mode_task(id).units();
            assert!(windowed > 0 && fullscreen > windowed);
        }
    }

    #[test]
    fn requery_repeats_while_fullscreen_and_stops_at_the_bound() {
        use iced::Size;
        use iced::window::Mode::{Fullscreen, Windowed};
        let monitor = Some(Size::new(1440.0, 900.0));
        let full = Size::new(1440.0, 860.0);
        let narrow = Size::new(1000.0, 700.0);

        assert!(should_requery_again(Fullscreen, full, monitor, 2));
        assert!(should_requery_again(Fullscreen, full, None, 1));
        assert!(!should_requery_again(Fullscreen, full, monitor, 0));
        assert!(!should_requery_again(Fullscreen, narrow, monitor, 2));
        assert!(!should_requery_again(Windowed, full, monitor, 2));
    }

    #[test]
    fn a_failed_probe_clears_the_detected_layout() {
        let (mut app, _task) = Signex::new();
        app.ui_state.desktop_button_layout = Some(DesktopButtonLayout::default());

        let _ = app.handle_controls_layout_detected(Err("probe panicked".into()));

        assert_eq!(app.ui_state.desktop_button_layout, None);
    }
}
