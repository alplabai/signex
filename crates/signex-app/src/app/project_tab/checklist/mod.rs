//! The Design Checklist tab (#1291, mockup row 206): a page tab of its
//! project, `<project> — Design Checklist`, that edits the checklist stored
//! in the `.snxprj`. Empty, it offers the templates; started, it shows the
//! progress per stage, the items grouped by stage with their state, owner,
//! link and the link's result, and the selected item's pane. Every edit is
//! one step of the project's General history and marks the `.snxprj` dirty;
//! nothing is written until Save. Run checks runs the checks the items link
//! to off the UI thread; a linked check that passes shows "Check passes ·
//! Mark done" and the owner ticks it. Rules: `docs/DESIGN_CHECKLIST.md`.
//! Catalog commands: `commands`.

mod commands;
mod detail;
mod tab;
mod view;
mod words;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use chrono::Utc;
use iced::Task;
use serde_json::json;
use signex_engine::ProjectChecklistCommand;
use signex_types::project::{
    ChecklistField, ChecklistLink, ChecklistProblem, ChecklistStage, ChecklistTemplate, ItemState,
    NewChecklistItem, ProjectData, RequirementEntry,
};

pub(crate) use commands::{
    project_add_checklist_item, project_remove_checklist_item, project_run_checklist,
    project_set_checklist_item, project_set_checklist_state, project_start_checklist,
};

pub use crate::app::core::checklist_tab::{ChecklistRun, ChecklistTabState};

use super::ProjectTabMsg;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::ignore::IgnoreResult;

/// Messages of the Design Checklist tab, under `ProjectTabMsg::Checklist`.
#[derive(Debug, Clone)]
pub enum ChecklistMsg {
    /// A template card clicked in the empty tab.
    TemplatePicked(ChecklistTemplate),
    /// Start from template: the picked template's items, worded now.
    StartPressed,
    /// Add item: a new open item, selected for typing its title.
    AddPressed,
    FilterPicked(Option<ItemState>),
    SearchEdited(String),
    /// A row clicked: its item opens in the pane.
    ItemSelected(String),
    DetailClosed,
    /// A field as typed, not committed yet.
    FieldEdited {
        item: String,
        field: ChecklistField,
        text: String,
    },
    /// Enter in a field's box: commit what was typed.
    FieldSubmitted {
        item: String,
        field: ChecklistField,
    },
    StagePicked {
        item: String,
        stage: ChecklistStage,
    },
    /// Open or Done, or Mark done on a passing check.
    StatePicked {
        item: String,
        state: ItemState,
    },
    /// Waived…: the reason box opens.
    WaiveStarted {
        item: String,
    },
    ReasonEdited(String),
    /// Waive with the typed reason.
    WaiveSubmitted,
    Removed {
        item: String,
    },
    RunPressed,
    RunFinished(Arc<ChecklistRun>),
}

/// `msg` for the Design Checklist tab of `project_id`.
pub(super) fn message(project_id: ProjectId, msg: ChecklistMsg) -> Message {
    Message::Project(ProjectMsg::Tab(ProjectTabMsg::Checklist {
        project_id,
        msg,
    }))
}

/// The day an edit is stamped with.
fn today() -> Option<String> {
    Some(chrono::Local::now().date_naive().to_string())
}

/// The requirement entries a run checks for `data`'s checklist: the
/// project's requirements an item links to, and the checklist's own ERC and
/// DRC checks when an item links to them (errors fail them, warnings do not).
pub(crate) fn run_entries(data: &ProjectData) -> Vec<RequirementEntry> {
    let Ok(Some(checklist)) = data.design_checklist() else {
        return Vec::new();
    };
    let links: Vec<&ChecklistLink> = checklist
        .items
        .iter()
        .filter_map(|item| item.link.as_ref())
        .collect();
    let mut entries: Vec<RequirementEntry> = data
        .requirements
        .iter()
        .filter(|entry| {
            links
                .iter()
                .any(|link| matches!(link, ChecklistLink::Requirement { id } if entry.id() == Some(id.as_str())))
        })
        .cloned()
        .collect();
    for (link, kind) in [
        (ChecklistLink::Erc, "erc_clean"),
        (ChecklistLink::Drc, "drc_clean"),
    ] {
        if links.contains(&&link)
            && let Some(id) = link.run_id()
        {
            entries.push(RequirementEntry::read(
                json!({"id": id, "kind": kind, "allow_warnings": true}),
            ));
        }
    }
    entries
}

impl Signex {
    pub(super) fn dispatch_checklist(
        &mut self,
        project_id: ProjectId,
        msg: ChecklistMsg,
    ) -> Task<Message> {
        // A late message for a closed project must not recreate its state.
        if self.document_state.project_by_id(project_id).is_none() {
            crate::diagnostics::log_warning(
                "Design Checklist: the project was closed; the edit was dropped",
            );
            return Task::none();
        }
        match msg {
            ChecklistMsg::TemplatePicked(template) => {
                self.checklist_tab(project_id).template = Some(template);
            }
            ChecklistMsg::StartPressed => {
                let template = self
                    .checklist_tab(project_id)
                    .template
                    .unwrap_or(ChecklistTemplate::ReleaseReview);
                let command = self.start_command(template);
                self.edit_checklist_from_tab(project_id, &command)
                    .ignore("the refusal is kept in the tab and shown there");
            }
            ChecklistMsg::AddPressed => self.add_item_from_tab(project_id),
            ChecklistMsg::FilterPicked(filter) => self.checklist_tab(project_id).filter = filter,
            ChecklistMsg::SearchEdited(text) => self.checklist_tab(project_id).search = text,
            ChecklistMsg::ItemSelected(item) => {
                let tab = self.checklist_tab(project_id);
                tab.selected = Some(item);
                tab.waiving = None;
                tab.problem = None;
            }
            ChecklistMsg::DetailClosed => {
                let tab = self.checklist_tab(project_id);
                tab.selected = None;
                tab.waiving = None;
            }
            ChecklistMsg::FieldEdited { item, field, text } => {
                let tab = self.checklist_tab(project_id);
                tab.drafts.insert((item, field), text);
                tab.problem = None;
            }
            ChecklistMsg::FieldSubmitted { item, field } => {
                let key = (item, field);
                let Some(text) = self.checklist_tab(project_id).drafts.get(&key).cloned() else {
                    return Task::none();
                };
                let (item, field) = key;
                self.commit_checklist_field(project_id, item, field, text);
            }
            ChecklistMsg::StagePicked { item, stage } => {
                let text = stage.key().to_string();
                self.commit_checklist_field(project_id, item, ChecklistField::Stage, text);
            }
            ChecklistMsg::StatePicked { item, state } => {
                let command = ProjectChecklistCommand::SetState {
                    item,
                    state,
                    reason: None,
                    today: today(),
                };
                self.checklist_tab(project_id).waiving = None;
                self.edit_checklist_from_tab(project_id, &command)
                    .ignore("the refusal is kept in the tab and shown there");
            }
            ChecklistMsg::WaiveStarted { item } => {
                self.checklist_tab(project_id).waiving = Some((item, String::new()));
            }
            ChecklistMsg::ReasonEdited(text) => {
                let tab = self.checklist_tab(project_id);
                if let Some((_, reason)) = tab.waiving.as_mut() {
                    *reason = text;
                }
                tab.problem = None;
            }
            ChecklistMsg::WaiveSubmitted => self.waive_from_tab(project_id),
            ChecklistMsg::Removed { item } => {
                let command = ProjectChecklistCommand::RemoveItem { item };
                if self.edit_checklist_from_tab(project_id, &command).is_ok() {
                    let tab = self.checklist_tab(project_id);
                    tab.selected = None;
                    tab.waiving = None;
                }
            }
            ChecklistMsg::RunPressed => return self.run_checklist_in_tab(project_id),
            ChecklistMsg::RunFinished(run) => {
                let tab = self.checklist_tab(project_id);
                tab.running = false;
                tab.last_run = Some(run);
            }
        }
        Task::none()
    }

    pub(super) fn checklist_tab(&mut self, project_id: ProjectId) -> &mut ChecklistTabState {
        &mut self
            .document_state
            .project_tabs
            .entry(project_id)
            .or_default()
            .checklist
    }

    /// The start of `template`, its items worded in the current language.
    pub(super) fn start_command(&self, template: ChecklistTemplate) -> ProjectChecklistCommand {
        let i18n = &self.ui_state.i18n;
        let items = template
            .items()
            .iter()
            .map(|item| NewChecklistItem {
                stage: item.stage,
                title: signex_i18n::t!(i18n, words::template_item_words(item.key)).to_string(),
                link: item.link.map(|link| link.to_link()),
            })
            .collect();
        ProjectChecklistCommand::Start {
            template: template.key().to_string(),
            items,
        }
    }

    /// Add item: a new open item in the selected item's stage (else
    /// Schematic), titled "New item" and selected, so its title is typed next.
    fn add_item_from_tab(&mut self, project_id: ProjectId) {
        let stage = self.selected_item_stage(project_id);
        let title = signex_i18n::t!(self.ui_state.i18n, signex_i18n::ids::CHECKLIST_NEW_ITEM);
        let command = ProjectChecklistCommand::AddItem {
            stage: stage.key().to_string(),
            title: title.to_string(),
            today: today(),
        };
        if self.edit_checklist_from_tab(project_id, &command).is_ok() {
            let added = self
                .document_state
                .project_by_id(project_id)
                .and_then(|project| project.data.design_checklist().ok().flatten())
                .and_then(|checklist| checklist.items.last().map(|item| item.id.clone()));
            let tab = self.checklist_tab(project_id);
            tab.selected = added;
            tab.waiving = None;
        }
    }

    /// The stage of the selected item, else Schematic.
    fn selected_item_stage(&self, project_id: ProjectId) -> ChecklistStage {
        let selected = self
            .document_state
            .project_tabs
            .get(&project_id)
            .and_then(|tab| tab.checklist.selected.clone());
        self.document_state
            .project_by_id(project_id)
            .and_then(|project| project.data.design_checklist().ok().flatten())
            .zip(selected)
            .and_then(|(checklist, id)| checklist.item(&id).map(|item| item.stage))
            .unwrap_or(ChecklistStage::Schematic)
    }

    fn commit_checklist_field(
        &mut self,
        project_id: ProjectId,
        item: String,
        field: ChecklistField,
        text: String,
    ) {
        let command = ProjectChecklistCommand::SetField {
            item: item.clone(),
            field,
            text,
            today: today(),
        };
        if self.edit_checklist_from_tab(project_id, &command).is_ok() {
            self.checklist_tab(project_id).drafts.remove(&(item, field));
        }
    }

    /// Waive the item the reason box is for, with the typed reason. A blank
    /// reason is refused and the box stays.
    fn waive_from_tab(&mut self, project_id: ProjectId) {
        let Some((item, reason)) = self.checklist_tab(project_id).waiving.clone() else {
            return;
        };
        let command = ProjectChecklistCommand::SetState {
            item,
            state: ItemState::Waived,
            reason: Some(reason),
            today: today(),
        };
        if self.edit_checklist_from_tab(project_id, &command).is_ok() {
            self.checklist_tab(project_id).waiving = None;
        }
    }

    /// Apply `command` from the tab and keep its refusal for the tab.
    fn edit_checklist_from_tab(
        &mut self,
        project_id: ProjectId,
        command: &ProjectChecklistCommand,
    ) -> Result<bool, ChecklistProblem> {
        let outcome = self.apply_project_checklist(project_id, command);
        self.checklist_tab(project_id).problem = outcome.as_ref().err().cloned();
        outcome
    }

    /// Apply one checklist edit to the project's data as one step of its
    /// General history and mark the `.snxprj` dirty: the tab's and the
    /// catalog commands' one commit path. `Ok(false)` when nothing changed
    /// or the project is no longer loaded. Drafts are keyed by item id, so
    /// an edit keeps every other item's.
    pub(crate) fn apply_project_checklist(
        &mut self,
        project_id: ProjectId,
        command: &ProjectChecklistCommand,
    ) -> Result<bool, ChecklistProblem> {
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Design Checklist: the project was closed; the edit was dropped",
            );
            return Ok(false);
        };
        let tab = document.project_tabs.entry(project_id).or_default();
        if !tab
            .general_history
            .apply_checklist(&mut project.data, command)?
        {
            return Ok(false);
        }
        document.sync_project_history_dirty(project_id, false);
        self.after_project_general_change(false);
        Ok(true)
    }

    /// Run checks: the checks the items link to, over the project as it is
    /// in memory, off the UI thread; the tab shows the result when it lands.
    /// Nothing is written and nothing reaches Messages. A run already going
    /// keeps going and this one is not started.
    pub(crate) fn run_checklist_in_tab(&mut self, project_id: ProjectId) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        if self
            .document_state
            .project_tabs
            .get(&project_id)
            .is_some_and(|tab| tab.checklist.running)
        {
            return Task::none();
        }
        let job = self.requirements_job_over(project, run_entries(&project.data), false);
        self.checklist_tab(project_id).running = true;
        Task::perform(
            async move {
                let ran = tokio::task::spawn_blocking(job)
                    .await
                    .map_err(|error| format!("the check did not finish: {error}"));
                ChecklistRun {
                    report: ran.map(|(report, _)| report),
                    finished_at: Utc::now(),
                }
            },
            move |run| message(project_id, ChecklistMsg::RunFinished(Arc::new(run))),
        )
    }
}
