//! The "Designator standard" row of the Project Options modal (#1232).

use iced::widget::{Space, column, container, pick_list, row, text};
use iced::{Color, Element, Length};
use signex_types::designator::DesignatorStandard;

use super::*;

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

impl Signex {
    /// Label + picker for the project at `project_idx`, read live from the
    /// project so the modal never holds a shadow copy.
    pub(super) fn designator_standard_row(
        &self,
        project_idx: usize,
        text_muted: Color,
    ) -> Element<'_, Message> {
        let unreadable = self
            .document_state
            .projects
            .get(project_idx)
            .is_some_and(|p| p.data.has_unreadable_annotation());
        let current = self
            .document_state
            .projects
            .get(project_idx)
            .and_then(|p| p.data.designator_standard());
        let picker = row![
            container(text("Designator standard").size(11).color(text_muted))
                .width(Length::Fixed(140.0)),
            pick_list(
                StandardChoice::ALL,
                Some(StandardChoice(current)),
                |choice: StandardChoice| Message::Project(ProjectMsg::DesignatorStandardPicked(
                    choice.0
                )),
            )
            .text_size(12)
            .width(220),
            Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if !unreadable {
            return picker.into();
        }
        column![
            picker,
            row![
                Space::new().width(Length::Fixed(148.0)),
                text("Set by a newer Signex version \u{2014} kept unchanged")
                    .size(10)
                    .color(text_muted),
            ],
        ]
        .spacing(4)
        .into()
    }
}
