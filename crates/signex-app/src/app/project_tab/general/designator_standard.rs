//! The "Designator standard" row of the General section (#1232): the view,
//! and the write path it drives.
//!
//! A pick is applied through the General section's undo history
//! (`ProjectGeneralCommand::SetDesignatorStandard`, #1421), in order with
//! description and parameter edits; it flips the `.snxprj` dirty and the
//! file is written by the normal explicit save (atomic write).

use iced::widget::{Column, Widget, pick_list};
use iced::{Element, Length};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::designator::DesignatorStandard;
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text;

use super::view::tab_message;
use crate::app::Message;
use crate::app::project_tab::ProjectTabMsg;
use crate::app::project_tab::look::kv_row;
use crate::app::project_tab::table::{SP, TS};
use crate::app::state::LoadedProject;
use crate::styles::tokens::resolve;

/// Width of the picker (280 px).
const PICKER_W: f32 = TS.body * 70.0 / 3.0;

/// One picker entry: a standard, or `None` for "Library designator".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StandardChoice(Option<DesignatorStandard>);

impl StandardChoice {
    const ALL: [Self; 5] = [
        Self(None),
        Self(Some(DesignatorStandard::Ieee315)),
        Self(Some(DesignatorStandard::Din40719_2)),
        Self(Some(DesignatorStandard::Iec81346_2)),
        Self(Some(DesignatorStandard::Iec81346_2Full)),
    ];

    /// The words shown for this entry.
    fn label(self, i18n: &I18n) -> String {
        match self.0 {
            Some(standard) => standard.to_string(),
            None => i18n.message_plain(ids::PROJECT_GENERAL_LIBRARY_DESIGNATOR, &[]),
        }
    }
}

/// The caption under the picker: every choice it offers.
fn standards_hint(i18n: &I18n) -> Localized {
    let names: Vec<String> = StandardChoice::ALL
        .iter()
        .map(|choice| choice.label(i18n))
        .collect();
    t!(
        i18n,
        ids::PROJECT_GENERAL_DESIGNATOR_HINT,
        "standards" => names.join(" · ")
    )
}

/// Label + picker + hint, read live from the project so the tab holds no
/// copy.
pub(super) fn row<'a>(
    project: &'a LoadedProject,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let labels = StandardChoice::ALL.map(|choice| choice.label(i18n));
    let picker: Element<'a, Message> = pick_list(
        Some(StandardChoice(project.data.designator_standard())),
        StandardChoice::ALL,
        move |choice: &StandardChoice| {
            let at = StandardChoice::ALL.iter().position(|c| c == choice);
            labels[at.unwrap_or(0)].clone()
        },
    )
    .graphite()
    .on_select(move |choice: StandardChoice| {
        tab_message(ProjectTabMsg::DesignatorStandardPicked {
            project_id,
            standard: choice.0,
        })
    })
    .width(Length::Fixed(PICKER_W))
    .boxed();
    let ink3 = resolve(tokens).ink3;
    let caption = |words: Localized| -> Element<'a, Message> {
        text(words).size(TS.caption).color(ink3).boxed()
    };
    let mut field: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.s)
        .push(picker)
        .push(caption(standards_hint(i18n)));
    if project.data.has_unreadable_annotation() {
        field = field.push(caption(t!(i18n, ids::PROJECT_GENERAL_DESIGNATOR_NEWER)));
    }
    kv_row(
        t!(i18n, ids::PROJECT_GENERAL_DESIGNATOR_STANDARD),
        field.boxed(),
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup row 29: the hint names every choice of the picker.
    #[test]
    fn the_hint_names_every_choice() {
        let i18n = I18n::en_us_ref();
        let hint = standards_hint(i18n).into_string();
        for choice in StandardChoice::ALL {
            let label = choice.label(i18n);
            assert!(hint.contains(&label), "{label} is not in {hint}");
        }
    }

    fn project() -> LoadedProject {
        let data = serde_json::from_str(
            r#"{"name":"dcdc-buck","dir":"","schematic_root":null,"pcb_file":null}"#,
        )
        .expect("a project");
        LoadedProject {
            id: crate::app::state::ProjectId::test_id(29),
            path: std::env::temp_dir().join("designator-hint.snxprj"),
            data,
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        }
    }

    /// Mockup row 29 (#2805): the hint sits in the value column under the
    /// picker, on one line, and names the choices in the picker's order.
    #[test]
    fn the_hint_lays_out_under_the_picker_in_the_value_column() {
        let (app, _boot) = crate::app::Signex::new();
        let i18n = &app.ui_state.i18n;
        let tokens = &app.document_state.panel_ctx.tokens;
        let project = project();
        let window = iced::Size::new(900.0, 400.0);
        let texts: Vec<crate::layout_probe::LaidOutText> =
            crate::layout_probe::lay_out_texts(row(&project, i18n, tokens), window)
                .into_iter()
                .map(|t| crate::layout_probe::LaidOutText {
                    text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
                    ..t
                })
                .collect();
        let hint_words = standards_hint(i18n)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        let names: Vec<String> = StandardChoice::ALL.iter().map(|c| c.label(i18n)).collect();
        assert_eq!(hint_words, names.join(" · "));
        let caption = crate::layout_probe::find(
            &texts,
            &t!(i18n, ids::PROJECT_GENERAL_DESIGNATOR_STANDARD).into_string(),
        )
        .bounds;
        let hint = crate::layout_probe::find(&texts, &hint_words).bounds;
        assert!(
            hint.x > caption.x + caption.width,
            "the hint is in the value column: {hint:?} vs {caption:?}"
        );
        assert!(
            hint.center_y() > caption.center_y(),
            "the hint is under the picker: {hint:?} vs {caption:?}"
        );
        let one_line = crate::layout_probe::natural::<Message>(
            text(signex_i18n::UserText::new(hint_words.clone()))
                .size(TS.caption)
                .boxed(),
            &hint_words,
        );
        assert!(
            hint.height < one_line.height * 1.5,
            "the hint wraps: {hint:?} vs {one_line:?}"
        );
        assert!(hint.x + hint.width <= window.width, "{hint:?}");
    }

    /// The picker offers "Library designator" and every standard once, in
    /// the standards' own order, so a new standard cannot be left out.
    #[test]
    fn the_picker_offers_every_standard_once() {
        let mut expected = vec![StandardChoice(None)];
        expected.extend(DesignatorStandard::ALL.map(|s| StandardChoice(Some(s))));
        assert_eq!(StandardChoice::ALL.to_vec(), expected);
        assert_eq!(
            StandardChoice(Some(DesignatorStandard::Iec81346_2Full)).label(I18n::en_us_ref()),
            "EN IEC 81346-2 (full codes)"
        );
        assert_eq!(
            StandardChoice(None).label(I18n::en_us_ref()),
            "Library designator"
        );
    }
}
