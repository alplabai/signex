//! The Project tab shell: a vertical section list beside the selected
//! section. Pure: `&Signex -> Element`, no mutation.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use super::{ProjectSection, ProjectTabMsg, general, parameters};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::styles::{Spacing, TypeScale};

/// Width of the section list, in `body`-size characters.
const SECTION_LIST_EMS: f32 = 16.0;

fn section_list<'a>(
    project_id: ProjectId,
    selected: ProjectSection,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let mut list = column![].spacing(sp.xs).padding([sp.l, sp.m]);
    for section in ProjectSection::ALL {
        let label = text(section.label()).size(ts.label);
        let entry = button(label)
            .width(Length::Fill)
            .padding([sp.s, sp.m])
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
        .width(Length::Fixed(ts.body * SECTION_LIST_EMS))
        .height(Length::Fill)
        .style(crate::styles::panel_region(tokens))
        .into()
}

impl Signex {
    /// The project whose Project tab `window` shows. Only the main window
    /// shows one: a Project tab is not undocked (see the module docs).
    pub(crate) fn project_tab_in_window(&self, is_main: bool) -> Option<ProjectId> {
        let document = &self.document_state;
        if !is_main {
            return None;
        }
        document
            .tabs
            .get(document.active_tab)
            .and_then(|tab| tab.kind.as_project())
    }

    /// The Project tab for `project_id`, in the main window or an undocked
    /// one. A tab whose project is gone renders a short note instead.
    pub(crate) fn view_project_tab(&self, project_id: ProjectId) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return container(
                text("This project is no longer open.")
                    .size(TypeScale::default().title)
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
        let new_parameter_name = self
            .document_state
            .project_tabs
            .get(&project_id)
            .map_or("", |tab| tab.new_parameter_name.as_str());
        let content = match selected {
            ProjectSection::General => general::view(project, tokens),
            ProjectSection::Parameters => parameters::view(project, new_parameter_name, tokens),
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
