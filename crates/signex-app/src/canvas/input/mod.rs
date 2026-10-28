//! Input handling for the schematic canvas, split by concern:
//! camera (zoom/fit), pointer (buttons + motion), keys (keyboard).
//!
//! Each file adds `impl CanvasSlot` methods that
//! `canvas::Program::update` dispatches to in the original event order.

mod camera;
mod keys;
mod pointer;

use super::*;

impl SchematicCanvas<'_> {
    /// Route one input event to its handler.
    pub(in crate::canvas) fn dispatch_event(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // Remember the bounds so menu zoom (no cursor) can find the view centre.
        self.viewport.set(bounds);
        // Consume any pending fit-to-content before dispatching the event.
        if let Some(action) = self.update_pending_fit(bounds) {
            return Some(action);
        }

        // Dispatch each input event to its handler, in the original order.
        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let action = self.update_wheel_scrolled(delta, bounds, cursor);
                // A zoom changes the ghost's step (`ghost_delta`); without
                // this the drag shows the plain shift until the cursor moves.
                self.refresh_drag_ghost(state);
                action
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.update_left_pressed(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                self.update_left_released(state)
            }
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                self.update_modifiers_changed(state, mods)
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => self.update_escape_pressed(state),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                self.update_right_pressed(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                self.update_middle_pressed(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                self.update_right_released(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                self.update_middle_released(state)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let action = self.update_cursor_moved(state, bounds, cursor);
                self.refresh_drag_ghost(state);
                action
            }
            _ => None,
        }
    }
}
