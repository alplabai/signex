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
        match msg {
            ProjectTabMsg::SetupEdited { project_id, msg } => {
                return self.edit_project_setup(project_id, msg);
            }
            ProjectTabMsg::Requirements { project_id, msg } => {
                return self.dispatch_requirements(project_id, msg);
            }
            ProjectTabMsg::AddSchematicRequested { project_id } => {
                return self.add_schematic_from_tab(project_id);
            }
            ProjectTabMsg::ErcDialogRequested { project_id } => {
                return self.open_erc_from_tab(project_id);
            }
            ProjectTabMsg::VariantPartsReadRequested { project_id } => {
                return self.read_variant_parts(project_id, true);
            }
            ProjectTabMsg::PreviewActed { project_id, msg } => {
                return self.act_on_project_preview(project_id, msg);
            }
            ProjectTabMsg::LibraryLocateRequested {
                project_id,
                missing,
            } => return self.pick_located_library(project_id, missing),
            ProjectTabMsg::LibraryLocatePicked {
                project_id,
                missing,
                picked,
            } => return self.finish_library_locate(project_id, &missing, picked),
            _ => {}
        }
        // Showing the Variants section reads its parts; a variant edit that
        // changed the columns reads them again (#1293).
        let parts_read = match &msg {
            ProjectTabMsg::SectionSelected {
                project_id,
                section: ProjectSection::Variants,
            } => Some((*project_id, true)),
            ProjectTabMsg::NewVariantRequested { project_id }
            | ProjectTabMsg::VariantRenameSubmitted { project_id }
            | ProjectTabMsg::VariantRemoved { project_id, .. } => Some((*project_id, false)),
            _ => None,
        };
        // Showing Version control reads the branch and last commit.
        let repo_read = match &msg {
            ProjectTabMsg::SectionSelected {
                project_id,
                section: ProjectSection::VersionControl,
            } => Some(*project_id),
            _ => None,
        };
        self.apply_project_tab_message(msg);
        if let Some(project_id) = repo_read {
            return self.read_project_repo(project_id);
        }
        match parts_read {
            Some((project_id, force)) => self.read_variant_parts(project_id, force),
            None => iced::Task::none(),
        }
    }

    pub(super) fn apply_project_tab_message(&mut self, msg: ProjectTabMsg) {
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
            ProjectTabMsg::ParameterSubmitted { project_id } => {
                self.report_project_variable_issues(project_id);
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
                // A parameter is a variable (#1987): `${NAME}` and an
                // expression can only name an identifier.
                if !name.is_empty() && !signex_types::variables::is_variable_name(&name) {
                    crate::diagnostics::log_warning(format!(
                        "Parameter '{name}' was not added: a name is letters, digits and _, \
                         and does not start with a digit."
                    ));
                    // The refused name stays typed, to be corrected.
                    if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
                        tab.new_parameter_name = name;
                    }
                    return;
                }
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
            ProjectTabMsg::ParameterFilterEdited { project_id, text } => {
                self.edit_parameter_filter(project_id, text);
            }
            ProjectTabMsg::ParameterGroupToggled { project_id, group } => {
                self.toggle_parameter_group(project_id, group);
            }
            ProjectTabMsg::ParameterRenameEdited {
                project_id,
                from,
                text,
            } => self.edit_parameter_rename(project_id, from, text),
            ProjectTabMsg::ParameterRenameSubmitted { project_id } => {
                self.submit_parameter_rename(project_id);
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
            ProjectTabMsg::ExploreRequested { project_id } => {
                self.explore_project_folder(project_id);
            }
            // Routed by `dispatch_project_tab_message`, which returns their task.
            ProjectTabMsg::SetupEdited { .. }
            | ProjectTabMsg::Requirements { .. }
            | ProjectTabMsg::PreviewActed { .. }
            | ProjectTabMsg::VariantPartsReadRequested { .. } => {}
            // Routed by `dispatch_project_tab_message`: they open a dialog.
            ProjectTabMsg::AddSchematicRequested { .. }
            | ProjectTabMsg::ErcDialogRequested { .. }
            | ProjectTabMsg::LibraryLocateRequested { .. }
            | ProjectTabMsg::LibraryLocatePicked { .. } => {}
            ProjectTabMsg::RowDragged { project_id, msg } => {
                self.act_on_row_drag(project_id, msg);
            }
            ProjectTabMsg::SheetMoved {
                project_id,
                filename,
                step,
            } => self.move_project_sheet(project_id, filename, step),
            ProjectTabMsg::VariantRenameEdited {
                project_id,
                from,
                text,
            } => self.edit_variant_rename(project_id, from, text),
            ProjectTabMsg::VariantRenameSubmitted { project_id } => {
                self.submit_variant_rename(project_id);
            }
            ProjectTabMsg::VariantRemoved { project_id, name } => {
                self.remove_project_variant(project_id, name);
            }
            ProjectTabMsg::ActiveVariantPicked { project_id, name } => {
                self.pick_active_variant(project_id, name);
            }
            ProjectTabMsg::NewVariantRequested { project_id } => self.add_new_variant(project_id),
            ProjectTabMsg::VariantSelected { project_id, name } => {
                self.select_variant(project_id, name);
            }
            ProjectTabMsg::VariantRenameStarted { project_id, name } => {
                self.start_variant_rename(project_id, name);
            }
            ProjectTabMsg::VariantFilterEdited { project_id, text } => {
                if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
                    tab.variant_filter = text;
                }
            }
            ProjectTabMsg::VariantDifferingToggled { project_id } => {
                if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
                    tab.variants_only_differ = !tab.variants_only_differ;
                }
            }
            ProjectTabMsg::VariantPartsRead {
                project_id,
                generation,
                epoch,
                parts,
            } => self.install_variant_parts(project_id, generation, epoch, parts),
            ProjectTabMsg::LibraryMoved {
                project_id,
                path,
                step,
            } => self.move_project_library(project_id, path, step),
            ProjectTabMsg::AnnotateUndoPicked { project_id, choice } => {
                self.pick_project_annotate_undo(project_id, choice);
            }
            ProjectTabMsg::ErcRulesStoreRequested { project_id } => {
                self.store_erc_rules_in(project_id);
            }
            ProjectTabMsg::VersionControlToggled { project_id, on } => {
                self.toggle_project_version_control(project_id, on);
            }
            ProjectTabMsg::EnableVersionControlRequested { project_id } => {
                self.open_version_control_from_tab(project_id);
            }
        }
    }

    /// True when the project's `schematic_design` section is unreadable, so
    /// an edit of it is refused; Messages says why (DECISIONS 2026-11-23).
    pub(crate) fn refuse_schematic_design_edit(&mut self, project_id: ProjectId) -> bool {
        let unreadable = self
            .document_state
            .project_by_id(project_id)
            .is_some_and(|project| project.data.has_unreadable_schematic_design());
        if unreadable {
            let line = signex_i18n::t!(
                self.ui_state.i18n,
                signex_i18n::ids::DIALOG_SCHEMATIC_DESIGN_UNREADABLE
            );
            crate::diagnostics::log_warning(line.into_string());
            self.sync_diagnostics_panel_ctx();
        }
        unreadable
    }

    /// Apply one section edit (#1689-#1693) to the project's data as one
    /// step of its General history and mark the `.snxprj` dirty. `Ok(false)`
    /// when nothing changed or the project is no longer loaded; a refused
    /// variant edit changes nothing.
    pub(crate) fn apply_project_section(
        &mut self,
        project_id: ProjectId,
        command: &signex_engine::ProjectSectionCommand,
    ) -> Result<bool, signex_types::project::VariantProblem> {
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return Ok(false);
        };
        let tab = document.project_tabs.entry(project_id).or_default();
        if !tab
            .general_history
            .apply_section(&mut project.data, command)?
        {
            return Ok(false);
        }
        document.sync_project_history_dirty(project_id, true);
        // The list order is a title block's Sheet N of M: redraw the sheets.
        let pages_moved = matches!(
            command,
            signex_engine::ProjectSectionCommand::MoveSheet { .. }
                | signex_engine::ProjectSectionCommand::PlaceSheet { .. }
        );
        self.after_project_general_change(pages_moved);
        Ok(true)
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
        // A preview's open label and window belong to the section left.
        self.ui_state.project_preview = super::ProjectPreviewUi::default();
        self.ui_state.project_git.close_switcher();
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.section = section;
            tab.row_drag = None;
            // A section switch ends the typing run: the next edit is its own
            // undo step.
            tab.general_history.close_run();
            tab.default_drafts.clear();
            tab.setup_draft = None;
            tab.journal_lines_draft = None;
            tab.variant_rename = None;
            tab.variant_problem = None;
            tab.parameter_rename = None;
            tab.parameter_rename_refused = false;
            tab.requirements.clear_drafts();
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
    pub(super) fn after_project_general_change(&mut self, defaults_moved: bool) {
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
        let pages_before = project.data.sheet_order();
        let rules_before = project.data.rules.clone();
        let requirements_before = project.data.requirements.clone();
        let moved = if undo {
            tab.general_history.undo(&mut project.data)
        } else {
            tab.general_history.redo(&mut project.data)
        };
        // Redraw the sheets when their defaults or page numbers moved.
        let defaults_moved = project.data.schematic_defaults != defaults_before
            || project.data.sheet_order() != pages_before;
        // A fab tier step (#2311) puts the rules back too.
        let rules_moved = project.data.rules != rules_before;
        // The boxes show the stored values again.
        tab.default_drafts.clear();
        tab.setup_draft = None;
        tab.journal_lines_draft = None;
        tab.variant_rename = None;
        tab.variant_problem = None;
        tab.parameter_rename = None;
        tab.parameter_rename_refused = false;
        // Requirement drafts are by id: only a stepped entry's go (#2547).
        tab.requirements
            .follow_step(&requirements_before, &project.data.requirements);
        if moved {
            // Dirty unless the step landed back on the saved state.
            document.sync_project_history_dirty(project_id, true);
            if rules_moved {
                self.invalidate_pending_profile_update();
            }
            self.after_project_general_change(defaults_moved);
        }
        // A label opened on the old value must not write over the step.
        self.ui_state.project_preview.close_labels();
        moved
    }
}

impl Signex {
    /// Write the project variables whose `=` expression does not evaluate
    /// (a cycle, an unknown name) to Messages (#1987).
    fn report_project_variable_issues(&self, project_id: ProjectId) {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return;
        };
        for issue in signex_types::variables::project_variable_issues(&project.data.parameters) {
            crate::diagnostics::log_warning(format!("{}: {issue}", project.data.name));
        }
    }
}
