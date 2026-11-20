//! Annotation page (mockup row 146): the undo scope of an annotate run, and a link to
//! Designators on paste, which lives on Editing.

use crate::annotate_undo::{AnnotateUndo, annotate_undo_label};
use iced::widget::Column;
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::{button, radio, text};

use super::setting_row::{self, SettingRow};
use super::{PrefMsg, PrefNav, PrefsView};
use crate::styles::{Spacing, TypeScale, chrome_px, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Side of a radio button (14 px).
const RADIO: f32 = chrome_px(14.0 * 11.0 / 12.0);
/// Gap between the two radio rows (6 px).
const RADIO_GAP: f32 = chrome_px(6.0 * 11.0 / 12.0);

fn annotate_undo_help(i18n: &I18n) -> signex_i18n::Localized {
    t!(
        i18n,
        ids::PREFERENCES_ANNOTATE_UNDO_HELP,
        "undo" => crate::keymap::shown("Mod+Z")
    )
}

/// The Annotation page. The help names the two choices in plain text: the
/// house text has no styled runs inside one catalog sentence, and splitting
/// the sentence to bold them would break it for translators.
pub(super) fn content_annotation<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (i18n, t) = (v.i18n, v.panel_tokens);
    let selected = Some(v.draft_annotate_undo);
    let radios = Column::with_children(AnnotateUndo::ALL.iter().map(|&scope| {
        radio(
            t!(i18n, annotate_undo_label(scope)),
            scope,
            selected,
            PrefMsg::DraftAnnotateUndo,
        )
        .size(RADIO)
        .spacing(SP.m)
        .text_size(TS.body)
        .boxed()
    }))
    .spacing(RADIO_GAP)
    .width(Length::Fill);

    let accent = tokens::resolve(t).accent_text;
    let open_editing = button(
        text(t!(i18n, ids::PREFERENCES_ANNOTATION_OPEN_EDITING))
            .size(TS.body)
            .color(accent),
    )
    .padding(0)
    .on_press(PrefMsg::Nav(PrefNav::Editing))
    .style(move |_: &Theme, _| button::Style {
        background: None,
        text_color: accent,
        ..button::Style::default()
    });

    setting_row::page(vec![
        setting_row::page_heading(
            t!(i18n, ids::PREFERENCES_ANNOTATION_HEADING),
            t!(i18n, ids::PREFERENCES_ANNOTATION_INTRO),
            t,
        ),
        setting_row::section_header(t!(i18n, ids::PREFERENCES_ANNOTATION_SECTION_UNDO), None, t),
        setting_row::setting_row(
            SettingRow {
                label: t!(i18n, ids::PREFERENCES_ANNOTATE_UNDO),
                pills: Vec::new(),
                help: Some(setting_row::help_text(annotate_undo_help(i18n), t)),
                control: radios.boxed(),
                error: None,
            },
            t,
        ),
        setting_row::section_header(
            t!(i18n, ids::PREFERENCES_ANNOTATION_SECTION_RELATED),
            None,
            t,
        ),
        setting_row::setting_row(
            SettingRow {
                label: t!(i18n, ids::PREFERENCES_PASTE_DESIGNATORS),
                pills: Vec::new(),
                help: Some(setting_row::help_text(
                    t!(i18n, ids::PREFERENCES_ANNOTATION_PASTE_HELP),
                    t,
                )),
                control: open_editing.boxed(),
                error: None,
            },
            t,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::annotate_undo_help;
    use crate::annotate_undo::{AnnotateUndo, annotate_undo_label};

    #[test]
    fn the_help_text_has_no_stray_line_continuation_backslash() {
        let help = annotate_undo_help(&signex_i18n::I18n::en_us()).into_string();
        assert!(!help.contains('\\'), "{help}");
    }

    /// Mockup row 146: the help names both choices as the radios read them.
    #[test]
    fn the_help_names_both_choices() {
        let en = signex_i18n::I18n::en_us();
        let help = annotate_undo_help(&en).into_string();
        for scope in AnnotateUndo::ALL {
            let name = signex_i18n::t!(en, annotate_undo_label(*scope)).into_string();
            assert!(help.contains(&name), "{name} not in {help}");
        }
    }
}
