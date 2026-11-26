//! The Alternate part picker and the Edit value popover of the canvas's In
//! variant flyout (mockup rows 208, 209), laid over the window they opened
//! in: the picker centred over a click-outside layer, the popover at the
//! point the menu opened. Esc and a click outside cancel either.

use super::*;
use crate::app::variant_part_edit::{EditCard, VariantPartEditMsg};

impl Signex {
    pub(in crate::app::view) fn variant_part_edit_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let cancel = Message::from(VariantPartEditMsg::CancelPressed);
        match self.variant_part_edit_card(window_id) {
            None => Vec::new(),
            Some(EditCard::Centred(card)) => Self::dismiss_and_centre(cancel, card),
            Some(EditCard::Anchored(card, anchor)) => Self::anchored_layers(
                cancel,
                crate::app::view::context_menu::AnchoredMenu::new(card, anchor),
            ),
        }
    }
}
