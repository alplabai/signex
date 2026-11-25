//! A project read from a newer `.snxprj` minor opens read-only (#2904,
//! DECISIONS 2026-11-25): the refusals that stop its writes before anything
//! on disk moves, and what the close and quit prompts withhold for it.

use std::path::{Path, PathBuf};

use signex_i18n::{I18n, Localized, Translatable, ids, t};
use signex_types::format_version::FormatVersionError;

use super::state::ProjectId;
use super::{Message, ProjectCloseChoice, Signex};

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
        self.document_state
            .projects
            .iter()
            .filter(|p| p.data.is_from_newer_minor() && dirty.contains(&p.path))
            .map(|p| p.data.name.clone())
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
        let Some(refusal) = self.read_only_project_save(project_id) else {
            return false;
        };
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        // The Messages ring clips a line: the file name goes first.
        crate::diagnostics::log_warning(format!(
            "Add New Schematic refused: {file} was not created; project '{}' is read-only ({})",
            refusal.project, refusal.source
        ));
        let detail = Translatable::new(ids::NOTICE_PROJECT_READ_ONLY_ADD)
            .arg("project", refusal.project.clone())
            .arg("file", file);
        self.document_state.error_notice = Some(
            crate::app::state::ErrorNotice::create(detail).with_system(refusal.source.to_string()),
        );
        true
    }

    /// The Close Project choice, except that Save All is refused while the
    /// project's own unsaved settings are read-only: the prompt does not offer
    /// it, and a forced one leaves the prompt up.
    pub(crate) fn handle_project_close_choice(
        &mut self,
        choice: ProjectCloseChoice,
    ) -> iced::Task<Message> {
        if choice == ProjectCloseChoice::SaveAll
            && let Some(state) = &self.ui_state.project_close_confirm
            && let Some(names) = self.save_all_withheld_for(&state.dirty_paths)
        {
            crate::diagnostics::log_warning(format!(
                "Close Project '{}': Save All refused: {names} opens read-only, so its settings \
                 cannot be saved. Discard All or Cancel",
                state.project_name
            ));
            return iced::Task::none();
        }
        self.handle_project_close_confirm(choice)
    }

    /// The Exit choice, with Save All refused the same way while any
    /// read-only project has unsaved settings.
    pub(crate) fn handle_app_quit_choice(
        &mut self,
        choice: ProjectCloseChoice,
    ) -> iced::Task<Message> {
        if choice == ProjectCloseChoice::SaveAll
            && let Some(state) = &self.ui_state.app_quit_confirm
            && let Some(names) = self.save_all_withheld_for(&state.dirty_paths)
        {
            crate::diagnostics::log_warning(format!(
                "Exit: Save All refused: {names} opens read-only, so its settings cannot be \
                 saved. Discard All or Cancel"
            ));
            return iced::Task::none();
        }
        self.handle_app_quit_confirm(choice)
    }

    /// The read-only projects in `dirty`, joined for a line; `None` when a
    /// Save All of `dirty` can write everything.
    fn save_all_withheld_for(&self, dirty: &[PathBuf]) -> Option<String> {
        let names = self.read_only_dirty_projects(dirty);
        (!names.is_empty()).then(|| names.join(", "))
    }
}

/// The close or quit prompt's note when Save All is withheld for the
/// read-only `projects`; `None` when there are none.
pub(crate) fn save_all_withheld_line(i18n: &I18n, projects: &[String]) -> Option<Localized> {
    if projects.is_empty() {
        return None;
    }
    let names: Vec<String> = projects.iter().map(|name| format!("'{name}'")).collect();
    Some(t!(
        i18n,
        ids::DIALOG_UNSAVED_READ_ONLY_PROJECTS,
        "count" => projects.len(),
        "projects" => names.join(", ")
    ))
}

/// The Project tab's banner line for a project read from a newer minor
/// written as `format`.
pub(crate) fn banner_line(i18n: &I18n, format: &str) -> Localized {
    t!(i18n, ids::PROJECT_READ_ONLY_BANNER, "format" => format.to_string())
}

#[cfg(test)]
#[path = "read_only_project_tests.rs"]
mod tests;
