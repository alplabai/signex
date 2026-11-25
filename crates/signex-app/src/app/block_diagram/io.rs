//! Reading and writing `.snxblk` files, always off the update thread: an
//! open reads in a task and the tab opens when the read lands; a save
//! writes a copy of the diagram in a task and marks it clean only with the
//! diagram it wrote. A diagram never saved refuses to replace a file that
//! is already at its path.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_types::block_diagram::{
    BlockDiagram, ReadBlockDiagram, read_block_diagram, write_block_diagram,
};

use super::refusal::Refusal;
use super::state::BlockDiagramEditor;
use super::{BlockDiagramMsg, file_name};
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::state::ErrorNotice;
use crate::app::{Message, Signex};

/// What a save writes: the diagram as it was when the save started.
#[derive(Debug, Clone, PartialEq)]
struct SaveJob {
    diagram: BlockDiagram,
    token: String,
    /// The diagram was never saved: refuse a file already at the path.
    is_new: bool,
}

/// Why a read or a write failed.
#[derive(Debug, Clone, PartialEq)]
pub enum FileFailure {
    /// A new diagram's path already holds a file.
    Exists,
    /// The file system or the format said no, in its own words.
    Failed(String),
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
            on_blocking_pool(move || read_block_diagram(&read_path).map_err(|e| e.to_string())),
            move |result| {
                Message::BlockDiagram(BlockDiagramMsg::Read {
                    path: path.clone(),
                    result: result.map(Box::new).map_err(FileFailure::Failed),
                })
            },
        )
    }

    /// A read landed: open the tab, or say why the file did not open.
    pub(super) fn block_diagram_read(
        &mut self,
        path: PathBuf,
        result: Result<Box<ReadBlockDiagram>, FileFailure>,
    ) -> Task<Message> {
        self.document_state.block_diagrams.opening.remove(&path);
        match result {
            Ok(read) => {
                // A diagram made for this path meanwhile (a New of the same
                // name) is not replaced by the file.
                self.document_state
                    .block_diagrams
                    .editors
                    .entry(path.clone())
                    .or_insert_with(|| BlockDiagramEditor::opened(*read));
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
                self.document_state.error_notice =
                    Some(ErrorNotice::open(detail).with_system(reason));
            }
        }
        Task::none()
    }

    /// Start writing the diagram at `path`. Refused while it is read-only
    /// or already being written.
    pub(crate) fn begin_block_diagram_save(
        &mut self,
        path: &Path,
    ) -> Result<Task<Message>, Refusal> {
        let diagrams = &mut self.document_state.block_diagrams;
        let editor = diagrams.editors.get(path).ok_or_else(|| Refusal::NotOpen {
            file: file_name(path),
        })?;
        if editor.is_read_only() {
            return Err(Refusal::ReadOnly {
                file: file_name(path),
                token: editor.token.clone(),
            });
        }
        if diagrams.saving.contains(path) {
            return Err(Refusal::Saving {
                file: file_name(path),
            });
        }
        let job = SaveJob {
            diagram: editor.diagram.clone(),
            token: editor.token.clone(),
            is_new: editor.is_new(),
        };
        diagrams.saving.insert(path.to_path_buf());
        let target = path.to_path_buf();
        let written = job.diagram.clone();
        let path = path.to_path_buf();
        Ok(Task::perform(
            on_blocking_pool(move || Ok(write_job(&target, &job))),
            move |result| {
                Message::BlockDiagram(BlockDiagramMsg::Saved {
                    path: path.clone(),
                    written: Box::new(written.clone()),
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
        result: Result<(), FileFailure>,
    ) -> Task<Message> {
        self.document_state.block_diagrams.saving.remove(&path);
        match result {
            Ok(()) => {
                if let Some(editor) = self.document_state.block_diagrams.editors.get_mut(&path) {
                    editor.saved = Some(written);
                }
                self.document_state.sync_block_diagram_dirty(&path);
                crate::diagnostics::log_info(format!("Saved {}", path.display()));
            }
            Err(failure) => {
                let reason = failure_text(&failure);
                crate::diagnostics::log_warning(format!(
                    "Save of {} failed: {reason}",
                    path.display()
                ));
                let id = match failure {
                    FileFailure::Exists => ids::BLOCK_DIAGRAM_SAVE_EXISTS,
                    FileFailure::Failed(_) => ids::BLOCK_DIAGRAM_SAVE_FAILED,
                };
                let detail = Translatable::new(id).arg("file", file_name(&path));
                let mut notice = ErrorNotice::save(detail);
                if let FileFailure::Failed(text) = &failure {
                    notice = notice.with_system(text.clone());
                }
                self.document_state.error_notice = Some(notice);
            }
        }
        self.refresh_panel_ctx();
        Task::none()
    }
}

fn failure_text(failure: &FileFailure) -> String {
    match failure {
        FileFailure::Exists => "a file is already at the path".to_string(),
        FileFailure::Failed(text) => text.clone(),
    }
}

/// The write itself, on the blocking pool. A new diagram never replaces a
/// file.
fn write_job(path: &Path, job: &SaveJob) -> Result<(), FileFailure> {
    if job.is_new && path.exists() {
        return Err(FileFailure::Exists);
    }
    write_block_diagram(path, &job.diagram, &job.token)
        .map_err(|e| FileFailure::Failed(e.to_string()))
}
