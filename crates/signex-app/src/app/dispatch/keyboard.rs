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
            InputMsg::FootprintModeShortcut(target) => {
                // v0.14.2 — gate on "active tab is a footprint
                // editor". When yes, route through the existing
                // FootprintSetMode dispatch (which also runs the
                // sketch dispatcher's SetMode handler so the
                // SetMode side-effects fire). Otherwise no-op so the
                // bare digits don't hijack other tabs.
                let path = self
                    .document_state
                    .tabs
                    .get(self.document_state.active_tab)
                    .and_then(|t| t.kind.as_footprint_editor())
                    .cloned();
                if let Some(path) = path {
                    self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::SetMode(target),
                            ),
                        },
                    ))
                } else {
                    Task::none()
                }
            }
        }
    }
}
