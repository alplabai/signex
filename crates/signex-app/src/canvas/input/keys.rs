use super::super::*;

impl SchematicCanvas {
    /// Track Ctrl/Shift modifier state for multi-select.
    pub(in crate::canvas) fn update_modifiers_changed(
        &self,
        state: &mut CanvasState,
        mods: &iced::keyboard::Modifiers,
    ) -> Option<canvas::Action<Message>> {
        state.ctrl_held = mods.command();
        state.shift_held = mods.shift();
        None
    }

    /// Escape: cancel any in-progress drag.
    pub(in crate::canvas) fn update_escape_pressed(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<Message>> {
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
