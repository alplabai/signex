//! The ids of project files nobody has opened, read off the UI thread.
//!
//! A project lists its sheets and board before any of them is open. Their ids
//! sit at the head of each file, so a bounded read of the head is enough:
//! nothing is parsed in full and no tab or engine is made.

use std::io::Read;
use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::document_id::DocumentId;

use super::entries::referenced_files;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};

/// The most of a file's head that is read: the id is among the first lines.
const HEAD_BYTES: u64 = 64 * 1024;

/// One file's id as read from its head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderRead {
    pub path: PathBuf,
    pub id: Option<DocumentId>,
    /// The file's modification time when the head was read.
    pub modified: Option<std::time::SystemTime>,
}

/// The id at the head of the file at `path`; `None` for a file that is
/// missing, unreadable, of a kind without an id, or holds none.
pub(crate) fn read_header_id(path: &Path) -> Option<DocumentId> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(HEAD_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 == HEAD_BYTES {
        // Cut at a line end: the read may have stopped inside a line.
        let end = bytes.iter().rposition(|b| *b == b'\n').unwrap_or(0);
        bytes.truncate(end);
    }
    signex_types::format::document_id_in_header(&extension, &bytes)
}

/// Read the heads of `paths` on a blocking worker.
fn spawn_reads(project_id: ProjectId, paths: Vec<PathBuf>) -> Task<Message> {
    if paths.is_empty() {
        return Task::none();
    }
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                paths
                    .into_iter()
                    .map(|path| HeaderRead {
                        id: read_header_id(&path),
                        modified: std::fs::metadata(&path)
                            .and_then(|meta| meta.modified())
                            .ok(),
                        path,
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .unwrap_or_default()
        },
        move |reads| Message::Project(ProjectMsg::DocumentHeadersRead { project_id, reads }),
    )
}

impl Signex {
    /// Start reading the heads of the files project `id` references that are
    /// not loaded and whose head has not been read. Nothing when there is
    /// nothing to read.
    pub(crate) fn read_missing_document_headers(&self, id: ProjectId) -> Task<Message> {
        let docs = &self.document_state;
        let Some(project) = docs.project_by_id(id) else {
            return Task::none();
        };
        let paths = referenced_files(project)
            .into_iter()
            .map(|(path, _)| path)
            .filter(|path| !docs.engines.contains_key(path))
            .filter(|path| docs.document_ids.header_wanted(path))
            .collect();
        spawn_reads(id, paths)
    }

    /// The same for every open project.
    pub(crate) fn read_all_missing_document_headers(&self) -> Task<Message> {
        Task::batch(
            self.document_state
                .projects
                .iter()
                .map(|project| self.read_missing_document_headers(project.id)),
        )
    }

    /// Store the heads a worker read, unless the project closed meanwhile.
    pub(crate) fn handle_document_headers_read(
        &mut self,
        project_id: ProjectId,
        reads: Vec<HeaderRead>,
    ) -> Task<Message> {
        if self.document_state.project_by_id(project_id).is_none() {
            return Task::none();
        }
        for read in reads {
            self.document_state
                .document_ids
                .set_header(read.path, read.id, read.modified);
        }
        Task::none()
    }
}

impl Signex {
    /// Read, now, the heads that are wanted (never read, holding no id, or
    /// forgotten after a write or a close). For the headless entry, whose
    /// answer must name what the files hold at the time it is given; a GUI
    /// reads them on a worker ([`Self::read_missing_document_headers`]).
    pub(crate) fn read_wanted_document_headers_now(&mut self) {
        let docs = &self.document_state;
        let wanted: Vec<PathBuf> = docs
            .projects
            .iter()
            .flat_map(referenced_files)
            .map(|(path, _)| path)
            .filter(|path| !docs.engines.contains_key(path))
            .filter(|path| docs.document_ids.header_wanted(path))
            .collect();
        for path in wanted {
            let id = read_header_id(&path);
            let modified = std::fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok();
            self.document_state
                .document_ids
                .set_header(path, id, modified);
        }
    }
}
