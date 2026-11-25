//! A project read from a newer `.snxprj` minor opens read-only (#2904,
//! DECISIONS 2026-11-25): the refusals that stop its writes before anything
//! on disk moves, and what the close and quit prompts offer for it.

use std::path::{Path, PathBuf};

use signex_i18n::{I18n, Localized, Translatable, ids, t};
use signex_types::format_version::FormatVersionError;

use super::state::{ErrorNotice, ProjectId};
use super::{AppQuitConfirmState, Message, ProjectCloseChoice, ProjectCloseConfirmState, Signex};
use crate::library::modal_chrome::UnsavedActions;

/// A save of a read-only project, refused before anything was written. Its
/// error chain is the path line over the format error.
#[derive(Debug)]
pub(crate) struct ReadOnlyProjectSave {
    pub project: String,
    pub path: PathBuf,
    pub source: FormatVersionError,
}

impl std::fmt::Display for ReadOnlyProjectSave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is read-only", self.path.display())
    }
}

impl std::error::Error for ReadOnlyProjectSave {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl ReadOnlyProjectSave {
    /// The Save Failed card's sentence for this refusal.
    pub(crate) fn detail(&self) -> Translatable {
        Translatable::new(ids::NOTICE_PROJECT_READ_ONLY_SAVE)
            .arg("project", self.project.clone())
            .arg(
                "format",
                self.source.found().unwrap_or_default().to_string(),
            )
    }

    /// The refusal inside `error`'s chain, if the save failed on one.
    pub(crate) fn find(error: &anyhow::Error) -> Option<&Self> {
        error.chain().find_map(|cause| cause.downcast_ref::<Self>())
    }
}

/// The Save Failed card's sentence for a save that failed with `error`:
/// a read-only project's own sentence, else the caller's `context`.
pub(crate) fn save_failure_detail(context: &str, error: &anyhow::Error) -> Translatable {
    ReadOnlyProjectSave::find(error).map_or_else(
        || crate::app::state::untranslated(context),
        ReadOnlyProjectSave::detail,
    )
}

/// What the close and quit prompts offer while read-only projects have
/// unsaved settings: the discard names those projects and drops only their
/// settings, and Save All is offered while anything else can be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadOnlyOffer {
    /// The read-only projects with unsaved settings, in project order.
    pub projects: Vec<String>,
    /// Some other file or row of the prompt is unsaved, so Save All saves it.
    pub saveable: bool,
}

/// A change of a read-only project's file list, refused before any file is
/// written, copied, moved or deleted.
pub(crate) enum FileListChange<'a> {
    /// Add New Schematic, creating the sheet at this path.
    AddNew(&'a Path),
    /// Add Existing, copying these files in.
    AddExisting(&'a [PathBuf]),
    /// Remove from Project (`delete: false`) or Delete of the file named.
    Remove { name: &'a str, delete: bool },
}

impl Signex {
    /// The refusal of a write to project `project_id`'s `.snxprj`: `Some`
    /// when the project was read from a newer minor.
    pub(crate) fn read_only_project_save(
        &self,
        project_id: ProjectId,
    ) -> Option<ReadOnlyProjectSave> {
        let project = self.document_state.project_by_id(project_id)?;
        let source = project.data.refuse_save().err()?;
        Some(ReadOnlyProjectSave {
            project: project.data.name.clone(),
            path: project.path.clone(),
            source,
        })
    }

    /// The names of the read-only projects whose unsaved `.snxprj` is in
    /// `dirty`, in project order: a Save All could never write them.
    pub(crate) fn read_only_dirty_projects(&self, dirty: &[PathBuf]) -> Vec<String> {
        self.read_only_dirty(dirty)
            .into_iter()
            .map(|p| p.data.name.clone())
            .collect()
    }

    /// The `.snxprj` paths of those projects, as listed in `dirty`.
    pub(crate) fn read_only_dirty_paths(&self, dirty: &[PathBuf]) -> Vec<PathBuf> {
        self.read_only_dirty(dirty)
            .into_iter()
            .map(|p| p.path.clone())
            .collect()
    }

    /// The read-only projects whose `.snxprj` is in `dirty`, in project order.
    fn read_only_dirty(&self, dirty: &[PathBuf]) -> Vec<&crate::app::state::LoadedProject> {
        self.document_state
            .projects
            .iter()
            .filter(|p| p.data.is_from_newer_minor() && dirty.contains(&p.path))
            .collect()
    }

    /// What a close or quit prompt listing `dirty` files and `rows` rows
    /// offers for its read-only projects; `None` when it lists none.
    pub(crate) fn read_only_offer(&self, dirty: &[PathBuf], rows: usize) -> Option<ReadOnlyOffer> {
        let paths = self.read_only_dirty_paths(dirty);
        if paths.is_empty() {
            return None;
        }
        Some(ReadOnlyOffer {
            projects: self.read_only_dirty_projects(dirty),
            saveable: rows > 0 || dirty.iter().any(|p| !paths.contains(p)),
        })
    }

    /// Whether the prompt listing `dirty` and `rows` offers Save All: not
    /// when only read-only projects' settings are unsaved.
    pub(crate) fn save_all_offered(&self, dirty: &[PathBuf], rows: usize) -> bool {
        self.read_only_offer(dirty, rows)
            .is_none_or(|offer| offer.saveable)
    }

    /// The files of `dirty` a Save All writes: all but the read-only
    /// projects' `.snxprj`, which are never written. A skip is noted in
    /// Messages under `context`.
    pub(crate) fn saveable_paths(&self, context: &str, dirty: &[PathBuf]) -> Vec<PathBuf> {
        let skipped = self.read_only_dirty_paths(dirty);
        if !skipped.is_empty() {
            let names = quoted(&self.read_only_dirty_projects(dirty));
            crate::diagnostics::log_warning(format!(
                "{context}: Save All skips {names}: it opens read-only, so its settings are not \
                 saved and stay unsaved"
            ));
        }
        dirty
            .iter()
            .filter(|p| !skipped.contains(p))
            .cloned()
            .collect()
    }

    /// The rename dialog's refusal for a file of the project at `project_idx`
    /// when that project is read-only: the rename also rewrites the saved
    /// `.snxprj`, so it is refused before the file moves.
    pub(crate) fn read_only_project_rename_refusal(
        &self,
        project_idx: Option<usize>,
    ) -> Option<String> {
        let id = self.document_state.projects.get(project_idx?)?.id;
        let refusal = self.read_only_project_save(id)?;
        let text = t!(
            self.ui_state.i18n.as_ref(),
            ids::NOTICE_PROJECT_READ_ONLY_RENAME,
            "project" => refusal.project.clone(),
            "format" => refusal.source.found().unwrap_or_default().to_string()
        )
        .into_string();
        crate::diagnostics::log_warning(format!("Rename refused: {}: {}", refusal, refusal.source));
        Some(text)
    }

    /// Add New Schematic in a read-only project: refused before the blank
    /// sheet is written, with a card naming why. True when refused.
    pub(crate) fn refuse_add_in_read_only_project(
        &mut self,
        project_id: ProjectId,
        path: &Path,
    ) -> bool {
        self.refuse_file_list_change(project_id, FileListChange::AddNew(path))
    }

    /// Remove from Project or Delete (`delete`) of the dialog's file when its
    /// project is read-only: refused before the unsaved-edits prompt and
    /// before anything is excluded, moved or deleted. True when refused.
    pub(crate) fn refuse_remove_in_read_only_project(
        &mut self,
        state: &crate::app::RemoveDialogState,
        delete: bool,
    ) -> bool {
        let project = state.project_id.or_else(|| {
            let idx = *state.tree_path.first()?;
            self.document_state.projects.get(idx).map(|p| p.id)
        });
        let Some(project) = project else {
            return false;
        };
        let name = state.display_name.as_str();
        self.refuse_file_list_change(project, FileListChange::Remove { name, delete })
    }

    /// A change of project `project_id`'s file list while it is read-only:
    /// refused before any file on disk changes, with a card naming why and a
    /// Messages line. True when refused.
    pub(crate) fn refuse_file_list_change(
        &mut self,
        project_id: ProjectId,
        change: FileListChange<'_>,
    ) -> bool {
        let Some(refusal) = self.read_only_project_save(project_id) else {
            return false;
        };
        type Notice = fn(Translatable) -> ErrorNotice;
        // The Messages ring clips a line: the file names go first.
        let (line, detail, notice): (String, Translatable, Notice) = match change {
            FileListChange::AddNew(path) => {
                let file = file_name(path);
                (
                    format!("Add New Schematic refused: {file} was not created"),
                    Translatable::new(ids::NOTICE_PROJECT_READ_ONLY_ADD).arg("file", file),
                    ErrorNotice::create,
                )
            }
            FileListChange::AddExisting(paths) => {
                let files: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
                let files = files.join(", ");
                (
                    format!("Add Existing refused: {files} not added"),
                    Translatable::new(ids::NOTICE_PROJECT_READ_ONLY_ADD_EXISTING)
                        .arg("count", paths.len())
                        .arg("files", files),
                    ErrorNotice::create,
                )
            }
            FileListChange::Remove { name, delete } => (
                format!(
                    "{} refused: {name} was not removed",
                    if delete {
                        "Delete"
                    } else {
                        "Remove from Project"
                    }
                ),
                Translatable::new(ids::NOTICE_PROJECT_READ_ONLY_REMOVE).arg("file", name),
                if delete {
                    ErrorNotice::delete
                } else {
                    ErrorNotice::exclude_refused
                },
            ),
        };
        crate::diagnostics::log_warning(format!(
            "{line}; project '{}' is read-only ({})",
            refusal.project, refusal.source
        ));
        let detail = detail.arg("project", refusal.project.clone());
        self.document_state.error_notice =
            Some(notice(detail).with_system(refusal.source.to_string()));
        true
    }

    /// The Close Project choice, after refusing what the prompt does not
    /// offer while a read-only project's settings are unsaved: Discard All,
    /// and Save All when nothing else saves. A refused choice leaves the
    /// prompt up.
    pub(crate) fn handle_project_close_choice(
        &mut self,
        choice: ProjectCloseChoice,
    ) -> iced::Task<Message> {
        let Some(state) = &self.ui_state.project_close_confirm else {
            return self.handle_project_close_confirm(choice);
        };
        let offer = self.read_only_offer(&state.dirty_paths, state.dirty_rows.len());
        if let Some(refused) = refused_choice(choice, offer.as_ref()) {
            crate::diagnostics::log_warning(format!(
                "Close Project '{}': {refused}",
                state.project_name
            ));
            return iced::Task::none();
        }
        self.handle_project_close_confirm(choice)
    }

    /// The Exit choice, refused the same way.
    pub(crate) fn handle_app_quit_choice(
        &mut self,
        choice: ProjectCloseChoice,
    ) -> iced::Task<Message> {
        let offer = self
            .ui_state
            .app_quit_confirm
            .as_ref()
            .and_then(|state| self.read_only_offer(&state.dirty_paths, state.dirty_rows.len()));
        if self.ui_state.app_quit_confirm.is_some()
            && let Some(refused) = refused_choice(choice, offer.as_ref())
        {
            crate::diagnostics::log_warning(format!("Exit: {refused}"));
            return iced::Task::none();
        }
        self.handle_app_quit_confirm(choice)
    }

    /// Close Project's discard of the read-only projects' settings: only
    /// their `.snxprj` drafts go. The project closes when nothing else of it
    /// is unsaved; otherwise the gate prompts again for the rest.
    pub(crate) fn discard_read_only_then_close(
        &mut self,
        state: ProjectCloseConfirmState,
    ) -> iced::Task<Message> {
        let dropped = self.read_only_dirty_paths(&state.dirty_paths);
        let names = quoted(&self.read_only_dirty_projects(&state.dirty_paths));
        let mut tasks: Vec<iced::Task<Message>> = dropped
            .iter()
            .map(|path| self.discard_document_draft(path))
            .collect();
        crate::diagnostics::log_info(format!(
            "Close Project '{}': the unsaved settings of {names} were discarded",
            state.project_name
        ));
        let nothing_else = state.dirty_rows.is_empty()
            && state.dirty_paths.iter().all(|path| dropped.contains(path));
        if nothing_else {
            tasks.push(self.autosave_sweep());
            tasks.push(self.execute_close_project(
                state.project_id,
                &state.sheet_refs,
                &state.kept_paths,
            ));
        } else {
            tasks.push(
                self.handle_close_refs_resolved(state.project_id, Ok(Box::new(state.sheet_refs))),
            );
        }
        iced::Task::batch(tasks)
    }

    /// Exit's discard of the read-only projects' settings: only their
    /// `.snxprj` drafts go, then the quit gate exits, or prompts again for
    /// what is still unsaved.
    pub(crate) fn discard_read_only_then_quit(
        &mut self,
        state: &AppQuitConfirmState,
    ) -> iced::Task<Message> {
        let dropped = self.read_only_dirty_paths(&state.dirty_paths);
        let names = quoted(&self.read_only_dirty_projects(&state.dirty_paths));
        let mut tasks: Vec<iced::Task<Message>> = dropped
            .iter()
            .map(|path| self.discard_document_draft(path))
            .collect();
        crate::diagnostics::log_info(format!(
            "Exit: the unsaved settings of {names} were discarded"
        ));
        tasks.push(self.handle_app_quit_requested());
        iced::Task::batch(tasks)
    }
}

/// Why `choice` is refused while `offer` stands; `None` when it goes ahead.
fn refused_choice(choice: ProjectCloseChoice, offer: Option<&ReadOnlyOffer>) -> Option<String> {
    let Some(offer) = offer else {
        // Nothing read-only to discard: the prompt stays as it is.
        return (choice == ProjectCloseChoice::DiscardReadOnly)
            .then(|| "nothing read-only is unsaved, so nothing was discarded".to_string());
    };
    let names = quoted(&offer.projects);
    match choice {
        ProjectCloseChoice::DiscardAll => Some(format!(
            "Discard All refused: {names} opens read-only; Discard {names} drops only its \
             settings. Nothing was discarded"
        )),
        ProjectCloseChoice::SaveAll if !offer.saveable => Some(format!(
            "Save All refused: {names} opens read-only, so its settings cannot be saved, and \
             nothing else is unsaved. Discard {names} or Cancel"
        )),
        _ => None,
    }
}

/// `names` quoted and joined, as the prompt shows them.
fn quoted(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("'{name}'")).collect();
    quoted.join(", ")
}

/// The file name of `path`, or the whole path when it has none.
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The close or quit prompt's note for `offer`: why the read-only projects'
/// settings cannot be saved, and what Save All and the discard do.
pub(crate) fn read_only_note(i18n: &I18n, offer: &ReadOnlyOffer) -> Localized {
    let id = if offer.saveable {
        ids::DIALOG_UNSAVED_READ_ONLY_PROJECTS
    } else {
        ids::DIALOG_UNSAVED_READ_ONLY_PROJECTS_ONLY
    };
    t!(
        i18n,
        id,
        "count" => offer.projects.len(),
        "projects" => quoted(&offer.projects)
    )
}

/// The discard button's label for the read-only `projects`.
pub(crate) fn discard_read_only_label(i18n: &I18n, projects: &[String]) -> Localized {
    t!(i18n, ids::DIALOG_UNSAVED_DISCARD_READ_ONLY, "projects" => quoted(projects))
}

/// The footer of the close or quit prompt. With `offer`, the discard names
/// the read-only projects and drops only their settings, and Save All is
/// offered while anything else saves; without, Discard All (when
/// `discard_all` allows it), Cancel and Save All.
pub(crate) fn unsaved_actions<M>(
    i18n: &I18n,
    offer: Option<&ReadOnlyOffer>,
    discard_all: bool,
    choose: impl Fn(ProjectCloseChoice) -> M,
) -> UnsavedActions<M> {
    match offer {
        Some(offer) => UnsavedActions {
            discard: Some(choose(ProjectCloseChoice::DiscardReadOnly)),
            discard_label: Some(discard_read_only_label(i18n, &offer.projects)),
            cancel: choose(ProjectCloseChoice::Cancel),
            save_all: offer.saveable.then(|| choose(ProjectCloseChoice::SaveAll)),
        },
        None => UnsavedActions {
            discard: discard_all.then(|| choose(ProjectCloseChoice::DiscardAll)),
            discard_label: None,
            cancel: choose(ProjectCloseChoice::Cancel),
            save_all: Some(choose(ProjectCloseChoice::SaveAll)),
        },
    }
}

/// The Project tab's banner line for a project read from a newer minor
/// written as `format`.
pub(crate) fn banner_line(i18n: &I18n, format: &str) -> Localized {
    t!(i18n, ids::PROJECT_READ_ONLY_BANNER, "format" => format.to_string())
}

#[cfg(test)]
#[path = "read_only_project_tests.rs"]
mod tests;
