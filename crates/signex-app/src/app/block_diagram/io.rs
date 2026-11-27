//! Reading and writing `.snxblk` files, always off the update thread: an
//! open reads in a task and the tab opens when the read lands; a save
//! writes a copy of the diagram in a task and marks it clean only with the
//! diagram it wrote. A diagram never saved refuses to replace a file that
//! is already at its path.
//!
//! A save goes through the document save tracker (`DocumentSaves`), as a
//! sheet's does: writes to one file are ordered, quit waits for a running
//! one, and a file changed on disk since it was read or saved is not
//! overwritten (the disk baseline, `docs/architecture/external-change.md`).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_types::block_diagram::{
    BlockDiagram, ReadBlockDiagram, encode_block_diagram, parse_block_diagram,
};

use super::refusal::Refusal;
use super::state::BlockDiagramEditor;
use super::{BlockDiagramMsg, file_name};
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::state::ErrorNotice;
use crate::app::{Message, Signex};
use crate::persist::{ContentHash, SaveOutcome};

/// Why a read or a write failed.
#[derive(Debug, Clone, PartialEq)]
pub enum FileFailure {
    /// A new diagram's path already holds a file.
    Exists,
    /// The file changed on disk since it was read or last saved; nothing
    /// was written.
    Changed,
    /// A newer save of the same file landed first; this one was skipped.
    Superseded,
    /// The file system or the format said no, in its own words.
    Failed(String),
}

/// A diagram file as read: the diagram, and the fingerprint of its bytes
/// for the save tracker's disk baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramFile {
    pub(crate) read: ReadBlockDiagram,
    pub(crate) hash: ContentHash,
}

/// Read the diagram at `path`. Blocking: run it on the blocking pool.
pub(super) fn read_diagram_file(path: &Path) -> Result<Box<DiagramFile>, FileFailure> {
    let text = std::fs::read_to_string(path).map_err(|e| FileFailure::Failed(e.to_string()))?;
    let read = parse_block_diagram(&text).map_err(|e| FileFailure::Failed(e.to_string()))?;
    Ok(Box::new(DiagramFile {
        read,
        hash: ContentHash::of(text.as_bytes()),
    }))
}

impl Signex {
    /// Open the diagram at `path`: focus its tab when it is open, else read
    /// it in a task.
    pub(crate) fn open_block_diagram_file(&mut self, path: PathBuf) -> Task<Message> {
        if self
            .document_state
            .block_diagrams
            .editors
            .contains_key(&path)
        {
            self.show_block_diagram_tab(&path);
            return Task::none();
        }
        if !self
            .document_state
            .block_diagrams
            .opening
            .insert(path.clone())
        {
            return Task::none();
        }
        let read_path = path.clone();
        Task::perform(
            on_blocking_pool(move || Ok(read_diagram_file(&read_path))),
            move |result| {
                Message::BlockDiagram(BlockDiagramMsg::Read {
                    path: path.clone(),
                    result: result.unwrap_or_else(|died| Err(FileFailure::Failed(died))),
                })
            },
        )
    }

    /// Hold the diagram read from `path` unless one is held there already
    /// (a New of the same name, an earlier read), whose edits are kept. A
    /// diagram taken from the file starts the file's disk baseline.
    pub(super) fn hold_read_diagram(&mut self, path: &Path, file: DiagramFile) {
        let editors = &mut self.document_state.block_diagrams.editors;
        if editors.contains_key(path) {
            return;
        }
        editors.insert(path.to_path_buf(), BlockDiagramEditor::opened(file.read));
        self.document_state
            .document_saves
            .set_baseline_from_read(path.to_path_buf(), file.hash);
    }

    /// A read landed: open the tab, or say why the file did not open.
    pub(super) fn block_diagram_read(
        &mut self,
        path: PathBuf,
        result: Result<Box<DiagramFile>, FileFailure>,
    ) -> Task<Message> {
        self.document_state.block_diagrams.opening.remove(&path);
        match result {
            Ok(file) => {
                self.hold_read_diagram(&path, *file);
                self.show_block_diagram_tab(&path);
            }
            Err(failure) => {
                let reason = failure_text(&failure);
                crate::diagnostics::log_warning(format!(
                    "Open of {} failed: {reason}",
                    path.display()
                ));
                let detail =
                    Translatable::new(ids::BLOCK_DIAGRAM_OPEN_FAILED).arg("file", file_name(&path));
                self.document_view.error_notice =
                    Some(ErrorNotice::open(detail).with_system(reason));
            }
        }
        Task::none()
    }

    /// Start writing the diagram at `path` through the save tracker.
    /// Refused while it is read-only or already being written.
    pub(crate) fn begin_block_diagram_save(
        &mut self,
        path: &Path,
    ) -> Result<Task<Message>, Refusal> {
        let docs = &mut self.document_state;
        let editor = docs
            .block_diagrams
            .editors
            .get(path)
            .ok_or_else(|| Refusal::NotOpen {
                file: file_name(path),
            })?;
        if editor.is_read_only() {
            return Err(Refusal::ReadOnly {
                file: file_name(path),
                token: editor.token.clone(),
            });
        }
        if docs.block_diagrams.saving.contains(path) {
            return Err(Refusal::Saving {
                file: file_name(path),
            });
        }
        let written = editor.diagram.clone();
        let is_new = editor.is_new();
        let bytes = encode_block_diagram(&written)
            .map_err(|e| Refusal::Unwritable {
                file: file_name(path),
                reason: e.to_string(),
            })?
            .into_bytes();
        let hash = ContentHash::of(&bytes);
        // A diagram read from its file is compared with what the file held
        // then; a new one has no baseline and refuses any file there.
        let expected = (!is_new)
            .then(|| docs.document_saves.baseline(path))
            .flatten();
        let job = docs
            .document_saves
            .begin(path.to_path_buf(), bytes)
            .expecting(expected);
        docs.block_diagrams.saving.insert(path.to_path_buf());
        let target = path.to_path_buf();
        let path = path.to_path_buf();
        Ok(Task::perform(
            on_blocking_pool(move || {
                if is_new && target.exists() {
                    return Ok(Err(FileFailure::Exists));
                }
                Ok(job.run().map_err(FileFailure::Failed).and_then(outcome))
            }),
            move |result| {
                Message::BlockDiagram(BlockDiagramMsg::Saved {
                    path: path.clone(),
                    written: Box::new(written.clone()),
                    hash,
                    result: result.unwrap_or_else(|died| Err(FileFailure::Failed(died))),
                })
            },
        ))
    }

    /// A save landed: the diagram it wrote is the file's now; a failure
    /// keeps the edits and says why.
    pub(super) fn block_diagram_saved(
        &mut self,
        path: PathBuf,
        written: BlockDiagram,
        hash: ContentHash,
        result: Result<(), FileFailure>,
    ) -> Task<Message> {
        let docs = &mut self.document_state;
        docs.block_diagrams.saving.remove(&path);
        docs.document_saves.finish(&path);
        match result {
            Ok(()) => {
                if let Some(editor) = docs.block_diagrams.editors.get_mut(&path) {
                    editor.saved = Some(written);
                }
                docs.document_saves.set_baseline(path.clone(), hash);
                docs.document_saves.clear_failure(&path);
                docs.sync_block_diagram_dirty(&path);
                crate::diagnostics::log_info(format!("Saved {}", path.display()));
            }
            Err(FileFailure::Superseded) => {
                crate::diagnostics::log_info(format!(
                    "Save of {} skipped: a newer save of the file was written",
                    path.display()
                ));
            }
            Err(failure) => self.say_save_failed(&path, failure),
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    fn say_save_failed(&mut self, path: &Path, failure: FileFailure) {
        let reason = failure_text(&failure);
        crate::diagnostics::log_warning(format!("Save of {} failed: {reason}", path.display()));
        self.document_state
            .document_saves
            .record_failure(path.to_path_buf(), reason);
        let id = match failure {
            FileFailure::Exists => ids::BLOCK_DIAGRAM_SAVE_EXISTS,
            FileFailure::Changed => ids::BLOCK_DIAGRAM_SAVE_CHANGED,
            FileFailure::Superseded | FileFailure::Failed(_) => ids::BLOCK_DIAGRAM_SAVE_FAILED,
        };
        let detail = Translatable::new(id).arg("file", file_name(path));
        let mut notice = ErrorNotice::save(detail);
        if let FileFailure::Failed(text) = &failure {
            notice = notice.with_system(text.clone());
        }
        self.document_view.error_notice = Some(notice);
    }
}

/// What the tracker did with the write, as a save result.
fn outcome(done: SaveOutcome) -> Result<(), FileFailure> {
    match done {
        SaveOutcome::Written => Ok(()),
        SaveOutcome::Superseded => Err(FileFailure::Superseded),
        SaveOutcome::DiskChanged(_) => Err(FileFailure::Changed),
    }
}

pub(super) fn failure_text(failure: &FileFailure) -> String {
    match failure {
        FileFailure::Exists => "a file is already at the path".to_string(),
        FileFailure::Changed => "the file changed on disk since it was opened or saved".to_string(),
        FileFailure::Superseded => "a newer save of the file was written".to_string(),
        FileFailure::Failed(text) => text.clone(),
    }
}
