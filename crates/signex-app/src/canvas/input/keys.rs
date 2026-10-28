use super::super::*;

impl SchematicCanvas<'_> {
    /// Track Ctrl/Shift modifier state for multi-select.
    pub(in crate::canvas) fn update_modifiers_changed(
        &self,
        state: &mut CanvasState,
        mods: &iced::keyboard::Modifiers,
    ) -> Option<canvas::Action<Message>> {
        state.ctrl_held = mods.command();
        state.shift_held = mods.shift();
        let alt_changed = state.alt_held != mods.alt();
        state.alt_held = mods.alt();
        // Alt flips the wire behaviour of a drag in flight: redo the ghost
        // now rather than at the next cursor move.
        if alt_changed && state.move_dragging {
            self.refresh_drag_ghost(state);
            return Some(canvas::Action::request_redraw());
        }
        None
    }

    /// Escape: cancel any in-progress drag.
    pub(in crate::canvas) fn update_escape_pressed(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<Message>> {
        if state.is_floating() {
            state.clear_move();
            return Some(canvas::Action::capture());
        }
        if state.move_dragging || state.click_on_selected {
            state.move_dragging = false;
            state.click_on_selected = false;
            state.move_origin = None;
            state.move_current = None;
            return Some(canvas::Action::capture());
        }
        None
    }
}
