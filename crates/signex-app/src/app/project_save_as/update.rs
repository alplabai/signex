//! The Save Project As dialog's update: opening it, its edits, the Save that
//! writes the copy off the update thread, and opening the copy.

use std::path::PathBuf;

use iced::Task;
use iced::widget::operation;

use super::plan::plan_copy;
use super::write::write_copy;
use super::{SaveProjectAsMsg, SaveProjectAsState};
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::handlers::document_files::new_project::project_file_in;
use crate::app::new_project::{TargetProblem, TargetStatus, check_name, check_target};
use crate::app::state::{NoticeAction, ProjectId};
use crate::app::{Message, Signex};

/// Widget id of the name field, focused when the dialog opens.
pub(crate) static NAME_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.save_project_as.name"));

impl Signex {
    pub(crate) fn dispatch_save_project_as_message(
        &mut self,
        msg: SaveProjectAsMsg,
    ) -> Task<Message> {
        use SaveProjectAsMsg as M;
        match msg {
            M::Requested { project } => self.open_save_project_as(project),
            M::Written {
                project_path,
                result,
            } => self.save_project_as_written(&project_path, result),
            _ if self.ui_state.save_project_as.is_none() => Task::none(),
            M::NameEdited(name) => {
                self.with_save_as(|s| s.name = name);
                self.recheck_save_as_target()
            }
            M::LocationEdited(location) => {
                self.with_save_as(|s| s.location = location);
                self.recheck_save_as_target()
            }
            M::LocationPicked(Some(path)) => {
                self.with_save_as(|s| s.location = path.display().to_string());
                self.recheck_save_as_target()
            }
            M::LocationPicked(None) => Task::none(),
            M::BrowseRequested => self.browse_save_as_location(),
            M::TargetChecked { generation, status } => {
                self.with_save_as(|s| {
                    if s.generation == generation {
                        s.target = status;
                    }
                });
                Task::none()
            }
            M::SubmitRequested => self.save_project_copy(),
            // While the copy is written, Cancel and Esc wait: it is written
            // and opened anyway.
            M::CancelRequested
                if self
                    .ui_state
                    .save_project_as
                    .as_ref()
                    .is_some_and(|s| s.saving.is_some()) =>
            {
                crate::diagnostics::log_info(
                    "Save Project As: cancel ignored, the copy is still being written",
                );
                Task::none()
            }
            M::CancelRequested => {
                self.ui_state.save_project_as = None;
                Task::none()
            }
        }
    }

    fn with_save_as(&mut self, edit: impl FnOnce(&mut SaveProjectAsState)) {
        if let Some(state) = self.ui_state.save_project_as.as_deref_mut() {
            edit(state);
        }
    }

    /// The project Save Project As copies: `project` while it is open, else
    /// the active document's, else the first one.
    fn save_as_project_for(&self, project: Option<ProjectId>) -> Option<ProjectId> {
        let docs = &self.document_state;
        self.rules_project_for(project)
            .or_else(|| docs.projects.first().map(|p| p.id))
    }

    /// Open the dialog on `project`. Writes nothing. A Save Failed card that
    /// offered it closes.
    pub(crate) fn open_save_project_as(&mut self, project: Option<ProjectId>) -> Task<Message> {
        let offered = matches!(
            self.document_state
                .error_notice
                .as_ref()
                .and_then(|n| n.action.as_ref()),
            Some(NoticeAction::SaveProjectAs { .. })
        );
        if offered {
            self.document_state.error_notice = None;
        }
        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;
        if self.ui_state.save_project_as.is_some() {
            return operation::focus(NAME_INPUT_ID.clone());
        }
        let Some(id) = self.save_as_project_for(project) else {
            crate::diagnostics::log_info("Save Project As: no project is open");
            return Task::none();
        };
        let Some(loaded) = self.document_state.project_by_id(id) else {
            return Task::none();
        };
        let dir = loaded.dir().to_path_buf();
        let unsaved = self.unsaved_in_copy(&loaded.data, &dir).len();
        let name = self.ui_state.i18n.message_plain(
            signex_i18n::ids::DIALOG_SAVE_PROJECT_AS_DEFAULT_NAME,
            &[("name", loaded.data.name.as_str().into())],
        );
        let location = dir
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let state = SaveProjectAsState {
            project: id,
            project_name: loaded.data.name.clone(),
            name,
            location,
            read_only_format: loaded.data.unknown.newer_minor_source.clone(),
            unsaved,
            target: TargetStatus::Unchecked,
            generation: 0,
            saving: None,
            error: None,
        };
        self.ui_state.save_project_as = Some(Box::new(state));
        Task::batch([
            operation::focus(NAME_INPUT_ID.clone()),
            operation::text_input::select_all(NAME_INPUT_ID.clone()),
            self.recheck_save_as_target(),
        ])
    }

    /// Ask the blocking pool whether the copy fits where the dialog points.
    fn recheck_save_as_target(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.save_project_as.as_deref_mut() else {
            return Task::none();
        };
        state.generation += 1;
        state.target = TargetStatus::Unchecked;
        // The name or location changed: a reason given for the old target no
        // longer applies.
        state.error = None;
        if check_name(&state.name).is_err() {
            return Task::none();
        }
        let generation = state.generation;
        let (name, location) = (state.name.clone(), state.location.clone());
        Task::perform(
            on_blocking_pool(move || Ok(check_target(&name, &location, true))),
            move |result| {
                let status =
                    result.unwrap_or_else(|why| TargetStatus::Problem(TargetProblem::Other(why)));
                Message::from(SaveProjectAsMsg::TargetChecked { generation, status })
            },
        )
    }

    fn browse_save_as_location(&mut self) -> Task<Message> {
        let start = self
            .ui_state
            .save_project_as
            .as_deref()
            .and_then(SaveProjectAsState::location_path);
        let base = self.ui_state.io.picker.dialog().title_in(
            &self.ui_state.i18n,
            signex_i18n::ids::DIALOG_NEW_PROJECT_PICK_LOCATION_TITLE,
        );
        Task::perform(
            async move {
                let mut dialog = base;
                if let Some(dir) = start.filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                dialog.pick_folder().await
            },
            |picked| Message::from(SaveProjectAsMsg::LocationPicked(picked)),
        )
    }

    /// Save: plan the copy from the open project, its unsaved edits encoded
    /// from memory now, and write it off-thread. The originals stay unsaved.
    /// The dialog stays, its Save off, until the writes land. Ignored while
    /// Save is unavailable.
    fn save_project_copy(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.save_project_as.as_deref() else {
            return Task::none();
        };
        if !state.can_save() {
            return Task::none();
        }
        let (Ok(name), Some(location)) = (check_name(&state.name), state.location_path()) else {
            return Task::none();
        };
        let name = name.to_string();
        let planned = match self.document_state.project_by_id(state.project) {
            Some(project) => {
                self.unsaved_copies(&project.data, project.dir())
                    .and_then(|unsaved| {
                        plan_copy(&project.data, project.dir(), &name, &location, unsaved)
                    })
            }
            None => Err(format!("'{}' is no longer open", state.project_name)),
        };
        let plan = match planned {
            Ok(plan) => plan,
            Err(why) => {
                self.with_save_as(|s| s.error = Some(why));
                return Task::none();
            }
        };
        let project_path = project_file_in(&location, &name, true);
        self.with_save_as(|s| {
            s.saving = Some(project_path.clone());
            s.error = None;
        });
        Task::perform(on_blocking_pool(move || write_copy(&plan)), move |result| {
            Message::from(SaveProjectAsMsg::Written {
                project_path,
                result,
            })
        })
    }

    /// The copy's writes landed: on success the dialog closes and the copy
    /// opens; a failure keeps the dialog with the reason.
    fn save_project_as_written(
        &mut self,
        project_path: &std::path::Path,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let ours = self
            .ui_state
            .save_project_as
            .as_ref()
            .is_some_and(|s| s.saving.as_deref() == Some(project_path));
        let written = match result {
            Ok(written) => written,
            Err(why) => {
                crate::diagnostics::log_error(
                    "Save Project As: the copy was not saved",
                    &anyhow::anyhow!(why.clone()),
                );
                if ours {
                    self.with_save_as(|s| {
                        s.saving = None;
                        s.error = Some(why);
                    });
                }
                return Task::none();
            }
        };
        if ours {
            self.ui_state.save_project_as = None;
        }
        crate::diagnostics::log_info(format!(
            "Save Project As: the copy was written to {}",
            written.display()
        ));
        match self.open_document_path(written) {
            Ok(task) => {
                self.refresh_panel_ctx();
                task
            }
            Err(error) => {
                self.report_open_failure(format!("{error:#}"));
                Task::none()
            }
        }
    }
}
