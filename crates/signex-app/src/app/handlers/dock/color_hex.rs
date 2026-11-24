//! The hex field of a colour row (mockup row 46): keystrokes are a draft,
//! Enter or leaving the field runs the pick the row's popover sends, so a
//! typed colour is one undo step; refused text stays, marked; Esc drops it.

use iced::Task;

use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{ColorHexDraft, ColorHexMsg};

impl Signex {
    pub(crate) fn handle_color_hex(&mut self, msg: ColorHexMsg) -> Task<Message> {
        let draft = &mut self.document_state.panel_ctx.color_hex_draft;
        match msg {
            ColorHexMsg::Typed(key, text) => {
                *draft = Some(ColorHexDraft {
                    key,
                    text,
                    refused: false,
                });
                Task::none()
            }
            ColorHexMsg::Restored(key) => {
                if draft.as_ref().is_some_and(|d| d.key == key) {
                    *draft = None;
                }
                Task::none()
            }
            ColorHexMsg::Left { key, text, apply } => {
                // A blur after Enter or Esc finds no draft; a Left built
                // from older text than the box now holds waits for the next.
                let Some(typed) = draft.as_mut().filter(|d| d.key == key && d.text == text) else {
                    return Task::none();
                };
                if crate::panels::parse_typed_hex(&typed.text).is_none() {
                    typed.refused = true;
                    return Task::none();
                }
                *draft = None;
                match apply {
                    Some(pick) => self.handle_dock_message(DockMessage::Panel(*pick)),
                    None => Task::none(),
                }
            }
        }
    }
}
