//! Recovery-copy moves that have not finished (#1510).
//!
//! A copy is keyed by its document's full path, so renaming a document or
//! its folder owes the copy a move to the new key. The move can fail (a
//! scanner holds the file, a permission) and keep failing until the app
//! quits; nothing in memory survives that. Each owed move is therefore
//! journalled next to the recovery store as a small file written with
//! [`signex_types::atomic_io::atomic_write`], one file per SOURCE key
//! (the document the copy leaves), so a later move of the same copy
//! replaces the entry instead of adding a second one. The next start
//! finishes the move ([`finish_all`], before any document opens, never while
//! the live autosave state owns the moves) and clears the entry. The entry
//! is also cleared when the owed move is dropped (the document came back, or
//! its copy was removed).
//!
//! A copy is never deleted here: it is only renamed onto a free new key.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::sidecar::sidecar_path;

/// Extension of a journal entry file.
pub const ENTRY_EXTENSION: &str = "move";

#[derive(Serialize, Deserialize)]
struct Entry {
    old: PathBuf,
    new: PathBuf,
}

fn entry_path(recovery_dir: &Path, old: &Path) -> PathBuf {
    let mut name = sidecar_path(recovery_dir, old).into_os_string();
    name.push(".");
    name.push(ENTRY_EXTENSION);
    PathBuf::from(name)
}

/// Journal that the copy of `old` is owed a move to the key of `new`,
/// replacing any earlier entry for `old`.
pub fn record(recovery_dir: &Path, old: &Path, new: &Path) -> Result<(), String> {
    let entry = Entry {
        old: old.to_path_buf(),
        new: new.to_path_buf(),
    };
    let bytes = serde_json::to_vec(&entry).map_err(|e| format!("pending move not saved: {e}"))?;
    let file = entry_path(recovery_dir, old);
    signex_types::atomic_io::atomic_write(&file, &bytes).map_err(|e| {
        format!(
            "{e}: could not journal the recovery copy move from {}",
            file.display()
        )
    })
}

/// Drop the journal entry of the copy leaving `old`. Already gone is success.
pub fn clear(recovery_dir: &Path, old: &Path) {
    let file = entry_path(recovery_dir, old);
    match std::fs::remove_file(&file) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => crate::diagnostics::log_warning(format!(
            "{e}: could not clear the finished move entry {}",
            file.display()
        )),
    }
}

/// Every journalled move as (old, new) documents, for a folder rename that
/// finds a document's copy still at the key of an earlier name.
pub fn owed(recovery_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let Ok(dir) = std::fs::read_dir(recovery_dir) else {
        return Vec::new();
    };
    dir.flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ENTRY_EXTENSION))
        .filter_map(|file| {
            let entry: Entry = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
            Some((entry.old, entry.new))
        })
        .collect()
}

/// Outcome of the startup pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Finished {
    /// Copies now under their new key.
    pub moved: usize,
    /// Moves that still fail, as (old, new) documents; their entries stay.
    pub failed: Vec<(PathBuf, PathBuf)>,
}

/// Finish every journalled move. Run once at startup, before any document
/// opens: from then on the live autosave state owns the moves and rewrites
/// or clears the entries as it goes. An entry that cannot be read is logged
/// and left.
pub fn finish_all(recovery_dir: &Path) -> Finished {
    let mut report = Finished::default();
    let dir = match std::fs::read_dir(recovery_dir) {
        Ok(dir) => dir,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return report,
        Err(e) => {
            crate::diagnostics::log_warning(format!(
                "{e}: could not list {} to finish pending recovery copy moves",
                recovery_dir.display()
            ));
            return report;
        }
    };
    for file in dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ENTRY_EXTENSION))
    {
        let entry = std::fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| serde_json::from_slice::<Entry>(&bytes).map_err(|e| e.to_string()));
        match entry {
            Ok(entry) => run_one(recovery_dir, &file, &entry, &mut report),
            Err(why) => crate::diagnostics::log_warning(format!(
                "The pending recovery copy move {} could not be read ({why}); it was left.",
                file.display()
            )),
        }
    }
    report
}

/// Whether the document moved from `old` to `new`: a file is at `new` and
/// none is left at `old` (or `old` names the same file, a case-only rename).
fn document_moved(old: &Path, new: &Path) -> bool {
    if !new.is_file() {
        return false;
    }
    if !old.exists() {
        return true;
    }
    matches!(
        (std::fs::canonicalize(old), std::fs::canonicalize(new)),
        (Ok(a), Ok(b)) if a == b
    )
}

fn run_one(recovery_dir: &Path, file: &Path, entry: &Entry, report: &mut Finished) {
    let (src, dst) = (
        sidecar_path(recovery_dir, &entry.old),
        sidecar_path(recovery_dir, &entry.new),
    );
    let drop_entry = || {
        if let Err(e) = std::fs::remove_file(file)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            crate::diagnostics::log_warning(format!("{e}: could not clear {}", file.display()));
        }
    };
    if src == dst || !src.is_file() {
        // Nothing left at the old key: already moved, or never existed.
        drop_entry();
        return;
    }
    // The entry may be stale: the folder came back, or moved on again. Only
    // a document that now lives at `new` (and no longer at `old`) owns the
    // move; otherwise the copy stays at the old key, where it is offered.
    if !document_moved(&entry.old, &entry.new) {
        crate::diagnostics::log_warning(format!(
            "The pending move of recovery copy {} was dropped: {} is not where the document \
             moved to. The copy stays under its old name.",
            src.display(),
            entry.new.display()
        ));
        drop_entry();
        return;
    }
    if dst.is_file() {
        crate::diagnostics::log_warning(format!(
            "The recovery copy {} was not moved to {}: a copy already sits under the new name. \
             It is still on disk as {} and is not offered; copy it out by hand if it matters.",
            src.display(),
            dst.display(),
            src.display()
        ));
        drop_entry();
        return;
    }
    match std::fs::rename(&src, &dst) {
        Ok(()) => {
            report.moved += 1;
            drop_entry();
        }
        Err(e) => {
            report.failed.push((entry.old.clone(), entry.new.clone()));
            crate::diagnostics::log_warning(format!(
                "{e}: the recovery copy {} is still owed a move to {}; retried.",
                src.display(),
                dst.display()
            ));
        }
    }
}
