//! The Project tab shell: a vertical section list beside the selected
//! section. Pure: `&Signex -> Element`, no mutation.

use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::{
    ProjectSection, ProjectTabMsg, erc_annotation, general, libraries, parameters, requirements,
    schematic_defaults, setup, sheets, variants, version_control,
};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::styles::{Spacing, TypeScale};

/// Width of the section list, in `body`-size characters.
const SECTION_LIST_EMS: f32 = 16.0;

fn section_list<'a>(
    project_id: ProjectId,
    selected: ProjectSection,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (ts, sp) = (TypeScale::default(), Spacing::default());
    let mut list: iced::widget::Column<Element<'a, Message>> =
        column![].spacing(sp.xs).padding([sp.l, sp.m]);
    for section in ProjectSection::ALL {
        let label = text(t!(i18n, section.title())).size(ts.label);
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
        list = list.push(entry.boxed());
    }
    container(list)
        .width(Length::Fixed(ts.body * SECTION_LIST_EMS))
        .height(Length::Fill)
        .style(crate::styles::panel_region(tokens))
        .boxed()
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
        let i18n = self.ui_state.i18n.as_ref();
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return container(
                text(raw("This project is no longer open."))
                    .size(TypeScale::default().title)
                    .color(crate::styles::ti(tokens.text_secondary)),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed();
        };
        let tab = self.document_state.project_tabs.get(&project_id);
        let selected = tab.map(|tab| tab.section).unwrap_or_default();
        let setup_draft = tab.and_then(|tab| tab.setup_draft.as_ref());
        let users = &self.ui_state.setup_profiles;
        let content = match selected {
            ProjectSection::General => general::view(&self.ui_state.i18n, project, tokens),
            ProjectSection::Parameters => parameters::view(project, tab, i18n, tokens),
            ProjectSection::Sheets => sheets::view(project, setup_draft, users, i18n, tokens),
            ProjectSection::Variants => variants::view(project, tab, i18n, tokens),
            ProjectSection::Libraries => libraries::view(
                project_id,
                &self.project_library_rows(project),
                i18n,
                tokens,
            ),
            ProjectSection::ErcAnnotation => {
                let facts = self.erc_section_facts(project);
                erc_annotation::view(project, &facts, i18n, tokens)
            }
            ProjectSection::VersionControl => {
                let facts = self.version_control_facts(project);
                version_control::view(project, &facts, i18n, tokens)
            }
            ProjectSection::SchematicDefaults => {
                schematic_defaults::view(project, tab.map(|tab| &tab.default_drafts), tokens)
            }
            ProjectSection::Setup => setup::view(
                project,
                setup_draft,
                tab.and_then(|tab| tab.journal_lines_draft.as_deref()),
                users,
                i18n,
                tokens,
            ),
            ProjectSection::Requirements => {
                requirements::view(project, tab.map(|tab| &tab.requirements), i18n, tokens)
            }
        };
        row![
            section_list(project_id, selected, i18n, tokens),
            container(column![content, Space::new().height(Length::Fill)])
                .width(Length::Fill)
                .height(Length::Fill)
                .style(crate::styles::panel_region(tokens)),
        ]
        .boxed()
    }
}
