//! Editing section — schematic move behaviour.

use super::*;
use iced::widget::{Space, checkbox, column, row, text};
use iced::{Element, Length};

const MOVE_KEEPS_WIRES_HELP: &str = "When on, moving, rotating or mirroring symbols, child sheets or sheet pins (drag, Move, Move By, align, rotate, mirror) drags the wires on their pins along, adding a bend where needed to keep them horizontal or vertical. A net check afterwards keeps this from changing connectivity beyond what a plain move, rotate or mirror would do; a pin that cannot be kept connected moves alone and is reported in the Messages panel. When off, only the selected items move or turn and attached wires stay where they were.";

const AUTOSAVE_HELP: &str = "When on, Signex keeps a recovery copy of each schematic with unsaved changes about once a minute. The copy is a separate file in the Signex config folder (recovery); your own file is never touched by it. The copy is removed when you save the document, close it without keeping the changes, or quit with Discard All, even if you turn this off first. Off by default.";

pub(super) fn content_editing<'a>(
    draft_move_keeps_wires: bool,
    draft_autosave_enabled: bool,
) -> Element<'a, PrefMsg> {
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

    let autosave = row![
        column![
            text("Autosave recovery copies")
                .size(12)
                .style(text_primary),
            text(AUTOSAVE_HELP).size(10).style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        checkbox(draft_autosave_enabled)
            .size(16)
            .on_toggle(PrefMsg::DraftAutosaveEnabled),
    ]
    .align_y(iced::Alignment::Center);

    column![
        section_title("Schematic — Editing"),
        Space::new().height(10),
        setting,
        Space::new().height(14),
        autosave,
    ]
    .padding([16, 20])
    .into()
}
