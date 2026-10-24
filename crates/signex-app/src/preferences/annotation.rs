//! Annotation section — the "Annotate undo" scope of an annotate run.

use super::*;
use crate::annotate_undo::AnnotateUndo;
use iced::widget::{Space, column, pick_list, row, text};
use iced::{Element, Length};

const ANNOTATE_UNDO_HELP: &str = "How many undo steps one annotate run is. One step reverts every sheet the run touched together from whichever sheet Ctrl+Z is pressed on; per sheet undoes each sheet on its own. Applies to the next annotate run.";

pub(super) fn content_annotation<'a>(draft: AnnotateUndo) -> Element<'a, PrefMsg> {
    let setting = row![
        column![
            text("Annotate undo").size(12).style(text_primary),
            text(ANNOTATE_UNDO_HELP).size(10).style(text_muted),
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

#[cfg(test)]
mod tests {
    use super::ANNOTATE_UNDO_HELP;

    #[test]
    fn the_help_text_has_no_stray_line_continuation_backslash() {
        assert!(!ANNOTATE_UNDO_HELP.contains('\\'), "{ANNOTATE_UNDO_HELP}");
    }
}
