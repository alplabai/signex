//! Writing a primitive editor's container back to disk (#902).
//!
//! The editor holds a whole `.snxsym` / `.snxfpt` in memory. Writing that
//! copy back would also write stale copies of the primitives it never
//! touched, dropping whatever another Signex window saved to them since. So
//! a save re-reads the file and merges only this editor's changes into it
//! (`signex_library::primitive::container_merge`), under the owning
//! library's cross-process write lock when the file belongs to one.

use super::cascade::SaveLibrary;
use signex_library::LibraryAdapter as _;
use signex_library::{ContainerMerge, FootprintFile, LibraryError, SymbolFile};

/// How a container save treats the file already at the path.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerWrite {
    /// The editor's own file: merge its changes into what is on disk.
    Merge,
    /// A file the user chose to replace (Save As): write the editor's
    /// container whole.
    Replace,
}

/// How a container save writes.
#[derive(Clone, Copy)]
pub(super) struct SaveOptions {
    pub mode: ContainerWrite,
    /// Patch-bump the version of each primitive the save changed, in the
    /// same write (#832).
    pub bump_versions: bool,
}

/// Outcome of a container save.
pub(super) struct ContainerSave {
    pub merge: ContainerMerge,
    /// The new version of each primitive the save bumped (#832), by uuid.
    pub versions: std::collections::HashMap<uuid::Uuid, String>,
}

fn rewrite_primitive_file(
    library: &SaveLibrary,
    path: &std::path::Path,
    commit_message: &str,
    rewrite: &mut signex_library::FileRewrite<'_>,
) -> Result<bool, LibraryError> {
    // The adapter commits before it releases the lock.
    if let SaveLibrary::Mounted(adapter) = library {
        return adapter.rewrite_file_locked(path, commit_message, rewrite);
    }
    // A file opened on its own may still sit in a library another
    // window has mounted; take that library's lock too.
    match signex_library::enclosing_library_file(path) {
        Some(library) => signex_library::rewrite_file_under_library_lock(&library, path, rewrite),
        None => signex_library::rewrite_file(path, rewrite),
    }
}

pub(super) fn save_symbol_container(
    library: &SaveLibrary,
    path: &std::path::Path,
    editor: &SymbolFile,
    baseline: &SymbolFile,
    now: chrono::DateTime<chrono::Utc>,
    options: SaveOptions,
    commit_message: &str,
) -> Result<ContainerSave, LibraryError> {
    let mut merge = ContainerMerge::default();
    let mut versions = std::collections::HashMap::new();
    rewrite_primitive_file(library, path, commit_message, &mut |current| {
        let whole = || {
            (
                editor.clone(),
                whole_file(editor.symbols.iter().map(|s| s.uuid)),
            )
        };
        let (mut file, m) = match current {
            Some(bytes) if options.mode == ContainerWrite::Merge && !holds_nothing(bytes) => {
                let disk = SymbolFile::from_bytes(bytes).map_err(|e| unreadable_on_disk(&e))?;
                editor
                    .merge_into(baseline, disk, now)
                    .or_else(|duplicate| duplicated_uuids(path, duplicate, whole()))?
            }
            _ => whole(),
        };
        if file.symbols.is_empty() {
            return Err(would_be_empty("symbol"));
        }
        merge = m;
        if merge.is_empty() {
            return Ok(None);
        }
        // Bump the primitives this save changed in the same write, so
        // the file never holds an edit at its old version (#832).
        if options.bump_versions {
            versions = super::cascade::bump_symbol_versions(&mut file, &merge.changed);
        }
        file.to_toml_string()
            .map(|text| Some(text.into_bytes()))
            .map_err(|e| LibraryError::Backend(format!("serialize symbol library: {e}")))
    })?;
    Ok(ContainerSave { merge, versions })
}

pub(super) fn save_footprint_container(
    library: &SaveLibrary,
    path: &std::path::Path,
    editor: &FootprintFile,
    baseline: &FootprintFile,
    now: chrono::DateTime<chrono::Utc>,
    options: SaveOptions,
    commit_message: &str,
) -> Result<ContainerSave, LibraryError> {
    let mut merge = ContainerMerge::default();
    let mut versions = std::collections::HashMap::new();
    rewrite_primitive_file(library, path, commit_message, &mut |current| {
        let whole = || {
            (
                editor.clone(),
                whole_file(editor.footprints.iter().map(|f| f.uuid)),
            )
        };
        let (mut file, m) = match current {
            Some(bytes) if options.mode == ContainerWrite::Merge && !holds_nothing(bytes) => {
                let disk = FootprintFile::from_bytes(bytes).map_err(|e| unreadable_on_disk(&e))?;
                editor
                    .merge_into(baseline, disk, now)
                    .or_else(|duplicate| duplicated_uuids(path, duplicate, whole()))?
            }
            _ => whole(),
        };
        if file.footprints.is_empty() {
            return Err(would_be_empty("footprint"));
        }
        merge = m;
        if merge.is_empty() {
            return Ok(None);
        }
        // Bump the primitives this save changed in the same write, so
        // the file never holds an edit at its old version (#832).
        if options.bump_versions {
            versions = super::cascade::bump_footprint_versions(&mut file, &merge.changed);
        }
        file.to_toml_string()
            .map(|text| Some(text.into_bytes()))
            .map_err(|e| LibraryError::Backend(format!("serialize footprint library: {e}")))
    })?;
    Ok(ContainerSave { merge, versions })
}

/// An empty file (created before its first save) has nothing to merge
/// with or to lose.
fn holds_nothing(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
}

/// A file that does not exist yet, is empty, or is being replaced, is
/// written whole.
fn whole_file(uuids: impl Iterator<Item = uuid::Uuid>) -> ContainerMerge {
    ContainerMerge {
        changed: uuids.collect(),
        overwritten: Vec::new(),
    }
}

/// A container holding two primitives with one uuid cannot be merged by
/// uuid. When the file had them already (an old or hand-edited file), it is
/// written whole, as before #902, and the user is told. When only the file
/// on disk has them, another writer added them since this editor loaded it,
/// and writing over it would drop that writer's change, so the save is
/// refused.
fn duplicated_uuids<F>(
    path: &std::path::Path,
    duplicate: signex_library::DuplicatedUuid,
    whole: (F, ContainerMerge),
) -> Result<(F, ContainerMerge), LibraryError> {
    match duplicate {
        signex_library::DuplicatedUuid::InEditorCopy => {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                "saved the whole file, not merged: it holds two primitives with one uuid, so a \
                 change another window saved to it since it was opened is lost: {}",
                path.display(),
            );
            Ok(whole)
        }
        signex_library::DuplicatedUuid::OnDisk => Err(LibraryError::Backend(
            "the save was refused so the file is not overwritten: another program gave two \
             of its primitives one uuid since it was opened"
                .into(),
        )),
    }
}

/// The file on disk no longer parses (another program wrote it). Refuse
/// rather than overwrite it with this editor's copy.
fn unreadable_on_disk(error: &dyn std::fmt::Display) -> LibraryError {
    LibraryError::Backend(format!(
        "the save was refused so the file is not overwritten: it no longer parses on disk \
         ({error})"
    ))
}

/// Each window deleted a different primitive, and together they would
/// leave none. A container with no primitive cannot be opened again.
fn would_be_empty(kind: &str) -> LibraryError {
    LibraryError::Backend(format!(
        "the save was refused: it would leave no {kind} in the file, which could then not be \
         opened again (other windows deleted the rest since it was opened here)"
    ))
}

/// Tell the user which primitives this save replaced or deleted after
/// another window had changed them. The editor's version wins: it is what
/// the user just saved.
pub(super) fn report_overwritten(path: &std::path::Path, replaced: &[String], deleted: &[String]) {
    for (names, done) in [(replaced, "replaced"), (deleted, "deleted")] {
        if names.is_empty() {
            continue;
        }
        // The action leads: the Messages panel cuts long lines, and the
        // path is the long part.
        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            "this save {done} another window's change to {}: {}",
            names.join(", "),
            path.display(),
        );
    }
}
