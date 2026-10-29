//! `update` for the Project tab: section switches, General edits, and
//! undo / redo of the General fields. The only mutation site of the tab.

use signex_engine::ProjectGeneralCommand;

use super::{ProjectSection, ProjectTabMsg};
use crate::app::Signex;
use crate::app::state::ProjectId;

impl Signex {
    pub(crate) fn dispatch_project_tab_message(&mut self, msg: ProjectTabMsg) {
        match msg {
            ProjectTabMsg::SectionSelected {
                project_id,
                section,
            } => self.select_project_section(project_id, section),
            ProjectTabMsg::DescriptionEdited { project_id, text } => {
                self.apply_project_general(project_id, ProjectGeneralCommand::SetDescription(text));
            }
            ProjectTabMsg::DesignatorStandardPicked {
                project_id,
                standard,
            } => {
                self.apply_project_general(
                    project_id,
                    ProjectGeneralCommand::SetDesignatorStandard(standard),
                );
            }
            ProjectTabMsg::ParameterEdited {
                project_id,
                name,
                value,
            } => {
                self.apply_project_general(
                    project_id,
                    ProjectGeneralCommand::SetParameter { name, value },
                );
            }
            ProjectTabMsg::ParameterRemoved { project_id, name } => {
                self.close_general_run(project_id);
                self.apply_project_general(
                    project_id,
                    ProjectGeneralCommand::RemoveParameter(name),
                );
            }
            ProjectTabMsg::NewParameterNameEdited { project_id, text } => {
                if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
                    tab.new_parameter_name = text;
                }
            }
            ProjectTabMsg::ParameterAdded { project_id } => {
                self.close_general_run(project_id);
                let tab = self
                    .document_state
                    .project_tabs
                    .entry(project_id)
                    .or_default();
                let name = std::mem::take(&mut tab.new_parameter_name)
                    .trim()
                    .to_string();
                let exists = self
                    .document_state
                    .project_by_id(project_id)
                    .is_none_or(|p| p.data.parameters.contains_key(&name));
                if !name.is_empty() && !exists {
                    self.apply_project_general(
                        project_id,
                        ProjectGeneralCommand::SetParameter {
                            name,
                            value: String::new(),
                        },
                    );
                    self.close_general_run(project_id);
                }
            }
            ProjectTabMsg::RenameRequested { project_id } => {
                self.open_project_rename_from_tab(project_id);
            }
        }
    }

    /// End the General history's typing run: the next edit is its own step.
    fn close_general_run(&mut self, project_id: ProjectId) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.general_history.close_run();
        }
    }

    /// Rename… in the General section: the same dialog and flow as the
    /// project tree's Rename. Not a document edit, so nothing is dirtied.
    fn open_project_rename_from_tab(&mut self, project_id: ProjectId) {
        match self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id)
        {
            Some(idx) => self.open_project_rename_dialog(vec![idx]),
            None => crate::diagnostics::log_warning(
                "Project tab: the project was closed; nothing to rename",
            ),
        }
    }

    fn select_project_section(&mut self, project_id: ProjectId, section: ProjectSection) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.section = section;
            // A section switch ends the typing run: the next edit is its own
            // undo step.
            tab.general_history.close_run();
        }
    }

    /// Apply one General edit to the project's data and mark the `.snxprj`
    /// dirty. An edit for a project that is no longer loaded is dropped.
    fn apply_project_general(&mut self, project_id: ProjectId, command: ProjectGeneralCommand) {
        let document = &mut self.document_state;
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return;
        };
        let tab = document.project_tabs.entry(project_id).or_default();
        if tab.general_history.apply(&mut project.data, command) {
            let project_file = project.path.clone();
            document.mark_document_dirty(&project_file);
            self.refresh_panel_ctx();
        }
    }

    /// The project whose Project tab is the active tab.
    pub(crate) fn active_project_tab(&self) -> Option<ProjectId> {
        let document = &self.document_state;
        document
            .tabs
            .get(document.active_tab)
            .and_then(|tab| tab.kind.as_project())
    }

    /// `(can_undo, can_redo)` of a Project tab's history.
    pub(crate) fn project_tab_history_available(&self, project_id: ProjectId) -> (bool, bool) {
        self.document_state
            .project_tabs
            .get(&project_id)
            .map_or((false, false), |tab| {
                (
                    tab.general_history.can_undo(),
                    tab.general_history.can_redo(),
                )
            })
    }

    /// Ctrl+Z / Ctrl+Y in a Project tab. Returns whether a step moved.
    pub(crate) fn step_project_tab_history(&mut self, project_id: ProjectId, undo: bool) -> bool {
        let document = &mut self.document_state;
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            return false;
        };
        let Some(tab) = document.project_tabs.get_mut(&project_id) else {
            return false;
        };
        let moved = if undo {
            tab.general_history.undo(&mut project.data)
        } else {
            tab.general_history.redo(&mut project.data)
        };
        if moved {
            // Like an edit: the document now differs from what was last
            // saved, or matches it only by coincidence; the save decides.
            let project_file = project.path.clone();
            document.mark_document_dirty(&project_file);
            self.refresh_panel_ctx();
        }
        moved
    }
}
