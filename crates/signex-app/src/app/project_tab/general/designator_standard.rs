//! The "Designator standard" row of the General section (#1232): the view,
//! and the write path it drives.
//!
//! The write mutates the project's `ProjectData` and flips its `.snxprj`
//! dirty; the file is written by the normal explicit save (atomic write). It
//! is not an undo step: the picker is the whole edit surface and re-picking
//! restores the previous value.

use iced::widget::{column, pick_list, text};
use iced::{Element, Length};
use signex_types::designator::DesignatorStandard;
use signex_types::theme::ThemeTokens;

use super::view::{labelled, tab_message};
use crate::app::project_tab::ProjectTabMsg;
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, Signex};
use crate::styles::TypeScale;

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
        StandardChoice::ALL,
        Some(StandardChoice(project.data.designator_standard())),
        move |choice: StandardChoice| {
            tab_message(ProjectTabMsg::DesignatorStandardPicked {
                project_id,
                standard: choice.0,
            })
        },
    )
    .text_size(ts.label)
    .width(Length::Fill)
    .into();
    let field = if project.data.has_unreadable_annotation() {
        column![
            picker,
            text("Set by a newer Signex version \u{2014} kept unchanged")
                .size(ts.caption)
                .color(crate::styles::ti(tokens.text_secondary)),
        ]
        .spacing(4)
        .into()
    } else {
        picker
    };
    labelled("Designator standard", field, tokens)
}

impl Signex {
    /// Set (or, with `None`, clear) the designator standard of the project.
    pub(in crate::app::project_tab) fn set_project_designator_standard(
        &mut self,
        project_id: ProjectId,
        standard: Option<DesignatorStandard>,
    ) {
        let ds = &mut self.document_state;
        let Some(project) = ds.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the designator standard was not changed",
            );
            return;
        };
        // An unreadable section from a newer Signex is replaced only by a real
        // standard; re-picking "Library designator" must not discard it.
        let unreadable = project.data.has_unreadable_annotation();
        if unreadable && standard.is_none() {
            return;
        }
        if !unreadable && project.data.designator_standard() == standard {
            return;
        }
        project.data.set_designator_standard(standard);
        let path = project.path.clone();
        ds.dirty_paths.insert(path);
        self.refresh_panel_ctx();
    }
}
