//! Save writes the notebook file atomically (temp file, sync, rename), off
//! the UI thread from the tab and Ctrl+S, on the calling thread from Save
//! All and the close and quit prompts, which already wait on their writes.
//! An unchanged header is written back byte for byte; a file a newer Signex
//! wrote is refused, so nothing it holds is dropped.

use std::path::Path;

use iced::Task;
use signex_notebook::NotebookDocument;

use super::NotebookMsg;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// The file text Save writes for `doc`: its file (or a new one) with the
/// body as typed. `None` when there is nothing to write.
pub(super) fn written_text(doc: &NotebookDocument) -> Option<Result<String, String>> {
    doc.written_text()
        .map(|written| written.map_err(|error| error.to_string()))
}

/// Blocking: write `text` to `path`, making the notebook folder first.
fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?;
    }
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|error| format!("{}: {error}", path.display()))
}

impl Signex {
    /// Save `id`'s notebook from the tab: the write runs in a task.
    pub(super) fn save_notebook(&mut self, id: ProjectId) -> Task<Message> {
        let Some(path) = self.sync_notebook_path(id) else {
            return Task::none();
        };
        let Some(doc) = self.document_state.notebooks.get_mut(&id) else {
            return Task::none();
        };
        if doc.saving {
            return Task::none();
        }
        let text = match written_text(doc) {
            Some(Ok(text)) => text,
            Some(Err(why)) => {
                doc.save_error = Some(why.clone());
                crate::diagnostics::log_warning(format!("[notebook] Save refused: {why}"));
                return Task::none();
            }
            None => return Task::none(),
        };
        doc.saving = true;
        doc.save_error = None;
        Task::perform(
            async move {
                let written = text.clone();
                let result = tokio::task::spawn_blocking(move || write_file(&path, &text))
                    .await
                    .map_err(|error| format!("the save did not finish: {error}"))
                    .and_then(|done| done);
                (written, result)
            },
            move |(written, result)| {
                Message::from(NotebookMsg::Saved {
                    project_id: id,
                    written,
                    result,
                })
            },
        )
    }

    /// A save landed. The text is clean when it is still what was written;
    /// an edit made while the write ran stays unsaved.
    pub(super) fn notebook_saved(
        &mut self,
        id: ProjectId,
        written: &str,
        result: Result<(), String>,
    ) {
        let Some(doc) = self.document_state.notebooks.get_mut(&id) else {
            return;
        };
        doc.saving = false;
        if let Err(why) = result {
            crate::diagnostics::log_warning(format!("[notebook] Save failed: {why}"));
            doc.save_error = Some(why);
            return;
        }
        let path = doc.path.clone();
        mark_written(doc, written);
        if !doc.is_unsaved() {
            self.document_state.clear_document_dirty(&path);
        }
    }

    /// Save All and the close and quit prompts: write `path`'s notebook now.
    pub(crate) fn save_notebook_blocking(&mut self, path: &Path) -> anyhow::Result<()> {
        let id = self
            .notebook_at_path(path)
            .ok_or_else(|| anyhow::anyhow!("no notebook is open at {}", path.display()))?;
        let target = self
            .sync_notebook_path(id)
            .unwrap_or_else(|| path.to_path_buf());
        let path = target.as_path();
        let Some(doc) = self.document_state.notebooks.get_mut(&id) else {
            anyhow::bail!("no notebook is open at {}", path.display());
        };
        let text = match written_text(doc) {
            Some(Ok(text)) => text,
            Some(Err(why)) => anyhow::bail!("{}: {why}", path.display()),
            None => anyhow::bail!("{} has no text to save", path.display()),
        };
        write_file(path, &text).map_err(anyhow::Error::msg)?;
        mark_written(doc, &text);
        self.document_state.clear_document_dirty(path);
        Ok(())
    }

    /// Discard All: the notebook's unsaved text goes, and its tab with it.
    pub(crate) fn discard_notebook_at(&mut self, path: &Path) -> Task<Message> {
        let Some(id) = self.notebook_at_path(path) else {
            return Task::none();
        };
        self.document_state.notebooks.remove(&id);
        self.ui_state.notebook_tabs.remove(&id);
        self.document_state.clear_document_dirty(path);
        match self.notebook_tab_index(id) {
            Some(idx) => self.close_notebook_tab(idx, false),
            None => Task::none(),
        }
    }
}

/// The file now holds `written`: keep it as the file read, so the next
/// save of an unchanged header writes it back byte for byte.
fn mark_written(doc: &mut NotebookDocument, written: &str) {
    let at = chrono::Local::now().format("%H:%M").to_string();
    if let Err(error) = doc.mark_written(written, at) {
        crate::diagnostics::log_warning(format!(
            "[notebook] the saved file reads back with an error: {error}"
        ));
    }
}
