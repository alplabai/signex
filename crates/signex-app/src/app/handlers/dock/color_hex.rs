//! The hex field of a colour row (mockup row 46): keystrokes are a draft,
//! Enter or leaving the field runs the pick the row's popover sends, so a
//! typed colour is one undo step; refused text stays, marked; Esc drops it.

use iced::Task;

use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{ColorHexDraft, ColorHexMsg, PanelMsg};

impl Signex {
    /// Settle `msg`, then finish the update once: an applied pick runs
    /// through the dock router, which finishes the update itself.
    pub(crate) fn handle_color_hex(&mut self, msg: ColorHexMsg) -> Task<Message> {
        match self.settle_color_hex(msg) {
            Some(pick) => self.handle_dock_message(DockMessage::Panel(pick)),
            None => self.finish_update(),
        }
    }

    /// Update the draft; returns the pick to apply, if any.
    fn settle_color_hex(&mut self, msg: ColorHexMsg) -> Option<PanelMsg> {
        let ctx = &mut self.document_view.panel_ctx;
        // The filter `refresh_panel_ctx` keeps a draft by.
        let (selected, has_tab_form) = (ctx.selected_uuid, ctx.pre_placement.is_some());
        let draft = &mut ctx.color_hex_draft;
        match msg {
            ColorHexMsg::Typed(key, text) => {
                *draft = Some(ColorHexDraft {
                    key,
                    text,
                    refused: false,
                });
                None
            }
            ColorHexMsg::Restored(key) => {
                if draft.as_ref().is_some_and(|d| d.key == key) {
                    *draft = None;
                }
                None
            }
            ColorHexMsg::Left { key, text, apply } => {
                // A blur after Enter or Esc finds no draft; a Left built
                // from older text than the box now holds waits for the next.
                let typed = draft.as_mut().filter(|d| d.key == key && d.text == text)?;
                // The row left the screen (the selection changed): its pick
                // would colour what is selected now, so nothing applies.
                if !typed.key.is_shown_for(selected, has_tab_form) {
                    *draft = None;
                    return None;
                }
                if crate::panels::parse_typed_hex(&typed.text).is_none() {
                    typed.refused = true;
                    return None;
                }
                *draft = None;
                apply.map(|pick| *pick)
            }
        }
    }
}
