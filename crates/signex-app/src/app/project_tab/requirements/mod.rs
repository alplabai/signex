//! Requirements section of the Project tab (#2339): the project's
//! requirements list with each entry's kind, fields and on/off switch; add
//! (kind picker), edit a field in place (Enter commits; a refused value stays
//! in its box with the reason; the id shows read-only and only the catalog
//! command changes it), remove, and Run checks, which runs the same
//! job as `check_requirements` off the UI thread and shows pass or fail per
//! requirement with its evidence. Every edit is one step of the project's
//! General history and marks the `.snxprj` dirty; nothing is written until
//! Save. Catalog commands: `commands`.

mod commands;
mod drafts;
mod view;
mod words;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use iced::Task;
use signex_engine::ProjectRequirementCommand;
use signex_types::check::CheckReport;
use signex_types::project::{RequirementField, RequirementProblem};

pub(crate) use commands::{
    project_add_requirement, project_enable_requirement, project_remove_requirement,
    project_run_requirements, project_set_requirement,
};
pub use drafts::EntryKey;
use drafts::key_at;
pub(super) use view::view;

use super::ProjectTabMsg;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::ignore::IgnoreResult;

/// One run of the checks from the tab: what it found and when it finished.
#[derive(Debug, Clone)]
pub struct RequirementsRun {
    pub report: Result<CheckReport, String>,
    /// The board outline's width and height in mm as the run measured them,
    /// for the Board size cards; `None` when no board was read or it has no
    /// outline.
    pub board_mm: Option<(f64, f64)>,
    pub finished_at: DateTime<Utc>,
}

/// The Requirements section's own state: what is typed and not committed,
/// the last refusal, the cards opened or closed by hand, and the last run.
#[derive(Debug, Clone, Default)]
pub struct RequirementsTabState {
    /// Fields as typed, by entry and field, until Enter commits them, the
    /// entry is removed, an undo or redo changes it, or a section switch
    /// shows the stored values again (#2547, `drafts`).
    pub drafts: HashMap<(EntryKey, RequirementField), String>,
    /// Why the last edit was refused, and the entry it was for (`None`: an
    /// add); cleared by the next edit.
    pub problem: Option<(Option<EntryKey>, RequirementProblem)>,
    /// The field the refused value was typed in, when the refusal was a
    /// field commit; cleared with `problem`.
    pub problem_field: Option<RequirementField>,
    /// Cards whose open or closed state the user flipped, by entry: a flip
    /// follows its entry like the drafts, so it never lands on another.
    pub toggled: HashSet<EntryKey>,
    pub running: bool,
    pub last_run: Option<Arc<RequirementsRun>>,
}

/// How many of `entries` failed the last run that finished (none before a
/// run, or when the run itself could not finish).
pub(super) fn failed_count(
    entries: &[signex_types::project::RequirementEntry],
    state: Option<&RequirementsTabState>,
) -> usize {
    state
        .and_then(|state| state.last_run.as_deref())
        .and_then(|run| run.report.as_ref().ok())
        .map_or(0, |report| view::outcome_counts(report, entries).failed)
}

impl RequirementsTabState {
    /// Forget what was typed: the boxes show the stored values again.
    pub(super) fn clear_drafts(&mut self) {
        self.drafts.clear();
        self.problem = None;
        self.problem_field = None;
        self.toggled.clear();
    }
}

/// Messages of the Requirements section, under `ProjectTabMsg::Requirements`.
#[derive(Debug, Clone)]
pub enum RequirementsMsg {
    /// A kind picked in the Add picker: a new requirement of that kind.
    AddPicked {
        kind: &'static str,
    },
    /// A field as typed, not committed yet.
    FieldEdited {
        index: usize,
        field: RequirementField,
        text: String,
    },
    /// Enter in a field's box: commit what was typed.
    FieldSubmitted {
        index: usize,
        field: RequirementField,
    },
    /// A pick (severity, a checkbox): committed at once.
    FieldPicked {
        index: usize,
        field: RequirementField,
        value: String,
    },
    EnabledToggled {
        index: usize,
        on: bool,
    },
    Removed {
        index: usize,
    },
    /// The card's expand or collapse button: flip its open state.
    ExpandToggled {
        index: usize,
    },
    RunRequested,
    RunFinished(Arc<RequirementsRun>),
}

/// `msg` for the Requirements section of `project_id`.
pub(super) fn message(project_id: ProjectId, msg: RequirementsMsg) -> Message {
    Message::Project(ProjectMsg::Tab(ProjectTabMsg::Requirements {
        project_id,
        msg,
    }))
}

impl Signex {
    pub(super) fn dispatch_requirements(
        &mut self,
        project_id: ProjectId,
        msg: RequirementsMsg,
    ) -> Task<Message> {
        // A late message for a closed project must not recreate its state.
        if self.document_state.project_by_id(project_id).is_none() {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the requirements edit was dropped",
            );
            return Task::none();
        }
        match msg {
            RequirementsMsg::AddPicked { kind } => {
                let command = ProjectRequirementCommand::Add {
                    kind: kind.to_string(),
                    id: None,
                };
                self.edit_requirements_from_tab(project_id, None, &command)
                    .ignore("the refusal is kept in the tab and shown at its entry");
            }
            RequirementsMsg::FieldEdited { index, field, text } => {
                let Some(requirement) = self.requirement_key_at(project_id, index) else {
                    return Task::none();
                };
                let tab = self.requirements_tab(project_id);
                tab.drafts.insert((requirement, field), text);
                tab.problem = None;
                tab.problem_field = None;
            }
            RequirementsMsg::FieldSubmitted { index, field } => {
                // The draft of the entry at `index` now: never another's.
                let Some(requirement) = self.requirement_key_at(project_id, index) else {
                    return Task::none();
                };
                let Some(text) = self
                    .requirements_tab(project_id)
                    .drafts
                    .get(&(requirement, field))
                else {
                    return Task::none();
                };
                let text = text.clone();
                self.commit_requirement_field(project_id, index, field, text);
            }
            RequirementsMsg::FieldPicked {
                index,
                field,
                value,
            } => self.commit_requirement_field(project_id, index, field, value),
            RequirementsMsg::EnabledToggled { index, on } => {
                let command = ProjectRequirementCommand::SetEnabled { index, on };
                self.edit_requirements_from_tab(project_id, Some(index), &command)
                    .ignore("the refusal is kept in the tab and shown at its entry");
            }
            RequirementsMsg::Removed { index } => {
                let command = ProjectRequirementCommand::Remove { index };
                self.edit_requirements_from_tab(project_id, Some(index), &command)
                    .ignore("the refusal is kept in the tab and shown at its entry");
            }
            RequirementsMsg::ExpandToggled { index } => {
                let Some(requirement) = self.requirement_key_at(project_id, index) else {
                    return Task::none();
                };
                let toggled = &mut self.requirements_tab(project_id).toggled;
                if !toggled.remove(&requirement) {
                    toggled.insert(requirement);
                }
            }
            RequirementsMsg::RunRequested => return self.run_requirements_in_tab(project_id),
            RequirementsMsg::RunFinished(run) => {
                let tab = self.requirements_tab(project_id);
                tab.running = false;
                tab.last_run = Some(run);
            }
        }
        Task::none()
    }

    fn requirements_tab(&mut self, project_id: ProjectId) -> &mut RequirementsTabState {
        &mut self
            .document_state
            .project_tabs
            .entry(project_id)
            .or_default()
            .requirements
    }

    /// The key of the requirement at `index` of project `project_id`.
    fn requirement_key_at(&self, project_id: ProjectId, index: usize) -> Option<EntryKey> {
        let project = self.document_state.project_by_id(project_id)?;
        key_at(&project.data.requirements, index)
    }

    fn commit_requirement_field(
        &mut self,
        project_id: ProjectId,
        index: usize,
        field: RequirementField,
        text: String,
    ) {
        let command = ProjectRequirementCommand::SetField { index, field, text };
        let committed = self
            .edit_requirements_from_tab(project_id, Some(index), &command)
            .is_ok();
        if committed {
            // Read after the commit: a committed id moved the entry's drafts.
            if let Some(requirement) = self.requirement_key_at(project_id, index) {
                self.requirements_tab(project_id)
                    .drafts
                    .remove(&(requirement, field));
            }
        } else {
            self.requirements_tab(project_id).problem_field = Some(field);
        }
    }

    /// Apply `command` from the tab and keep its refusal for the entry it
    /// was for.
    fn edit_requirements_from_tab(
        &mut self,
        project_id: ProjectId,
        index: Option<usize>,
        command: &ProjectRequirementCommand,
    ) -> Result<bool, RequirementProblem> {
        let outcome = self.apply_project_requirement(project_id, command);
        // A refused edit left the list as it was: `index` still names it.
        let entry = index.and_then(|index| self.requirement_key_at(project_id, index));
        let tab = self.requirements_tab(project_id);
        tab.problem = outcome
            .as_ref()
            .err()
            .map(|problem| (entry, problem.clone()));
        tab.problem_field = None;
        outcome
    }

    /// Apply one Requirements edit to the project's data as one step of its
    /// General history and mark the `.snxprj` dirty: the tab's and the
    /// catalog commands' one commit path. `Ok(false)` when nothing changed
    /// or the project is no longer loaded. Drafts and opened or closed
    /// cards are keyed by requirement id: an edit from the tab or a command
    /// keeps every other entry's.
    pub(crate) fn apply_project_requirement(
        &mut self,
        project_id: ProjectId,
        command: &ProjectRequirementCommand,
    ) -> Result<bool, RequirementProblem> {
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return Ok(false);
        };
        let tab = document.project_tabs.entry(project_id).or_default();
        let before = project.data.requirements.clone();
        if !tab
            .general_history
            .apply_requirement(&mut project.data, command)?
        {
            return Ok(false);
        }
        let removed = match command {
            ProjectRequirementCommand::Remove { index } => Some(*index),
            _ => None,
        };
        tab.requirements
            .follow_edit(&before, &project.data.requirements, removed);
        document.sync_project_history_dirty(project_id, true);
        self.after_project_general_change(false);
        Ok(true)
    }

    /// Run checks: the `check_requirements` job over the project as it is in
    /// memory, off the UI thread; the tab shows the result when it lands.
    /// Nothing is written and nothing reaches Messages.
    pub(crate) fn run_requirements_in_tab(&mut self, project_id: ProjectId) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        if self
            .document_state
            .project_tabs
            .get(&project_id)
            .is_some_and(|tab| tab.requirements.running)
        {
            return Task::none();
        }
        let job = self.requirements_measured_check_job(project);
        self.requirements_tab(project_id).running = true;
        Task::perform(
            async move {
                let ran = tokio::task::spawn_blocking(job)
                    .await
                    .map_err(|error| format!("the check did not finish: {error}"));
                let board_mm = ran.as_ref().ok().and_then(|(_, measured)| *measured);
                RequirementsRun {
                    report: ran.map(|(report, _)| report),
                    board_mm,
                    finished_at: Utc::now(),
                }
            },
            move |run| message(project_id, RequirementsMsg::RunFinished(Arc::new(run))),
        )
    }
}
