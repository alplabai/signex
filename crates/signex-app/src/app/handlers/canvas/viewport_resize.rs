//! A window resize or a dock-splitter drag carries its canvases' cameras
//! along (#2345): a fitted view is fitted again, any other view keeps its
//! centre and zoom.
//!
//! The runtime hands the resize to the widgets on the old layout, so no
//! canvas event sees the new size before `update` does. `update` works out
//! how much the canvas region grew and adds that to the size each canvas
//! was last laid out at; the next canvas event records the real size.
//!
//! A minimised window reports 0x0. A delta to or from an empty size is not a
//! resize of the laid-out canvas, so it is skipped: the cameras and their
//! recorded viewports stay as they were, and a restore finds them unchanged.

use iced::Size;

use super::super::super::*;

impl Signex {
    /// The main-window canvas region was `before` and is now
    /// `canvas_bounds()` (a window resize or a dock drag); its canvases
    /// grow or shrink by the difference.
    pub(crate) fn follow_main_canvas_resize(&mut self, before: iced::Rectangle) {
        let after = self.canvas_bounds();
        if !has_area(before.size()) || !has_area(after.size()) {
            return;
        }
        let delta = (after.width - before.width, after.height - before.height);
        if delta == (0.0, 0.0) {
            return;
        }
        let interaction = &mut self.interaction_state;
        follow_schematic(&mut interaction.canvas, delta);
        follow_board(&mut interaction.pcb_canvas, delta);
    }

    /// Undocked window `id` goes from its recorded size to `size`; its
    /// canvases grow or shrink by the same amount (the docks keep their size).
    /// A window with no recorded size yet, or an empty size on either side,
    /// leaves its cameras alone.
    pub(crate) fn follow_undocked_window_resize(&mut self, id: iced::window::Id, size: (f32, f32)) {
        let Some(&(w0, h0)) = self.ui_state.undocked_window_sizes.get(&id) else {
            return;
        };
        if !has_area(Size::new(w0, h0)) || !has_area(Size::new(size.0, size.1)) {
            return;
        }
        let delta = (size.0 - w0, size.1 - h0);
        let interaction = &mut self.interaction_state;
        if let Some(slot) = interaction.canvases.get_mut(&id) {
            follow_schematic(slot, delta);
        }
        if let Some(pcb) = interaction.pcb_canvases.get_mut(&id) {
            follow_board(pcb, delta);
        }
    }
}

fn has_area(size: Size) -> bool {
    size.width > 0.0 && size.height > 0.0
}

/// `old` grown by `(dw, dh)`, never below zero.
fn grown(old: Size, (dw, dh): (f32, f32)) -> Size {
    Size::new((old.width + dw).max(0.0), (old.height + dh).max(0.0))
}

fn follow_schematic(slot: &mut crate::canvas::CanvasSlot, delta: (f32, f32)) {
    if slot.follow_resize(grown(slot.viewport_size(), delta)) {
        slot.clear_bg_cache();
        slot.clear_overlay_cache();
    }
}

fn follow_board(pcb: &mut crate::pcb_canvas::PcbCanvas, delta: (f32, f32)) {
    if pcb.follow_resize(grown(pcb.viewport_size(), delta)) {
        pcb.clear_bg_cache();
    }
}
