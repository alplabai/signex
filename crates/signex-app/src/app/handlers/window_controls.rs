//! Window-control placement: resolving the strip layout from state, the
//! Linux desktop probe, and the Preferences restart note.

use iced::Task;

use crate::app::{Message, Signex, WindowMsg};
use crate::chrome::controls::{
    ChromePlatform, WindowControlsLayout, effective_placement, needs_restart, resolve,
};
use crate::chrome::desktop::{DesktopButtonLayout, detect};

/// How long winit's fullscreen exit animation takes to finish on macOS,
/// with margin.
const FULLSCREEN_EXIT_SETTLE: std::time::Duration = std::time::Duration::from_millis(900);

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
        Task::perform(
            async {
                tokio::task::spawn_blocking(detect)
                    .await
                    .map_err(|error| error.to_string())
            },
            |layout| Message::Window(WindowMsg::ControlsLayoutDetected(layout)),
        )
    }

    /// Read the main window's mode now and, while it is stored as
    /// fullscreen, once more after the exit animation. winit only clears its
    /// fullscreen state at the end of that animation, so the queries its
    /// resize events trigger still read `Fullscreen` and no later resize
    /// arrives. The delayed task sleeps first and queries after, so the last
    /// answer is always the current mode.
    pub(crate) fn main_window_mode_task(&self, id: iced::window::Id) -> Task<Message> {
        let query = move || {
            Task::batch([
                iced::window::mode(id)
                    .map(|m| Message::Window(WindowMsg::MainWindowModeChanged(m))),
                iced::window::monitor_size(id)
                    .map(|s| Message::Window(WindowMsg::MainWindowMonitorSizeRead(s))),
            ])
        };
        if self.ui_state.main_window_mode != iced::window::Mode::Fullscreen {
            return query();
        }
        // The `async` block keeps the timer lazy: `sleep` needs a runtime
        // only once polled, not when the task is built.
        let settled = Task::future(async { tokio::time::sleep(FULLSCREEN_EXIT_SETTLE).await })
            .then(move |()| query());
        Task::batch([query(), settled])
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

    /// Only a window stored as fullscreen schedules the deferred re-query.
    #[test]
    fn a_resize_while_fullscreen_also_schedules_a_deferred_mode_query() {
        let (mut app, _task) = Signex::new();
        let id = iced::window::Id::unique();

        app.ui_state.main_window_mode = iced::window::Mode::Windowed;
        let windowed = app.main_window_mode_task(id).units();
        app.ui_state.main_window_mode = iced::window::Mode::Fullscreen;
        let fullscreen = app.main_window_mode_task(id).units();

        assert!(fullscreen > windowed, "{fullscreen} vs {windowed}");
    }

    #[test]
    fn a_failed_probe_clears_the_detected_layout() {
        let (mut app, _task) = Signex::new();
        app.ui_state.desktop_button_layout = Some(DesktopButtonLayout::default());

        let _ = app.handle_controls_layout_detected(Err("probe panicked".into()));

        assert_eq!(app.ui_state.desktop_button_layout, None);
    }
}
