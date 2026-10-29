//! Router for the keyboard-input message family ([`InputMsg`]).

use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn dispatch_input_message(&mut self, message: InputMsg) -> Task<Message> {
        match message {
            // Esc means different things in different OS windows, so its
            // whole resolution lives in `dispatch/escape.rs` (#554).
            InputMsg::EscapePressed { window } => self.handle_escape_pressed(window),
            // Every other key the widgets didn't want. Which consumer
            // owns it is decided in `dispatch/input.rs` against live
            // state (#557 phase 1) — the subscription no longer decides
            // anything.
            InputMsg::KeyInput { window, event } => self.handle_key_input(window, event),
            InputMsg::FootprintModeShortcut(target) => self.update_active_footprint_editor(
                crate::library::messages::FootprintEditorMsg::SetMode(target),
            ),
            InputMsg::FootprintCourtyardOutlineRequested => self.update_active_footprint_editor(
                crate::library::messages::FootprintEditorMsg::RecomputeCourtyardOutline,
            ),
        }
    }

    /// Send `msg` to the footprint editor on the active tab; a no-op on any
    /// other tab, so a bare digit key never hijacks them. A mode switch
    /// goes through the same dispatch as the toolbar, so the sketch
    /// dispatcher's `SetMode` side effects fire.
    fn update_active_footprint_editor(
        &mut self,
        msg: crate::library::messages::FootprintEditorMsg,
    ) -> Task<Message> {
        let path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.kind.as_footprint_editor())
            .cloned();
        let Some(path) = path else {
            return Task::none();
        };
        self.update(Message::Library(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: crate::library::messages::PrimitiveEdit::Footprint(msg),
            },
        ))
    }
}
