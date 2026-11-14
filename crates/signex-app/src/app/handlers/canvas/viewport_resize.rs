//! A window resize carries its canvases' cameras along (#2345): a fitted view
//! is fitted again, any other view keeps its centre and zoom.
//!
//! The runtime hands the resize to the widgets on the old layout, so no
//! canvas event sees the new size before `update` does; the new canvas size
//! is worked out here and each canvas compares it with the size it was last
//! laid out at.

use iced::Size;

use super::super::super::*;

impl Signex {
    /// The main window now has `ui_state.window_size`; its canvases follow.
    pub(crate) fn follow_main_window_resize(&mut self) {
        let after = self.canvas_bounds().size();
        let interaction = &mut self.interaction_state;
        if interaction.canvas.follow_resize(after) {
            interaction.canvas.clear_bg_cache();
            interaction.canvas.clear_overlay_cache();
        }
        if interaction.pcb_canvas.follow_resize(after) {
            interaction.pcb_canvas.clear_bg_cache();
        }
    }

    /// Undocked window `id` goes from its recorded size to `size`; its
    /// canvases grow or shrink by the same amount (the docks keep their size).
    pub(crate) fn follow_undocked_window_resize(&mut self, id: iced::window::Id, size: (f32, f32)) {
        let Some(&(w0, h0)) = self.ui_state.undocked_window_sizes.get(&id) else {
            return;
        };
        let grown = |old: Size| {
            Size::new(
                (old.width + size.0 - w0).max(0.0),
                (old.height + size.1 - h0).max(0.0),
            )
        };
        let interaction = &mut self.interaction_state;
        if let Some(slot) = interaction.canvases.get_mut(&id)
            && slot.follow_resize(grown(slot.viewport_size()))
        {
            slot.clear_bg_cache();
            slot.clear_overlay_cache();
        }
        if let Some(pcb) = interaction.pcb_canvases.get_mut(&id)
            && pcb.follow_resize(grown(pcb.viewport_size()))
        {
            pcb.clear_bg_cache();
        }
    }
}
