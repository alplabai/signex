//! Filesystem moves and deletes that run off the UI thread (#859): the
//! pending-operation registry that refuses writes meanwhile. See
//! `docs/architecture/path-ops.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::project::LibraryRetarget;
use signex_types::schematic::SchematicSheet;

use crate::app::state::ProjectId;
use crate::app::state::scope::{CaseFold, path_is_under};

/// Identity of one in-flight move or delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathOpId(u64);

/// What the pending operation does, for the refusal text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOpKind {
    Rename,
    RenameLibrary,
    Delete,
}

impl PathOpKind {
    fn noun(self) -> &'static str {
        match self {
            Self::Rename | Self::RenameLibrary => "rename",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone)]
struct PendingPathOp {
    id: PathOpId,
    kind: PathOpKind,
    /// The path moved or deleted, and for a rename also its destination.
    paths: Vec<PathBuf>,
    /// Files the operation reads or rewrites without moving them (the
    /// project file of a library rename, the parent sheets of a sheet
    /// rename). Written or opened meanwhile, they would race the worker.
    dependents: Vec<PathBuf>,
    fold: CaseFold,
}

/// The pending operation a path collides with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathOpConflict {
    pub kind: PathOpKind,
    /// The path the operation moves or deletes.
    pub path: PathBuf,
    /// The collision was with a file the operation only rewrites.
    pub dependent: bool,
}

/// Every rename and delete whose disk work has not reported back yet.
#[derive(Debug, Default)]
pub struct PendingPathOps {
    next_id: u64,
    ops: Vec<PendingPathOp>,
}

impl PendingPathOps {
    /// Register an operation before its worker starts.
    pub fn begin(&mut self, kind: PathOpKind, paths: Vec<PathBuf>, fold: CaseFold) -> PathOpId {
        self.begin_with_dependents(kind, paths, Vec::new(), fold)
    }

    /// As [`Self::begin`], also guarding `dependents`: files the worker
    /// rewrites in place.
    pub fn begin_with_dependents(
        &mut self,
        kind: PathOpKind,
        paths: Vec<PathBuf>,
        dependents: Vec<PathBuf>,
        fold: CaseFold,
    ) -> PathOpId {
        let id = PathOpId(self.next_id);
        self.next_id += 1;
        self.ops.push(PendingPathOp {
            id,
            kind,
            paths,
            dependents,
            fold,
        });
        id
    }

    /// Whether the operation `id` is still registered.
    pub fn is_registered(&self, id: PathOpId) -> bool {
        self.ops.iter().any(|op| op.id == id)
    }

    /// Drop a registration when its operation is completely finished.
    /// Returns whether it was registered.
    pub fn finish(&mut self, id: PathOpId) -> bool {
        let before = self.ops.len();
        self.ops.retain(|op| op.id != id);
        self.ops.len() != before
    }

    /// The pending operation `path` lives at or under, if any: a write to it
    /// would recreate a moved path or land in a deleted one.
    pub fn blocking_write(&self, path: &Path) -> Option<PathOpConflict> {
        self.find(|op, root| path_is_under(path, root, op.fold))
    }

    /// As [`Self::blocking_write`], and also an operation that sits below
    /// `path`: moving or deleting a directory while something inside it is
    /// being moved collides with that move.
    pub fn blocking_start(&self, path: &Path) -> Option<PathOpConflict> {
        self.find(|op, root| {
            path_is_under(path, root, op.fold) || path_is_under(root, path, op.fold)
        })
    }

    /// Is anything running?
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Is a library rename or delete running? Library edits write through
    /// the mounted adapter, which still points at the old path until a
    /// rename lands (and at a vanishing directory during a delete).
    pub fn library_rename_running(&self) -> bool {
        self.ops.iter().any(|op| {
            op.kind == PathOpKind::RenameLibrary
                || (op.kind == PathOpKind::Delete
                    && op
                        .paths
                        .iter()
                        .any(|p| p.extension().is_some_and(|e| e == "snxlib")))
        })
    }

    fn find(&self, hit: impl Fn(&PendingPathOp, &Path) -> bool) -> Option<PathOpConflict> {
        self.ops.iter().find_map(|op| {
            let primary = op.paths.first()?;
            let conflict = |path: &PathBuf, dependent: bool| PathOpConflict {
                kind: op.kind,
                path: if dependent {
                    primary.clone()
                } else {
                    path.clone()
                },
                dependent,
            };
            op.paths
                .iter()
                .find(|root| hit(op, root))
                .map(|root| conflict(root, false))
                .or_else(|| {
                    op.dependents
                        .iter()
                        .find(|root| hit(op, root))
                        .map(|root| conflict(root, true))
                })
        })
    }
}

fn leaf(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Reason-first refusal text: the Messages panel cuts a line near 160 bytes,
/// so the cause leads and only file names follow.
fn refusal(conflict: &PathOpConflict, path: &Path, outcome: &str) -> String {
    let moved = leaf(&conflict.path);
    let subject = leaf(path);
    if conflict.dependent {
        format!(
            "{subject} cannot be {outcome} while the {} of {moved} is still rewriting it. \
             Try again once it finishes.",
            conflict.kind.noun()
        )
    } else if moved == subject {
        format!(
            "A {} of {moved} is still running, so it was not {outcome}. \
             Try again once it finishes.",
            conflict.kind.noun()
        )
    } else {
        format!(
            "A {} of {moved} is still running, so {subject} was not {outcome}. \
             Try again once it finishes.",
            conflict.kind.noun()
        )
    }
}

impl crate::app::Signex {
    /// Refuse to write `path` (`outcome` is "saved", "restored", ...) while a
    /// rename or delete of it, or of a directory above it, is in flight.
    pub(crate) fn refuse_while_path_op(&self, path: &Path, outcome: &str) -> anyhow::Result<()> {
        match self.document_state.pending_path_ops.blocking_write(path) {
            Some(conflict) => anyhow::bail!(refusal(&conflict, path, outcome)),
            None => Ok(()),
        }
    }

    /// Refuse to start a second operation (or open) on `path` while another
    /// one overlaps it.
    pub(crate) fn refuse_start_while_path_op(
        &self,
        path: &Path,
        outcome: &str,
    ) -> anyhow::Result<()> {
        match self.document_state.pending_path_ops.blocking_start(path) {
            Some(conflict) => anyhow::bail!(refusal(&conflict, path, outcome)),
            None => Ok(()),
        }
    }
}

// ---- rename file / project --------------------------------------------------

/// One parent-sheet reference to re-point: `parent` holds a child entry whose
/// `filename` is currently `from` and must become `to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReferenceRewrite {
    pub parent: PathBuf,
    pub from: String,
    pub to: String,
}

/// What a rename has to do to the rest of the project, planned before the
/// file moves.
#[derive(Debug, Clone, Default)]
pub struct ReferencePlan {
    pub rewrites: Vec<ReferenceRewrite>,
    /// Project sheets that could not be read or parsed, so whether they
    /// reference the renamed file is unknown. Reported, never skipped silently.
    pub unchecked: Vec<String>,
}

/// Which file the rename moves.
#[derive(Debug, Clone)]
pub enum FileRenameKind {
    /// A sheet, PCB or primitive: parent references are re-pointed.
    Document(ReferencePlan),
}

/// Everything a file rename's completion needs, captured when it started.
#[derive(Debug, Clone)]
pub struct FileRenameRequest {
    pub id: PathOpId,
    pub from: PathBuf,
    pub to: PathBuf,
    /// The new file name.
    pub new_name: String,
    /// The project that owns the file, by id: its index can shift meanwhile.
    pub owner: Option<ProjectId>,
    pub kind: FileRenameKind,
}

/// Why the disk move did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameIoError {
    TargetExists,
    /// The OS error text of `fs::rename`, or of the worker that ran it.
    Os(String),
}

/// Parent sheets read for the reference rewrite, keyed by their path.
pub type ParentSheets = HashMap<PathBuf, Result<Box<SchematicSheet>, String>>;

#[derive(Debug, Clone)]
pub struct FileRenameDone {
    pub request: FileRenameRequest,
    pub result: Result<(), RenameIoError>,
    pub parents: ParentSheets,
}

// ---- rename library ---------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LibraryRenameRequest {
    pub id: PathOpId,
    pub old: PathBuf,
    pub new: PathBuf,
    pub project: ProjectId,
    pub project_path: PathBuf,
}

/// What the library rename worker did on disk.
#[derive(Debug, Clone)]
pub enum LibraryRenameIo {
    /// The destination already exists; nothing changed.
    TargetExists,
    /// `fs::rename` failed with this OS text; nothing changed.
    NotRenamed(String),
    /// The library moved and the saved `.snxprj` was patched as described.
    Moved(LibraryRetarget),
    /// The library moved but the project file could not be patched, so the
    /// worker moved it back. `Err` carries the move-back failure: the library
    /// is stranded at the new path.
    ProjectNotPatched {
        reason: String,
        moved_back: Result<(), String>,
    },
}

#[derive(Debug, Clone)]
pub struct LibraryRenameDone {
    pub request: LibraryRenameRequest,
    pub io: LibraryRenameIo,
}

/// The follow-up worker that undoes a library rename the app could not follow.
#[derive(Debug, Clone)]
pub struct LibraryRollbackDone {
    pub request: LibraryRenameRequest,
    /// Why the app could not follow the rename.
    pub reason: String,
    /// `Err` carries text for the failure reason: the saved project still
    /// lists the new path.
    pub patch_undo: Result<(), String>,
    pub moved_back: Result<(), String>,
}

// ---- delete -------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DeleteRequest {
    pub id: PathOpId,
    pub target: PathBuf,
    pub display_name: String,
    pub fold: CaseFold,
    /// The Remove dialog the delete came from: the completion prunes the
    /// project entry from it once the file is gone.
    pub dialog: crate::app::RemoveDialogState,
}

/// What the delete worker found or did.
#[derive(Debug, Clone)]
pub enum DeleteIo {
    /// Nothing on disk: an orphan entry.
    Orphan,
    /// The target is outside the project or a shared library; not deleted.
    Refused,
    Deleted,
    /// `remove_file` / `remove_dir_all` failed with this OS text.
    Failed(String),
    /// Whether the target exists could not be determined.
    CheckFailed(String),
}

#[derive(Debug, Clone)]
pub struct DeleteDone {
    pub request: DeleteRequest,
    pub io: DeleteIo,
}

/// Drive a message and every task it produces to completion without an iced
/// runtime, for unit tests of flows whose result lands on a worker (#859).
#[cfg(test)]
pub(crate) fn settle(app: &mut crate::app::Signex, message: crate::app::Message) {
    use futures::StreamExt as _;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut pending = vec![app.update(message)];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(next) = action {
                    messages.push(next);
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn a_pending_directory_blocks_writes_below_it_only() {
        let mut ops = PendingPathOps::default();
        ops.begin(
            PathOpKind::Delete,
            vec![p("/w/lib.snxlib")],
            CaseFold::PRESERVE,
        );
        assert!(ops.blocking_write(&p("/w/lib.snxlib/a.snxsym")).is_some());
        assert!(ops.blocking_write(&p("/w/lib.snxlib")).is_some());
        assert!(ops.blocking_write(&p("/w/lib.snxlib2/a")).is_none());
        assert!(ops.blocking_write(&p("/w")).is_none());
    }

    #[test]
    fn starting_an_operation_also_collides_with_one_inside_it() {
        let mut ops = PendingPathOps::default();
        ops.begin(
            PathOpKind::Rename,
            vec![p("/w/a/x.snxsch")],
            CaseFold::PRESERVE,
        );
        assert!(ops.blocking_start(&p("/w/a")).is_some());
        assert!(ops.blocking_write(&p("/w/a")).is_none());
    }

    #[test]
    fn a_rename_blocks_its_destination_too() {
        let mut ops = PendingPathOps::default();
        ops.begin(
            PathOpKind::Rename,
            vec![p("/w/a.snxsch"), p("/w/b.snxsch")],
            CaseFold::PRESERVE,
        );
        assert!(ops.blocking_write(&p("/w/b.snxsch")).is_some());
    }

    #[test]
    fn finishing_releases_the_path_and_reports_unknown_ids() {
        let mut ops = PendingPathOps::default();
        let id = ops.begin(PathOpKind::Delete, vec![p("/w/a")], CaseFold::PRESERVE);
        assert!(ops.finish(id));
        assert!(ops.blocking_write(&p("/w/a")).is_none());
        assert!(!ops.finish(id));
    }

    #[test]
    fn the_refusal_leads_with_the_reason() {
        let conflict = PathOpConflict {
            kind: PathOpKind::Rename,
            path: p("/w/a.snxsch"),
            dependent: false,
        };
        let text = refusal(&conflict, &p("/w/a.snxsch"), "saved");
        assert!(text.starts_with("A rename of a.snxsch is still running"));
        assert!(text.len() < 157, "{text}");
    }
}
