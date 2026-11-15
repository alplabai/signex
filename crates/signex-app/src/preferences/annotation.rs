//! Annotation section — the "Annotate undo" scope of an annotate run.

use super::*;
use crate::annotate_undo::AnnotateUndo;
use iced::widget::{Space, Widget, column, pick_list, row};
use iced::{Element, Length};
use signex_i18n::{I18n, ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

fn annotate_undo_help(i18n: &I18n) -> signex_i18n::Localized {
    t!(
        i18n,
        ids::PREFERENCES_ANNOTATE_UNDO_HELP,
        "undo" => crate::keymap::shown("Mod+Z")
    )
}

pub(super) fn content_annotation<'a>(draft: AnnotateUndo, i18n: &I18n) -> Element<'a, PrefMsg> {
    let setting = row![
        column![
            text(t!(i18n, ids::PREFERENCES_ANNOTATE_UNDO))
                .size(TS.label)
                .style(text_primary),
            text(annotate_undo_help(i18n))
                .size(TS.caption)
                .style(text_muted),
        ]
        .spacing(3)
        .width(320),
        Space::new().width(Length::Fill),
        // The scope names come from `AnnotateUndo`'s Display (#2059).
        pick_list(Some(draft), AnnotateUndo::ALL, |v: &AnnotateUndo| v
            .to_string())
        .graphite()
        .on_select(PrefMsg::DraftAnnotateUndo),
    ]
    .align_y(iced::Alignment::Center);

    column![
        section_title(t!(i18n, ids::PREFERENCES_SECTION_ANNOTATION)),
        Space::new().height(10),
        setting,
    ]
    .padding([SP.xl, 20.0])
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::annotate_undo_help;

    #[test]
    fn the_help_text_has_no_stray_line_continuation_backslash() {
        let help = annotate_undo_help(&signex_i18n::I18n::en_us()).into_string();
        assert!(!help.contains('\\'), "{help}");
    }
}
