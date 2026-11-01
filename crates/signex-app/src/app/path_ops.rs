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
    /// Annotate or Reset Duplicate Designators reading or rewriting the
    /// project's sheets.
    ProjectRewrite,
    /// Add New Schematic writing a blank sheet and listing it in the saved
    /// project file.
    AddSheet,
    /// A sheet Save As re-pointing the project file's entry for the sheet.
    SaveAsSheet,
}

impl PathOpKind {
    fn noun(self) -> &'static str {
        match self {
            Self::Rename | Self::RenameLibrary => "rename",
            Self::Delete => "delete",
            Self::ProjectRewrite => "project-wide rewrite",
            Self::AddSheet => "new-sheet add",
            Self::SaveAsSheet => "Save As",
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

    /// A path being deleted at or under `dir`, if any.
    pub fn delete_under(&self, dir: &Path) -> Option<PathBuf> {
        self.ops
            .iter()
            .filter(|op| op.kind == PathOpKind::Delete)
            .flat_map(|op| op.paths.iter().map(move |p| (p, op.fold)))
            .find(|(p, fold)| path_is_under(p, dir, *fold))
            .map(|(p, _)| p.clone())
    }

    /// Finish the [`PathOpKind::SaveAsSheet`] operation whose new sheet is
    /// `sheet`. Returns whether one was registered.
    pub fn finish_save_as_sheet(&mut self, sheet: &Path) -> bool {
        let before = self.ops.len();
        self.ops.retain(|op| {
            !(op.kind == PathOpKind::SaveAsSheet
                && op.paths.first().map(PathBuf::as_path) == Some(sheet))
        });
        self.ops.len() != before
    }

    /// Finish the [`PathOpKind::AddSheet`] operation that creates `sheet`.
    /// Returns whether one was registered.
    pub fn finish_add_sheet(&mut self, sheet: &Path) -> bool {
        let before = self.ops.len();
        self.ops.retain(|op| {
            !(op.kind == PathOpKind::AddSheet
                && op.paths.first().map(PathBuf::as_path) == Some(sheet))
        });
        self.ops.len() != before
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

    /// Is an Annotate or Reset Duplicate Designators run holding sheets?
    pub fn rewrite_running(&self) -> bool {
        self.ops
            .iter()
            .any(|op| op.kind == PathOpKind::ProjectRewrite)
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
                        .any(|p| crate::app::file_extension::has_extension(p, "snxlib")))
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
            "{subject} cannot be {outcome} while the {} of {moved} updates the project's files. \
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

    /// Why the project at `project_id` cannot be saved or closed now, when a
    /// delete under its folder is pending: its entry is pruned when the delete
    /// finishes, and the project file would meanwhile still list the file.
    pub(crate) fn refuse_project_while_delete_pending(
        &self,
        project_id: crate::app::state::ProjectId,
        outcome: &str,
    ) -> anyhow::Result<()> {
        let Some(project) = self
            .document_state
            .projects
            .iter()
            .find(|p| p.id == project_id)
        else {
            return Ok(());
        };
        match self
            .document_state
            .pending_path_ops
            .delete_under(project.dir())
        {
            Some(path) => anyhow::bail!(
                "A delete of {} is still running, so the project {} was not {outcome}. \
                 Try again once it finishes.",
                leaf(&path),
                project.data.name
            ),
            None => Ok(()),
        }
    }

    /// Refuse a project-wide rewrite (Annotate, project Replace) while a
    /// rename or delete touches one of `sheets`, a declared page of the
    /// active project, or anything under its folder: a sheet the worker has
    /// already moved is in neither the scanned set nor its unreadable list.
    pub(crate) fn refuse_project_wide_while_path_op<'a>(
        &self,
        sheets: impl IntoIterator<Item = &'a Path>,
        outcome: &str,
    ) -> anyhow::Result<()> {
        for path in sheets {
            self.refuse_while_path_op(path, outcome)?;
        }
        for path in crate::app::project_sheets::declared_pages(&self.document_state) {
            self.refuse_while_path_op(&path, outcome)?;
        }
        let Some(dir) = self
            .document_state
            .active_document_project()
            .map(|p| p.dir())
        else {
            return Ok(());
        };
        match self.document_state.pending_path_ops.blocking_start(dir) {
            Some(conflict) => anyhow::bail!(refusal(&conflict, dir, outcome)),
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
    /// Parents of a rewrite that are read-only (written by a newer Signex),
    /// so the engine would refuse the re-point. The rename must not start
    /// when this is non-empty: the file would move and the reference stay.
    pub read_only_parents: Vec<PathBuf>,
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
    /// The file moved but the saved `.snxprj` could not be updated to follow,
    /// so the worker moved the file back (#1821). `moved_back` is that
    /// attempt's result: when it failed the file is still at the new name.
    ProjectNotPatched {
        reason: String,
        moved_back: Result<(), String>,
    },
}

/// Parent sheets read for the reference rewrite, keyed by their path.
pub type ParentSheets = HashMap<PathBuf, Result<Box<SchematicSheet>, String>>;

#[derive(Debug, Clone)]
pub struct FileRenameDone {
    pub request: FileRenameRequest,
    /// What the move did to the saved `.snxprj` entry (#1821).
    pub result: Result<crate::app::project_entry_disk::EntryPatch, RenameIoError>,
    pub parents: ParentSheets,
    /// What the move did to the saved parent sheets' references (#1849).
    pub parent_disk: crate::app::parent_reference_disk::ParentDiskOutcomes,
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

/// One sheet symbol that places a file: the sheet it sits on and its name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlacingSymbol {
    pub parent: PathBuf,
    pub name: String,
    /// `ChildSheet.uuid`, so two symbols of one name stay two.
    pub id: uuid::Uuid,
}

impl PlacingSymbol {
    /// `"name" on parent.snxsch`, for the dialog and Messages.
    pub fn describe(&self) -> String {
        let sheet = self.parent.file_name().map_or_else(
            || self.parent.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        format!("\"{}\" on {sheet}", self.name)
    }

    /// Several symbols as one sentence fragment.
    pub fn describe_all(symbols: &[PlacingSymbol]) -> String {
        symbols
            .iter()
            .map(PlacingSymbol::describe)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone)]
pub struct DeleteRequest {
    pub id: PathOpId,
    pub target: PathBuf,
    pub display_name: String,
    pub fold: CaseFold,
    /// The Remove dialog the delete came from: the completion prunes the
    /// project entry from it once the file is gone.
    pub dialog: crate::app::RemoveDialogState,
    /// Sheet symbols that placed the file when the delete began; the ones
    /// that are left pointing at nothing once it is gone.
    pub placing: Vec<PlacingSymbol>,
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
    /// As `Failed`, and the saved project no longer lists the file either: its
    /// entry was removed first and could not be put back (#1849). The in-memory
    /// project still lists it, so a project save writes the entry back.
    FailedProjectStale(String),
    /// Whether the target exists could not be determined.
    CheckFailed(String),
}

#[derive(Debug, Clone)]
pub struct DeleteDone {
    pub request: DeleteRequest,
    pub io: DeleteIo,
    /// What the delete did to the saved `.snxprj` entry (#1849).
    pub entry: crate::app::project_entry_disk::EntryPatch,
}

/// Drive a message and every task it produces to completion without an iced
/// runtime, for unit tests of flows whose result lands on a worker (#859).
#[cfg(test)]
pub(crate) fn settle(app: &mut crate::app::Signex, message: crate::app::Message) {
    let task = app.update(message);
    settle_task(app, task);
}

/// [`settle`] for a task a handler returned directly.
#[cfg(test)]
pub(crate) fn settle_task(app: &mut crate::app::Signex, task: iced::Task<crate::app::Message>) {
    use futures::StreamExt as _;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut pending = vec![task];
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
