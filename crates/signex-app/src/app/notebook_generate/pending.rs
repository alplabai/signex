//! The report Generate made in the app (mockup row 201): held unsaved in
//! `DocumentState::generated_reports`, its path in the dirty set so Save
//! All and the close and quit prompts see it, and written by Save (Save
//! All, or Save in those prompts) atomically to
//! `notebook/generated/design-report.snxnbk`, replacing the report made
//! before (Regenerate). Discard drops it.

use std::path::{Path, PathBuf};

use iced::Task;

use super::NotebookGenerateMsg;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// Blocking: write `text` to `path`, making its folder first.
fn write_report(path: &Path, text: &str) -> Result<(), String> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?;
    }
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|error| format!("{}: {error}", path.display()))
}

impl Signex {
    /// Where `id`'s held report is written.
    fn generated_report_path(&self, id: ProjectId) -> Option<PathBuf> {
        let report = self.document_state.generated_reports.get(&id)?;
        let project = self.document_state.project_by_id(id)?;
        Some(project.dir().join(&report.path))
    }

    /// The project whose held report is written at `path`.
    pub(crate) fn generated_report_at(&self, path: &Path) -> Option<ProjectId> {
        self.document_state
            .generated_reports
            .keys()
            .copied()
            .find(|id| self.generated_report_path(*id).is_some_and(|at| at == path))
    }

    /// Hold `report` for the project at `project`, unsaved: its path joins
    /// the dirty set.
    pub(super) fn hold_generated_report(
        &mut self,
        project: &Path,
        report: signex_notebook::generate::GeneratedNotebook,
    ) {
        let Some(id) = self
            .document_state
            .projects
            .iter()
            .find(|loaded| loaded.path == project)
            .map(|loaded| loaded.id)
        else {
            return;
        };
        self.document_state.generated_reports.insert(id, report);
        if let Some(path) = self.generated_report_path(id) {
            self.document_state.mark_document_dirty(&path);
        }
    }

    /// Save All and the close and quit prompts: write the report held at
    /// `path` in a task that lands as [`NotebookGenerateMsg::ReportSaved`].
    pub(crate) fn begin_generated_report_save_at(
        &mut self,
        path: &Path,
    ) -> anyhow::Result<Task<Message>> {
        let id = self
            .generated_report_at(path)
            .ok_or_else(|| anyhow::anyhow!("no design report is held at {}", path.display()))?;
        let text = self.document_state.generated_reports[&id].text.clone();
        let target = path.to_path_buf();
        Ok(Task::perform(
            async move {
                let written = text.clone();
                let result = tokio::task::spawn_blocking(move || write_report(&target, &text))
                    .await
                    .map_err(|error| format!("the save did not finish: {error}"))
                    .and_then(|done| done);
                (written, result)
            },
            move |(text, result)| {
                Message::from(NotebookGenerateMsg::ReportSaved {
                    project_id: id,
                    text,
                    result,
                })
            },
        ))
    }

    /// A report write landed: written, it is no longer held, unless a newer
    /// run replaced it meanwhile.
    pub(super) fn generated_report_saved(
        &mut self,
        id: ProjectId,
        text: &str,
        result: Result<(), String>,
    ) {
        if let Err(why) = result {
            crate::diagnostics::log_warning(format!("[notebook] the design report: {why}"));
            return;
        }
        let Some(path) = self.generated_report_path(id) else {
            return;
        };
        let same = self
            .document_state
            .generated_reports
            .get(&id)
            .is_some_and(|held| held.text == text);
        if same {
            self.document_state.generated_reports.remove(&id);
            self.document_state.clear_document_dirty(&path);
        }
    }

    /// Write the report held at `path` now, on this thread.
    pub(crate) fn save_generated_report_blocking(&mut self, path: &Path) -> anyhow::Result<()> {
        let id = self
            .generated_report_at(path)
            .ok_or_else(|| anyhow::anyhow!("no design report is held at {}", path.display()))?;
        let text = self.document_state.generated_reports[&id].text.clone();
        let result = write_report(path, &text);
        self.generated_report_saved(id, &text, result.clone());
        result.map_err(anyhow::Error::msg)
    }

    /// Discard All: the held report goes, and its dirty mark.
    pub(crate) fn discard_generated_report_at(&mut self, path: &Path) {
        if let Some(id) = self.generated_report_at(path) {
            self.document_state.generated_reports.remove(&id);
            self.document_state.clear_document_dirty(path);
        }
    }
}
