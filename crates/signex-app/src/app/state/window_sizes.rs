//! Per-window size of the undocked tab windows (#1910): overlays that clamp
//! to the window they paint in read it, not the main window's size.

use super::{UiState, WindowKind};

impl UiState {
    /// The size of `window`: its own for an undocked tab window that has
    /// reported one, the main window's otherwise.
    #[must_use]
    pub fn size_of_window(&self, window: iced::window::Id) -> (f32, f32) {
        self.undocked_window_sizes
            .get(&window)
            .copied()
            .unwrap_or(self.window_size)
    }

    /// Record an undocked tab window's size. Only a tracked undocked window
    /// is stored, so a late reading after it closed brings nothing back. An
    /// empty size (a minimised window reports 0x0) keeps the last real one,
    /// which the canvas resize follow measures from (#2345).
    pub fn set_undocked_window_size(&mut self, window: iced::window::Id, size: (f32, f32)) {
        let tracked = matches!(
            self.windows.get(&window),
            Some(WindowKind::UndockedTab { .. })
        );
        if tracked && Self::is_real_window_size(size) {
            self.undocked_window_sizes.insert(window, size);
        }
    }

    /// Whether a reported window size is a real one. A minimised window
    /// reports 0x0; no window's size is stored from such a reading.
    #[must_use]
    pub fn is_real_window_size((w, h): (f32, f32)) -> bool {
        w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0
    }

    /// Forget a closed or reattached window's size.
    pub fn forget_undocked_window_size(&mut self, window: iced::window::Id) {
        self.undocked_window_sizes.remove(&window);
    }
}
