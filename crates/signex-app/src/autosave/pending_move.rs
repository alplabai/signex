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
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::sidecar::sidecar_path;

/// Most entries one startup pass finishes. The pass is synchronous on the
/// boot thread, so a huge journal must not hold the first frame; the rest
/// stay journalled for the next start.
pub const MAX_ENTRIES_PER_PASS: usize = 256;

/// Serialises every write and removal of a journal entry, so a conditional
/// clear cannot delete an entry a record wrote between its read and its
/// remove. Guards file IO only; it holds no application state.
static JOURNAL_IO: Mutex<()> = Mutex::new(());

fn journal_io() -> std::sync::MutexGuard<'static, ()> {
    JOURNAL_IO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    let _io = journal_io();
    signex_types::atomic_io::atomic_write(&file, &bytes).map_err(|e| {
        format!(
            "{e}: could not journal the recovery copy move from {}",
            file.display()
        )
    })
}

/// Drop the journal entry of the copy leaving `old`. Already gone is success.
pub fn clear(recovery_dir: &Path, old: &Path) {
    let _io = journal_io();
    remove_entry(&entry_path(recovery_dir, old));
}

/// Remove an entry file; the caller holds the journal lock.
fn remove_entry(file: &Path) {
    match std::fs::remove_file(file) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => crate::diagnostics::log_warning(format!(
            "{e}: could not clear the finished move entry {}",
            file.display()
        )),
    }
}

/// Drop the entry of the copy leaving `old` only if it still names one of
/// `targets`. A newer entry for the same copy (a later rename) survives a
/// clear that was queued for an older one.
pub fn clear_if(recovery_dir: &Path, old: &Path, targets: &[PathBuf]) {
    let file = entry_path(recovery_dir, old);
    // Read and remove under one lock: a record cannot land in between.
    let _io = journal_io();
    let entry = match std::fs::read(&file) {
        Ok(bytes) => serde_json::from_slice::<Entry>(&bytes).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => Err(e.to_string()),
    };
    match entry {
        Ok(entry)
            if targets.iter().any(|t| {
                sidecar_path(recovery_dir, t) == sidecar_path(recovery_dir, &entry.new)
            }) =>
        {
            remove_entry(&file);
        }
        Ok(_) => {}
        Err(why) => crate::diagnostics::log_warning(format!(
            "The pending recovery copy move {} could not be read ({why}); it was left.",
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
            let entry = std::fs::read(&file)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Entry>(&bytes).map_err(|e| e.to_string())
                });
            match entry {
                Ok(entry) => Some((entry.old, entry.new)),
                Err(why) => {
                    crate::diagnostics::log_warning(format!(
                        "The pending recovery copy move {} could not be read ({why}); it was \
                         left.",
                        file.display()
                    ));
                    None
                }
            }
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
    let mut files: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ENTRY_EXTENSION))
        .collect();
    if files.len() > MAX_ENTRIES_PER_PASS {
        // Start each pass at a different entry, so moves that keep failing
        // cannot fill every pass and starve the rest.
        files.sort();
        let start = pass_start(files.len());
        files.rotate_left(start);
        crate::diagnostics::log_warning(format!(
            "{} pending recovery copy moves are journalled; {} were left for the next start.",
            files.len(),
            files.len() - MAX_ENTRIES_PER_PASS
        ));
    }
    for file in files.into_iter().take(MAX_ENTRIES_PER_PASS) {
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

/// Where a capped pass starts in the sorted entry list: it varies from start
/// to start, so every entry is reached within a few starts.
fn pass_start(len: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    nanos % len.max(1)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A conditional clear queued for an older target must never delete the
    /// newer entry a record writes at the same moment. Statistical: the race
    /// window is small, so it repeats; with the lock it can never fail.
    #[test]
    fn a_conditional_clear_never_deletes_a_concurrent_newer_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let old = tmp.path().join("a.snxsch");
        let (older, newer) = (tmp.path().join("b.snxsch"), tmp.path().join("c.snxsch"));
        for _ in 0..300 {
            record(&rec, &old, &older).unwrap();
            std::thread::scope(|s| {
                s.spawn(|| clear_if(&rec, &old, std::slice::from_ref(&older)));
                s.spawn(|| record(&rec, &old, &newer).unwrap());
            });
            assert_eq!(owed(&rec), vec![(old.clone(), newer.clone())]);
        }
    }

    #[test]
    fn a_startup_pass_finishes_at_most_the_cap_and_leaves_the_rest_journalled() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let total = MAX_ENTRIES_PER_PASS + 3;
        for i in 0..total {
            let new = tmp.path().join(format!("new/{i}.snxsch"));
            std::fs::create_dir_all(new.parent().unwrap()).unwrap();
            std::fs::write(&new, b"saved").unwrap();
            let old = tmp.path().join(format!("old/{i}.snxsch"));
            super::super::sidecar::write(&rec, &old, b"x").unwrap();
            record(&rec, &old, &new).unwrap();
        }
        assert_eq!(finish_all(&rec).moved, MAX_ENTRIES_PER_PASS);
        assert_eq!(owed(&rec).len(), 3);
        assert_eq!(finish_all(&rec).moved, 3);
    }
}
