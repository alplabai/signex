//! Annotation section — the "Annotate undo" scope of an annotate run.

use super::*;
use crate::annotate_undo::AnnotateUndo;
use iced::widget::{Space, column, pick_list, row, text};
use iced::{Element, Length};

pub(super) fn content_annotation<'a>(draft: AnnotateUndo) -> Element<'a, PrefMsg> {
    let setting = row![
        column![
            text("Annotate undo").size(12).style(text_primary),
            text(
                "How many undo steps one annotate run is. One step reverts every sheet the \\
                 run touched together from whichever sheet Ctrl+Z is pressed on; per sheet \\
                 undoes each sheet on its own. Applies to the next annotate run."
            )
            .size(10)
            .style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        pick_list(AnnotateUndo::ALL, Some(draft), PrefMsg::DraftAnnotateUndo).text_size(12),
    ]
    .align_y(iced::Alignment::Center);

    column![
        section_title("Schematic — Annotation"),
        Space::new().height(10),
        setting,
    ]
    .padding([16, 20])
    .into()
}
