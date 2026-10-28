//! Recovery-copy moves that have not finished (#1510).
//!
//! A copy is keyed by its document's full path, so renaming a document or
//! its folder owes the copy a move to the new key. The move can fail (a
//! scanner holds the file, a permission) and keep failing until the app
//! quits; nothing in memory survives that. Each owed move is therefore
//! journalled next to the recovery store as a small file written with
//! [`signex_types::atomic_io::atomic_write`], one file per destination key
//! so concurrent writers never share a file to rewrite. The next run
//! finishes the move ([`finish_all`], and [`finish_for`] when the document
//! opens) and clears the entry.
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

fn entry_path(recovery_dir: &Path, new: &Path) -> PathBuf {
    let mut name = sidecar_path(recovery_dir, new).into_os_string();
    name.push(".");
    name.push(ENTRY_EXTENSION);
    PathBuf::from(name)
}

/// Journal that the copy of `old` is owed a move to the key of `new`.
pub fn record(recovery_dir: &Path, old: &Path, new: &Path) -> Result<(), String> {
    let entry = Entry {
        old: old.to_path_buf(),
        new: new.to_path_buf(),
    };
    let bytes = serde_json::to_vec(&entry).map_err(|e| format!("pending move not saved: {e}"))?;
    let file = entry_path(recovery_dir, new);
    signex_types::atomic_io::atomic_write(&file, &bytes).map_err(|e| {
        format!(
            "{e}: could not journal the recovery copy move to {}",
            file.display()
        )
    })
}

/// Drop the journal entry for the move to `new`. Already gone is success.
pub fn clear(recovery_dir: &Path, new: &Path) {
    let file = entry_path(recovery_dir, new);
    match std::fs::remove_file(&file) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => crate::diagnostics::log_warning(format!(
            "{e}: could not clear the finished move entry {}",
            file.display()
        )),
    }
}

fn entries(recovery_dir: &Path) -> Vec<(PathBuf, Entry)> {
    let Ok(dir) = std::fs::read_dir(recovery_dir) else {
        return Vec::new();
    };
    dir.flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ENTRY_EXTENSION))
        .filter_map(|file| {
            let entry = serde_json::from_slice(&std::fs::read(&file).ok()?).ok()?;
            Some((file, entry))
        })
        .collect()
}

/// Outcome of one finishing pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Finished {
    /// Copies now under their new key.
    pub moved: usize,
    /// Entries whose move still fails; kept for the next run.
    pub failed: usize,
}

/// Finish every journalled move. A chain (A to B, then B to C) is walked in
/// order whatever the directory order. Entries that cannot finish stay.
pub fn finish_all(recovery_dir: &Path) -> Finished {
    let mut report = Finished::default();
    let mut todo = entries(recovery_dir);
    loop {
        let feeds: Vec<PathBuf> = todo
            .iter()
            .map(|(_, e)| sidecar_path(recovery_dir, &e.new))
            .collect();
        let (waiting, ready): (Vec<_>, Vec<_>) = todo.into_iter().partition(|(_, e)| {
            let src = sidecar_path(recovery_dir, &e.old);
            !src.is_file() && feeds.contains(&src)
        });
        // Nothing ready means a cycle of waits: run them all as they are.
        let stuck = ready.is_empty();
        for (file, entry) in ready
            .iter()
            .chain(if stuck { waiting.iter() } else { [].iter() })
        {
            run_one(recovery_dir, file, entry, &mut report);
        }
        if stuck || waiting.is_empty() {
            return report;
        }
        todo = waiting;
    }
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
    if dst.is_file() {
        crate::diagnostics::log_warning(format!(
            "The recovery copy {} was not moved to {}: a copy already sits under the new name. \
             It was kept.",
            src.display(),
            dst.display()
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
            report.failed += 1;
            crate::diagnostics::log_warning(format!(
                "{e}: the recovery copy {} is still owed a move to {}; retried at next start.",
                src.display(),
                dst.display()
            ));
        }
    }
}
