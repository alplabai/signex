//! Save writes the notebook file atomically (temp file, sync, rename), off
//! the UI thread: from the tab and Ctrl+S, and from Save All and the close
//! and quit prompts, which run it inside their settle like every other
//! document's write. One save runs at a time; a blocking save (the tests'
//! and the prompts' fallback) is refused while one runs. An unchanged
//! header is written back byte for byte; a file a newer Signex wrote is
//! refused, so nothing it holds is dropped; the first Save of a new
//! notebook never replaces a file written at its path meanwhile.
//!
//! Before the text is written the header stores the id of every reference
//! the project's last notebook read found, so a part renamed later, outside
//! Annotate, is still found under its new name (mockup row 196).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use iced::Task;
use signex_i18n::{RenderTranslatable as _, Translatable, ids};
use signex_notebook::{NotebookDocument, references, resolve_references};

use super::NotebookMsg;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// The file text Save writes for `doc`: its file (or a new one) with the
/// body as typed. `None` when there is nothing to write.
pub(super) fn written_text(doc: &NotebookDocument) -> Option<Result<String, String>> {
    doc.written_text()
        .map(|written| written.map_err(|error| error.to_string()))
}

/// Blocking: write `text` to `path`, making the notebook folder first. A
/// new notebook's write refuses a file at `path` (`appeared`, the reason).
fn write_file(
    path: &Path,
    text: &str,
    appeared: Option<&str>,
    changed: Option<&(String, String)>,
) -> Result<(), String> {
    if let Some(why) = appeared
        && path.exists()
    {
        return Err(why.to_string());
    }
    // Something else wrote the file since it was read or saved (#3019).
    if let Some((was, why)) = changed
        && let Ok(now) = std::fs::read_to_string(path)
        && now != *was
    {
        return Err(why.clone());
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?;
    }
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// A write to start: the text, where it goes, and the reason a new
/// notebook's write gives when a file appeared at its path.
struct Write {
    path: PathBuf,
    text: String,
    appeared: Option<String>,
    /// The file's text as last read or saved, and the reason a write gives
    /// when the file is no longer so.
    changed: Option<(String, String)>,
}

impl Signex {
    /// The write of `id`'s notebook as it is now, marked as running; why
    /// there is none.
    fn notebook_write(&mut self, id: ProjectId) -> Result<Write, String> {
        let path = self
            .sync_notebook_path(id)
            .ok_or_else(|| "the project is no longer open".to_string())?;
        self.store_reference_ids(id);
        let appeared = self.notebook_appeared_reason(id, &path);
        let changed_why = Translatable::new(ids::NOTEBOOK_TAB_CMD_CHANGED_ON_DISK)
            .arg("path", path.display().to_string())
            .localized(&self.ui_state.i18n)
            .into_string();
        let doc = self
            .document_state
            .notebooks
            .get_mut(&id)
            .ok_or_else(|| "the notebook is no longer open".to_string())?;
        if doc.saving {
            return Err("a save of the notebook is already running".to_string());
        }
        let text = match written_text(doc) {
            Some(Ok(text)) => text,
            Some(Err(why)) => {
                doc.save_error = Some(why.clone());
                return Err(why);
            }
            None => return Err(format!("{} has no text to save", path.display())),
        };
        if let Some(why) = appeared.as_ref().filter(|_| path.exists()) {
            doc.save_error = Some(why.clone());
            return Err(why.clone());
        }
        // Checked on the write's worker, where the file is read.
        let changed = doc.disk_text.clone().map(|was| (was, changed_why));
        doc.saving = true;
        doc.save_error = None;
        Ok(Write {
            path,
            text,
            appeared,
            changed,
        })
    }

    /// The reason the first Save of `id`'s new notebook refuses a file at
    /// `path`; `None` when the notebook was read from or saved to it.
    fn notebook_appeared_reason(&self, id: ProjectId, path: &Path) -> Option<String> {
        let doc = self.document_state.notebooks.get(&id)?;
        doc.is_new().then(|| {
            Translatable::new(ids::NOTEBOOK_TAB_CMD_FILE_APPEARED)
                .arg("path", path.display().to_string())
                .localized(&self.ui_state.i18n)
                .into_string()
        })
    }

    /// The header keeps the id of every reference the project's last
    /// notebook read resolved (mockup row 196): with no design read,
    /// nothing changes.
    fn store_reference_ids(&mut self, id: ProjectId) {
        let design = self.notebook_design(id);
        if design.parts().is_empty() && design.nets().is_empty() {
            return;
        }
        let Some(doc) = self.document_state.notebooks.get_mut(&id) else {
            return;
        };
        let Some(body) = doc.body.clone() else {
            return;
        };
        let Some(file) = doc.file.as_mut() else {
            return;
        };
        let found = references(&body);
        let report = resolve_references(&found, &file.header, &design);
        let _changed = report.store_ids(&mut file.header);
    }

    /// The task of `write`, landing as [`NotebookMsg::Saved`].
    fn notebook_write_task(id: ProjectId, write: Write) -> Task<Message> {
        let Write {
            path,
            text,
            appeared,
            changed,
        } = write;
        Task::perform(
            async move {
                let written = text.clone();
                let result = tokio::task::spawn_blocking(move || {
                    write_file(&path, &text, appeared.as_deref(), changed.as_ref())
                })
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

    /// Save `id`'s notebook from the tab: the write runs in a task.
    pub(super) fn save_notebook(&mut self, id: ProjectId) -> Task<Message> {
        match self.notebook_write(id) {
            Ok(write) => Self::notebook_write_task(id, write),
            Err(why) => {
                crate::diagnostics::log_warning(format!("[notebook] Save refused: {why}"));
                Task::none()
            }
        }
    }

    /// Save All and the close and quit prompts: start the write of the
    /// notebook at `path` in a task that lands as [`NotebookMsg::Saved`]; the
    /// Save All settle reads the dirty mark it leaves.
    pub(crate) fn begin_notebook_save_at(&mut self, path: &Path) -> anyhow::Result<Task<Message>> {
        let id = self
            .notebook_at_path(path)
            .ok_or_else(|| anyhow::anyhow!("no notebook is open at {}", path.display()))?;
        let write = self
            .notebook_write(id)
            .map_err(|why| anyhow::anyhow!("{}: {why}", path.display()))?;
        Ok(Self::notebook_write_task(id, write))
    }

    /// A save landed. The text is clean when it is still what was written;
    /// an edit made while the write ran stays unsaved, and its path dirty.
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
        if doc.is_unsaved() {
            self.document_state.mark_document_dirty(&path);
        } else {
            self.document_state.clear_document_dirty(&path);
        }
    }

    /// Write `path`'s notebook now, on this thread. Refused while a save of
    /// it runs, so two writes never race.
    pub(crate) fn save_notebook_blocking(&mut self, path: &Path) -> anyhow::Result<()> {
        let id = self
            .notebook_at_path(path)
            .ok_or_else(|| anyhow::anyhow!("no notebook is open at {}", path.display()))?;
        let write = self
            .notebook_write(id)
            .map_err(|why| anyhow::anyhow!("{}: {why}", path.display()))?;
        let result = write_file(
            &write.path,
            &write.text,
            write.appeared.as_deref(),
            write.changed.as_ref(),
        );
        self.notebook_saved(id, &write.text, result.clone());
        result.map_err(anyhow::Error::msg)
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
    if let Err(error) = doc.mark_written(written, SystemTime::now()) {
        crate::diagnostics::log_warning(format!(
            "[notebook] the saved file reads back with an error: {error}"
        ));
    }
}
