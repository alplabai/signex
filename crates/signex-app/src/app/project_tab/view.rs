//! The Project tab shell: a vertical section list beside the selected
//! section. Pure: `&Signex -> Element`, no mutation.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use super::{ProjectSection, ProjectTabMsg, general};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};

const SECTION_LIST_WIDTH: f32 = 180.0;

fn section_list<'a>(
    project_id: ProjectId,
    selected: ProjectSection,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut list = column![].spacing(2).padding([12, 8]);
    for section in ProjectSection::ALL {
        let label = text(section.label()).size(12);
        let entry = button(label)
            .width(Length::Fill)
            .padding([6, 10])
            .on_press(Message::Project(ProjectMsg::Tab(
                ProjectTabMsg::SectionSelected {
                    project_id,
                    section,
                },
            )));
        let entry = if section == selected {
            entry.style(crate::styles::accent_button(tokens))
        } else {
            entry.style(crate::styles::subtle_button(tokens))
        };
        list = list.push(entry);
    }
    container(list)
        .width(Length::Fixed(SECTION_LIST_WIDTH))
        .height(Length::Fill)
        .style(crate::styles::panel_region(tokens))
        .into()
}

impl Signex {
    /// The project whose Project tab `window` shows: the active tab in the
    /// main window, the undocked tab in an undocked window.
    pub(crate) fn project_tab_in_window(
        &self,
        window: iced::window::Id,
        is_main: bool,
    ) -> Option<ProjectId> {
        let document = &self.document_state;
        let tab = if is_main {
            document.tabs.get(document.active_tab)
        } else {
            let Some(crate::app::state::WindowKind::UndockedTab { path, .. }) =
                self.ui_state.windows.get(&window)
            else {
                return None;
            };
            document.tabs.iter().find(|tab| &tab.path == path)
        };
        tab.and_then(|tab| tab.kind.as_project())
    }

    /// The Project tab for `project_id`, in the main window or an undocked
    /// one. A tab whose project is gone renders a short note instead.
    pub(crate) fn view_project_tab(&self, project_id: ProjectId) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return container(
                text("This project is no longer open.")
                    .size(13)
                    .color(crate::styles::ti(tokens.text_secondary)),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .into();
        };
        let selected = self
            .document_state
            .project_tabs
            .get(&project_id)
            .map(|tab| tab.section)
            .unwrap_or_default();
        let content = match selected {
            ProjectSection::General => general::view(project, tokens),
        };
        row![
            section_list(project_id, selected, tokens),
            container(column![content, Space::new().height(Length::Fill)])
                .width(Length::Fill)
                .height(Length::Fill)
                .style(crate::styles::panel_region(tokens)),
        ]
        .into()
    }
}
