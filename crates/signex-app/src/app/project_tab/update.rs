//! `update` for the Project tab: section switches, General edits, and
//! undo / redo of the General fields. The only mutation site of the tab.

use signex_engine::ProjectGeneralCommand;

use super::{ProjectSection, ProjectTabMsg};
use crate::app::Signex;
use crate::app::state::ProjectId;

impl Signex {
    pub(crate) fn dispatch_project_tab_message(
        &mut self,
        msg: ProjectTabMsg,
    ) -> iced::Task<crate::app::Message> {
        if let ProjectTabMsg::SetupEdited { project_id, msg } = msg {
            return self.edit_project_setup(project_id, msg);
        }
        self.apply_project_tab_message(msg);
        iced::Task::none()
    }

    fn apply_project_tab_message(&mut self, msg: ProjectTabMsg) {
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
            ProjectTabMsg::SchematicDefaultEdited {
                project_id,
                field,
                text,
            } => {
                use super::schematic_defaults::DefaultInput;
                // A late message for a closed project must not recreate its
                // tab state.
                if self.document_state.project_by_id(project_id).is_none() {
                    crate::diagnostics::log_warning(
                        "Project tab: the project was closed; the edit was dropped",
                    );
                    return;
                }
                let input = super::schematic_defaults::parse_override(field, &text);
                self.document_state
                    .project_tabs
                    .entry(project_id)
                    .or_default()
                    .default_drafts
                    .insert(field, text);
                let value = match input {
                    DefaultInput::Clear => None,
                    DefaultInput::Set(value) => Some(value),
                    DefaultInput::Keep => return,
                };
                self.apply_project_general(
                    project_id,
                    ProjectGeneralCommand::SetSchematicDefault { field, value },
                );
            }
            ProjectTabMsg::SchematicDefaultFontEdited { project_id, text } => {
                self.apply_project_general(
                    project_id,
                    ProjectGeneralCommand::SetSchematicDefaultFont(Some(text)),
                );
            }
            ProjectTabMsg::SchematicDefaultLineStylePicked { project_id, line } => {
                self.apply_project_general(
                    project_id,
                    ProjectGeneralCommand::SetSchematicDefaultLineStyle(line),
                );
            }
            ProjectTabMsg::SchematicDefaultShapeFillPicked { project_id, fill } => {
                self.apply_project_general(
                    project_id,
                    ProjectGeneralCommand::SetSchematicDefaultShapeFill(fill),
                );
            }
            ProjectTabMsg::AgentJournalPicked {
                project_id,
                settings,
            } => self.pick_agent_journal(project_id, settings),
            ProjectTabMsg::AgentJournalLinesEdited { project_id, text } => {
                self.edit_agent_journal_lines(project_id, text);
            }
            ProjectTabMsg::RenameRequested { project_id } => {
                self.open_project_rename_from_tab(project_id);
            }
            // Routed by `dispatch_project_tab_message`, which returns its task.
            ProjectTabMsg::SetupEdited { .. } => {}
        }
    }

    /// End the General history's typing run: the next edit is its own step.
    pub(super) fn close_general_run(&mut self, project_id: ProjectId) {
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
            tab.default_drafts.clear();
            tab.setup_draft = None;
            tab.journal_lines_draft = None;
        }
    }

    /// Apply one General edit to the project's data and mark the `.snxprj`
    /// dirty. An edit for a project that is no longer loaded is dropped.
    pub(crate) fn apply_project_general(
        &mut self,
        project_id: ProjectId,
        command: ProjectGeneralCommand,
    ) {
        let document = &mut self.document_state;
        // The cursor to undo back to is the state before the first edit.
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return;
        };
        let tab = document.project_tabs.entry(project_id).or_default();
        let defaults_before = project.data.schematic_defaults.clone();
        if tab.general_history.apply(&mut project.data, command) {
            let defaults_moved = project.data.schematic_defaults != defaults_before;
            document.sync_project_history_dirty(project_id, true);
            self.after_project_general_change(defaults_moved);
        }
    }

    /// A General-history change landed: refresh the panel, and redraw every
    /// schematic window when the schematic defaults moved (#2026).
    fn after_project_general_change(&mut self, defaults_moved: bool) {
        if defaults_moved {
            self.clear_schematic_content_caches();
        }
        self.refresh_panel_ctx();
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
        let defaults_before = project.data.schematic_defaults.clone();
        let moved = if undo {
            tab.general_history.undo(&mut project.data)
        } else {
            tab.general_history.redo(&mut project.data)
        };
        let defaults_moved = project.data.schematic_defaults != defaults_before;
        // The boxes show the stored values again.
        tab.default_drafts.clear();
        tab.setup_draft = None;
        tab.journal_lines_draft = None;
        if moved {
            // Dirty unless the step landed back on the saved state.
            document.sync_project_history_dirty(project_id, true);
            self.after_project_general_change(defaults_moved);
        }
        moved
    }
}
