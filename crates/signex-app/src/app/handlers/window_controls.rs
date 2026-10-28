//! Window-control placement: resolving the strip layout from state, the
//! Linux desktop probe, and the Preferences restart note.

use iced::Task;

use crate::app::{Message, Signex, WindowMsg};
use crate::chrome::controls::{
    ChromePlatform, WindowControlsLayout, effective_placement, needs_restart, resolve,
};
use crate::chrome::desktop::{DesktopButtonLayout, detect};

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
            async { tokio::task::spawn_blocking(detect).await.ok().flatten() },
            |layout| Message::Window(WindowMsg::ControlsLayoutDetected(layout)),
        )
    }

    /// The probe landed. No answer is normal on desktops that expose
    /// neither GNOME's setting nor KDE's `kwinrc`; the strip then uses its
    /// default, and the Messages panel says so.
    pub(crate) fn handle_controls_layout_detected(
        &mut self,
        layout: Option<DesktopButtonLayout>,
    ) -> Task<Message> {
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
