//! The "Designator standard" row of the General section (#1232): the view,
//! and the write path it drives.
//!
//! A pick is applied through the General section's undo history
//! (`ProjectGeneralCommand::SetDesignatorStandard`, #1421), in order with
//! description and parameter edits; it flips the `.snxprj` dirty and the
//! file is written by the normal explicit save (atomic write).

use iced::widget::{Widget, column, pick_list};
use iced::{Element, Length};
use signex_types::designator::DesignatorStandard;
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use super::view::{labelled, tab_message};
use crate::app::Message;
use crate::app::project_tab::ProjectTabMsg;
use crate::app::state::LoadedProject;
use crate::styles::{Spacing, TypeScale};

/// One picker entry: a standard, or `None` for "Library designator".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StandardChoice(Option<DesignatorStandard>);

impl StandardChoice {
    const ALL: [Self; 4] = [
        Self(None),
        Self(Some(DesignatorStandard::Ieee315)),
        Self(Some(DesignatorStandard::Din40719_2)),
        Self(Some(DesignatorStandard::Iec81346_2)),
    ];
}

impl std::fmt::Display for StandardChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(standard) => standard.fmt(f),
            None => f.write_str("Library designator"),
        }
    }
}

/// Label + picker, read live from the project so the tab holds no copy.
pub(super) fn row<'a>(project: &'a LoadedProject, tokens: &ThemeTokens) -> Element<'a, Message> {
    let ts = TypeScale::default();
    let project_id = project.id;
    let picker: Element<'a, Message> = pick_list(
        Some(StandardChoice(project.data.designator_standard())),
        StandardChoice::ALL,
        |choice: &StandardChoice| choice.to_string(),
    )
    .on_select(move |choice: StandardChoice| {
        tab_message(ProjectTabMsg::DesignatorStandardPicked {
            project_id,
            standard: choice.0,
        })
    })
    .text_size(ts.label)
    .width(Length::Fill)
    .boxed();
    let field: Element<'a, Message> = if project.data.has_unreadable_annotation() {
        column![
            picker,
            text(raw("Set by a newer Signex version \u{2014} kept unchanged"))
                .size(ts.caption)
                .color(crate::styles::ti(tokens.text_secondary))
                .boxed(),
        ]
        .spacing(Spacing::default().s)
        .boxed()
    } else {
        picker
    };
    labelled("Designator standard", field, tokens)
}
