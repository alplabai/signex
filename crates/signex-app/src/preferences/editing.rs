//! Editing section — schematic move behaviour.

use super::*;
use iced::widget::{Space, checkbox, column, row, text};
use iced::{Element, Length};

const MOVE_KEEPS_WIRES_HELP: &str = "When on, moving symbols, child sheets or sheet pins (drag, Move, Move By, align) drags the wires on their pins along, adding a bend where needed to keep them horizontal or vertical. A net check after the move keeps this from changing connectivity beyond what a plain move would do; a pin that cannot be kept connected moves alone and is reported in the Messages panel. Rotate and mirror do not rubber-band yet. When off, only the selected items move and attached wires stay where they were.";

pub(super) fn content_editing<'a>(draft_move_keeps_wires: bool) -> Element<'a, PrefMsg> {
    let setting = row![
        column![
            text("Keep wires connected when moving")
                .size(12)
                .style(text_primary),
            text(MOVE_KEEPS_WIRES_HELP).size(10).style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        checkbox(draft_move_keeps_wires)
            .size(16)
            .on_toggle(PrefMsg::DraftMoveKeepsWires),
    ]
    .align_y(iced::Alignment::Center);

    column![
        section_title("Schematic — Editing"),
        Space::new().height(10),
        setting,
    ]
    .padding([16, 20])
    .into()
}
