//! The Design Checklist as a page tab of its project (mockup row 206): one
//! tab per project, `<project> — Design Checklist`, saving into the
//! `.snxprj`. Opening it again focuses it. Like the Project tab, it is not
//! undocked. Opened from Project ▸ Design Checklist and from the project's
//! row in the Projects panel; opening it reads the user's saved templates
//! and asks who the user is (for Mine and the history), both off the UI
//! thread.

use iced::widget::container;
use iced::{Element, Length, Task, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::link_card::picker_open;
use super::view::Seen;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::styles::TypeScale;

impl Signex {
    /// Project ▸ Design Checklist: `project`'s tab, else the active
    /// document's project's, focused or opened. With no project open there
    /// is no tab to open.
    pub(crate) fn open_design_checklist(&mut self, project: Option<ProjectId>) -> Task<Message> {
        match self.rules_project_for(project) {
            Some(id) => self.open_design_checklist_tab(id),
            None => {
                crate::diagnostics::log_warning(
                    "Design Checklist: no project is open; nothing was opened",
                );
                Task::none()
            }
        }
    }

    /// The Projects panel's project row ▸ Design Checklist: the tab of the
    /// project at tree index `project_idx`.
    pub(crate) fn open_design_checklist_at(&mut self, project_idx: usize) -> Task<Message> {
        match self.document_state.projects.get(project_idx).map(|p| p.id) {
            Some(id) => self.open_design_checklist_tab(id),
            None => {
                crate::diagnostics::log_warning(format!(
                    "Design Checklist: no project at tree index {project_idx}"
                ));
                Task::none()
            }
        }
    }

    /// Focus `id`'s Design Checklist tab, or open it; the saved templates
    /// are read again and the user's identity asked once.
    pub(crate) fn open_design_checklist_tab(&mut self, id: ProjectId) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(id) else {
            return Task::none();
        };
        let path = project.path.clone();
        let existing = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.kind.as_design_checklist() == Some(id));
        let idx = match existing {
            Some(idx) => idx,
            None => {
                let kind = TabKind::DesignChecklist(id);
                let title = self.page_tab_title(&kind).unwrap_or_default();
                self.document_state.project_tabs.entry(id).or_default();
                self.document_state.tabs.push(TabInfo {
                    title,
                    path,
                    project_id: Some(id),
                    kind,
                });
                self.document_state.tabs.len() - 1
            }
        };
        if idx != self.document_state.active_tab || existing.is_none() {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
        Task::batch([self.ask_user_identity(), self.read_checklist_templates(id)])
    }

    /// The project whose Design Checklist tab the main window shows.
    pub(crate) fn design_checklist_tab_in_window(&self, is_main: bool) -> Option<ProjectId> {
        if !is_main {
            return None;
        }
        let docs = &self.document_state;
        docs.tabs
            .get(docs.active_tab)
            .and_then(|tab| tab.kind.as_design_checklist())
    }

    /// The Design Checklist tab of `project_id`. A tab whose project is gone
    /// renders a short note instead.
    pub(crate) fn view_design_checklist_tab(&self, project_id: ProjectId) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return container(
                text(t!(self.ui_state.i18n, ids::RULES_NO_PROJECT))
                    .size(TypeScale::default().title)
                    .color(crate::styles::ti(tokens.text_secondary)),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed();
        };
        let state = self.checklist_tab_ref(project_id);
        let me = self.checklist_user();
        // The picker's rows read the open documents: only while it is open.
        let choices = if picker_open(state) {
            self.checklist_link_choices(project_id)
        } else {
            Vec::new()
        };
        let seen = Seen {
            me: &me,
            choices: &choices,
            i18n: self.ui_state.i18n.as_ref(),
            tokens,
        };
        let body = super::view::view(project, state, &seen);
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed()
    }
}
