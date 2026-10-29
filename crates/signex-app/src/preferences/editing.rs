//! Editing section — schematic move behaviour.

use super::*;
use iced::widget::{Space, checkbox, column, pick_list, row, slider, text};
use iced::{Element, Length};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const MOVE_KEEPS_WIRES_HELP: &str = "When on, moving, rotating or mirroring symbols, child sheets or sheet pins (drag, Move, Move By, align, rotate, mirror) drags the wires on their pins along, adding a bend where needed to keep them horizontal or vertical. A net check afterwards keeps this from changing connectivity beyond what a plain move, rotate or mirror would do; a pin that cannot be kept connected moves alone and is reported in the Messages panel. When off, only the selected items move or turn and attached wires stay where they were. Hold Alt through one drag to do the opposite for that drag only; the Drag with connections command (G in the Classic profile) drags the picked object with its wires whatever this says.";

const DRAG_DIRECTION_HELP: &str = "When on, dragging a selection box from left to right selects only what the box fully contains, and from right to left selects everything the box touches. When off, the Inside / Touching selection mode (Shift+S cycles it) decides for both directions; while on, that mode is not used for box selection.";

const INCREMENT_STEP_HELP: &str = "How far the trailing number of a text moves each time it is repeated: placing the label D0 again and again gives D1, D2, ... with a step of 1, and Paste Array offers the same step. A negative step counts down; a number at the start or middle of a text is left alone.";

const PASTE_DESIGNATORS_HELP: &str = "What pasted parts are called. Keep leaves the designators of the copy as they were (duplicates stay until Annotate runs). Reset turns them into R?, U?, ... for Annotate to number. Next free number gives each part the next unused number of its prefix in the project. Applies to Paste, Smart Paste, Duplicate, rubber-stamp copy and Paste Array; the units of one part always stay one part.";

const AUTOSAVE_HELP: &str = "When on, Signex keeps a recovery copy of each schematic, symbol and footprint with unsaved changes about once a minute. The copy is a separate file in the Signex config folder (recovery); your own file is never touched by it. The copy is removed when you save the document, close it without keeping the changes, or quit with Discard All, even if you turn this off first. Off by default.";

pub(super) fn content_editing<'a>(
    draft_move_keeps_wires: bool,
    draft_drag_direction_select: bool,
    draft_increment_step: i32,
    draft_paste_designators: crate::paste_prefs::PasteDesignators,
    draft_autosave_enabled: bool,
) -> Element<'a, PrefMsg> {
    let setting = row![
        column![
            text("Keep wires connected when moving")
                .size(TS.label)
                .style(text_primary),
            text(MOVE_KEEPS_WIRES_HELP)
                .size(TS.caption)
                .style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        checkbox(draft_move_keeps_wires)
            .size(16)
            .on_toggle(PrefMsg::DraftMoveKeepsWires),
    ]
    .align_y(iced::Alignment::Center);

    let drag_direction = row![
        column![
            text("Select by drag direction")
                .size(TS.label)
                .style(text_primary),
            text(DRAG_DIRECTION_HELP).size(TS.caption).style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        checkbox(draft_drag_direction_select)
            .size(16)
            .on_toggle(PrefMsg::DraftDragDirectionSelect),
    ]
    .align_y(iced::Alignment::Center);

    let increment_step = row![
        column![
            text("Increment step").size(TS.label).style(text_primary),
            text(INCREMENT_STEP_HELP).size(TS.caption).style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        slider(
            signex_types::text_increment::STEP_MIN..=signex_types::text_increment::STEP_MAX,
            draft_increment_step,
            PrefMsg::DraftIncrementStep,
        )
        .width(140),
        text(format!("{draft_increment_step:+}"))
            .size(TS.label)
            .style(text_primary)
            .width(48)
            .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let paste_designators = row![
        column![
            text("Designators on paste")
                .size(TS.label)
                .style(text_primary),
            text(PASTE_DESIGNATORS_HELP)
                .size(TS.caption)
                .style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        pick_list(
            crate::paste_prefs::PasteDesignators::ALL,
            Some(draft_paste_designators),
            PrefMsg::DraftPasteDesignators,
        )
        .text_size(12),
    ]
    .align_y(iced::Alignment::Center);

    let autosave = row![
        column![
            text("Autosave recovery copies")
                .size(TS.label)
                .style(text_primary),
            text(AUTOSAVE_HELP).size(TS.caption).style(text_muted),
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
        drag_direction,
        Space::new().height(14),
        increment_step,
        Space::new().height(14),
        paste_designators,
        Space::new().height(14),
        autosave,
    ]
    .padding([SP.xl, 20.0])
    .into()
}
