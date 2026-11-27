//! The Design Checklist tab (#1291, mockup row 206): a page tab of its
//! project, `<project> — Design Checklist`, that edits the checklist stored
//! in the `.snxprj`. Empty, it offers the templates (built in and the user's
//! own) and Import from another project; started, it shows the progress per
//! stage, the filter chips with their counts (Mine is the items the user
//! owns), the items grouped by stage with their state, owner, link and the
//! link's result, and the selected item's pane with its link card and
//! history. Every edit is one step of the project's General history and
//! marks the `.snxprj` dirty; nothing is written until Save. Run checks runs
//! the checks the items link to off the UI thread; a linked check that
//! passes shows "Check passes · Mark done" and the owner ticks it. Rules:
//! `docs/DESIGN_CHECKLIST.md`. Catalog commands: `commands`, `templates`,
//! `transfer` and `links`.

mod bar;
mod commands;
mod detail;
mod empty;
mod history_view;
mod link_card;
mod links;
mod run;
mod tab;
mod templates;
mod transfer;
mod view;
mod words;

#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_engine::ProjectChecklistCommand;
use signex_types::project::{
    ChecklistField, ChecklistProblem, ChecklistStage, ChecklistTemplate, EditStamp, ItemState,
    NewChecklistItem, SavedTemplate, TemplateRef,
};

pub(crate) use commands::{
    project_add_checklist_item, project_remove_checklist_item, project_run_checklist,
    project_set_checklist_item, project_set_checklist_state, project_start_checklist,
};
pub(crate) use links::project_open_checklist_link;
pub use templates::LibraryUse;
pub(crate) use templates::{
    project_checklist_templates, project_reset_checklist, project_save_checklist_template,
};
pub use transfer::Imported;
pub(crate) use transfer::{project_export_checklist_csv, project_import_checklist};

pub use crate::app::core::checklist_tab::{
    ChecklistFilter, ChecklistNotice, ChecklistRun, ChecklistTabState, SavedTemplates,
};

use super::ProjectTabMsg;
use crate::app::agent::WaitingCommand;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::ignore::IgnoreResult;

/// Messages of the Design Checklist tab, under `ProjectTabMsg::Checklist`.
#[derive(Debug, Clone)]
pub enum ChecklistMsg {
    /// A template card clicked in the empty tab.
    TemplatePicked(TemplateRef),
    /// Start from template: the picked template's items, worded now.
    StartPressed,
    /// Add item: a new open item, selected for typing its title.
    AddPressed,
    FilterPicked(ChecklistFilter),
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
    /// The link card's Run check: only this item's check.
    ItemRunPressed {
        item: String,
    },
    /// A run finished; `only` holds the run ids of a one-item run.
    RunFinished {
        run: Arc<ChecklistRun>,
        only: Option<Vec<String>>,
    },
    /// The bar's More menu opened or closed.
    MoreToggled,
    /// More ▸ Save as template…: the name box opens.
    SaveTemplateStarted,
    TemplateNameEdited(String),
    SaveTemplateCancelled,
    SaveTemplateSubmitted,
    /// The library file was written with the template `name`, or not.
    TemplateSaved {
        saved: Result<Vec<SavedTemplate>, String>,
        name: String,
        waiting: Option<WaitingCommand>,
    },
    /// The library file was read, for `then`.
    LibraryRead {
        read: Result<Vec<SavedTemplate>, String>,
        then: LibraryUse,
        waiting: Option<WaitingCommand>,
    },
    /// More ▸ Reset from template.
    ResetPressed,
    /// More ▸ Export CSV…: the save dialog opens.
    ExportPressed,
    ExportPicked(Option<PathBuf>),
    /// Import from another project…: the open dialog opens.
    ImportPressed,
    /// The picked project was read (`None`: the dialog was cancelled).
    ImportRead {
        read: Option<Result<Imported, ChecklistNotice>>,
        waiting: Option<WaitingCommand>,
    },
    /// The link card's Open, or a design object's link clicked: show what
    /// the link names.
    LinkOpenPressed {
        item: String,
    },
    /// "Link a design object or check…": the picker opens or closes.
    LinkPickerToggled {
        item: String,
    },
    LinkQueryEdited(String),
    /// A picker row clicked: the item links to `link` (its text form).
    LinkPicked {
        item: String,
        link: String,
    },
}

/// `msg` for the Design Checklist tab of `project_id`.
pub(super) fn message(project_id: ProjectId, msg: ChecklistMsg) -> Message {
    Message::Project(ProjectMsg::Tab(ProjectTabMsg::Checklist {
        project_id,
        msg,
    }))
}

impl Signex {
    /// The day and the editor an edit is stamped with: today, and the git
    /// identity once it is known.
    pub(super) fn checklist_stamp(&self) -> EditStamp {
        let day = chrono::Local::now().date_naive().to_string();
        let by = self.checklist_user();
        EditStamp::new(Some(day.as_str()), Some(by.as_str()))
    }

    /// Who "Mine" is: the git identity, empty until it is known.
    pub(super) fn checklist_user(&self) -> String {
        self.ui_state.validation_panels.identity.author()
    }

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
            ChecklistMsg::StartPressed => self.start_from_tab(project_id),
            ChecklistMsg::AddPressed => self.add_item_from_tab(project_id),
            ChecklistMsg::FilterPicked(filter) => self.checklist_tab(project_id).filter = filter,
            ChecklistMsg::SearchEdited(text) => self.checklist_tab(project_id).search = text,
            ChecklistMsg::ItemSelected(item) => {
                let tab = self.checklist_tab(project_id);
                tab.selected = Some(item);
                tab.waiving = None;
                tab.link_picker = None;
                tab.problem = None;
            }
            ChecklistMsg::DetailClosed => {
                let tab = self.checklist_tab(project_id);
                tab.selected = None;
                tab.waiving = None;
                tab.link_picker = None;
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
                    stamp: self.checklist_stamp(),
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
                    tab.link_picker = None;
                }
            }
            other => return self.dispatch_checklist_more(project_id, other),
        }
        Task::none()
    }

    /// The tab's runs, menus, files and links.
    fn dispatch_checklist_more(
        &mut self,
        project_id: ProjectId,
        msg: ChecklistMsg,
    ) -> Task<Message> {
        match msg {
            ChecklistMsg::RunPressed => self.run_checklist_in_tab(project_id, None),
            ChecklistMsg::ItemRunPressed { item } => {
                self.run_checklist_in_tab(project_id, Some(item))
            }
            ChecklistMsg::RunFinished { run, only } => {
                self.checklist_run_finished(project_id, run, only);
                Task::none()
            }
            ChecklistMsg::MoreToggled => {
                let tab = self.checklist_tab(project_id);
                tab.more_open = !tab.more_open;
                Task::none()
            }
            ChecklistMsg::SaveTemplateStarted => {
                self.start_template_name(project_id);
                Task::none()
            }
            ChecklistMsg::TemplateNameEdited(text) => {
                if let Some(name) = self.checklist_tab(project_id).template_name.as_mut() {
                    *name = text;
                }
                Task::none()
            }
            ChecklistMsg::SaveTemplateCancelled => {
                self.checklist_tab(project_id).template_name = None;
                Task::none()
            }
            ChecklistMsg::SaveTemplateSubmitted => self.save_template_from_tab(project_id),
            ChecklistMsg::TemplateSaved {
                saved,
                name,
                waiting,
            } => self.checklist_template_saved(project_id, saved, name, waiting),
            ChecklistMsg::LibraryRead {
                read,
                then,
                waiting,
            } => self.checklist_library_read(project_id, read, then, waiting),
            ChecklistMsg::ResetPressed => {
                self.reset_from_tab(project_id);
                Task::none()
            }
            ChecklistMsg::ExportPressed => self.export_csv_dialog(project_id),
            ChecklistMsg::ExportPicked(path) => match path {
                Some(path) => self.write_checklist_csv(project_id, path),
                None => Task::none(),
            },
            ChecklistMsg::ImportPressed => self.import_dialog(project_id),
            ChecklistMsg::ImportRead { read, waiting } => match read {
                Some(read) => self.checklist_import_read(project_id, read, waiting),
                None => Task::none(),
            },
            ChecklistMsg::LinkOpenPressed { item } => self.open_checklist_link(project_id, &item).1,
            ChecklistMsg::LinkPickerToggled { item } => {
                let tab = self.checklist_tab(project_id);
                let open = tab.link_picker.as_deref() == Some(item.as_str());
                tab.link_picker = (!open).then_some(item);
                tab.link_query.clear();
                Task::none()
            }
            ChecklistMsg::LinkQueryEdited(text) => {
                self.checklist_tab(project_id).link_query = text;
                Task::none()
            }
            ChecklistMsg::LinkPicked { item, link } => {
                self.checklist_tab(project_id).link_picker = None;
                self.commit_checklist_field(project_id, item, ChecklistField::Link, link);
                Task::none()
            }
            // The edits above route here only by a router mistake.
            ChecklistMsg::TemplatePicked(_)
            | ChecklistMsg::StartPressed
            | ChecklistMsg::AddPressed
            | ChecklistMsg::FilterPicked(_)
            | ChecklistMsg::SearchEdited(_)
            | ChecklistMsg::ItemSelected(_)
            | ChecklistMsg::DetailClosed
            | ChecklistMsg::FieldEdited { .. }
            | ChecklistMsg::FieldSubmitted { .. }
            | ChecklistMsg::StagePicked { .. }
            | ChecklistMsg::StatePicked { .. }
            | ChecklistMsg::WaiveStarted { .. }
            | ChecklistMsg::ReasonEdited(_)
            | ChecklistMsg::WaiveSubmitted
            | ChecklistMsg::Removed { .. } => Task::none(),
        }
    }

    pub(super) fn checklist_tab(&mut self, project_id: ProjectId) -> &mut ChecklistTabState {
        &mut self
            .document_view
            .project_tabs
            .entry(project_id)
            .or_default()
            .checklist
    }

    /// The tab's state of `project_id`, when it has any.
    pub(super) fn checklist_tab_ref(&self, project_id: ProjectId) -> Option<&ChecklistTabState> {
        self.document_view
            .project_tabs
            .get(&project_id)
            .map(|tab| &tab.checklist)
    }

    /// The start of `template`, its items worded in the current language.
    pub(super) fn start_command(&self, template: ChecklistTemplate) -> ProjectChecklistCommand {
        ProjectChecklistCommand::Start {
            template: Some(template.key().to_string()),
            items: self.template_items(template),
            stamp: self.checklist_stamp(),
        }
    }

    /// The items of the built-in `template`, worded in the current language.
    pub(super) fn template_items(&self, template: ChecklistTemplate) -> Vec<NewChecklistItem> {
        let i18n = &self.ui_state.i18n;
        template
            .items()
            .iter()
            .map(|item| NewChecklistItem {
                stage: item.stage,
                title: signex_i18n::t!(i18n, words::template_item_words(item.key)).to_string(),
                link: item.link.map(|link| link.to_link()),
            })
            .collect()
    }

    /// Start from template: the picked one, Release review by default. A
    /// saved template is started from the list the tab read.
    fn start_from_tab(&mut self, project_id: ProjectId) {
        let picked = self
            .checklist_tab(project_id)
            .template
            .clone()
            .unwrap_or(TemplateRef::Builtin(ChecklistTemplate::ReleaseReview));
        let command = match picked {
            TemplateRef::Builtin(template) => self.start_command(template),
            TemplateRef::Saved(name) => {
                let tab = self.checklist_tab(project_id);
                let Some(saved) = tab.saved.list().iter().find(|t| t.name == name).cloned() else {
                    tab.problem = Some(ChecklistProblem::UnknownSavedTemplate(name));
                    return;
                };
                self.saved_start_command(&saved)
            }
        };
        self.edit_checklist_from_tab(project_id, &command)
            .ignore("the refusal is kept in the tab and shown there");
    }

    /// Add item: a new open item in the selected item's stage (else
    /// Schematic), titled "New item" and selected, so its title is typed next.
    fn add_item_from_tab(&mut self, project_id: ProjectId) {
        let stage = self.selected_item_stage(project_id);
        let title = signex_i18n::t!(self.ui_state.i18n, signex_i18n::ids::CHECKLIST_NEW_ITEM);
        let command = ProjectChecklistCommand::AddItem {
            stage: stage.key().to_string(),
            title: title.to_string(),
            stamp: self.checklist_stamp(),
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
            .checklist_tab_ref(project_id)
            .and_then(|tab| tab.selected.clone());
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
            stamp: self.checklist_stamp(),
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
            stamp: self.checklist_stamp(),
        };
        if self.edit_checklist_from_tab(project_id, &command).is_ok() {
            self.checklist_tab(project_id).waiving = None;
        }
    }

    /// Apply `command` from the tab and keep its refusal for the tab.
    pub(super) fn edit_checklist_from_tab(
        &mut self,
        project_id: ProjectId,
        command: &ProjectChecklistCommand,
    ) -> Result<bool, ChecklistProblem> {
        let outcome = self.apply_project_checklist(project_id, command);
        let tab = self.checklist_tab(project_id);
        tab.problem = outcome.as_ref().err().cloned();
        tab.notice = None;
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
        let history = document.general_history.entry(project_id).or_default();
        if !history.apply_checklist(&mut project.data, command)? {
            return Ok(false);
        }
        document.sync_project_history_dirty(project_id, false);
        self.after_project_general_change(false);
        Ok(true)
    }
}
