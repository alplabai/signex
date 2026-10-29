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

use super::keys::{KeyForm, all_keys, has_case_variant, key_for};
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
    finish_all_from(recovery_dir, pass_seed())
}

/// [`finish_all`] with the capped pass's start chosen by `seed`.
fn finish_all_from(recovery_dir: &Path, seed: u64) -> Finished {
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
        let start = (seed % files.len() as u64) as usize;
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

/// A per-process random seed for where a capped pass starts in the sorted
/// entry list (std's `RandomState` is randomly keyed per process, so this
/// needs no clock and no extra crate). Over several starts every entry is
/// expected to be reached; nothing about which entries fail can pin it.
fn pass_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
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

/// Finish one owed move. Only pure key arithmetic and `stat`s on files in the
/// recovery folder and the two documents: the pass runs synchronously at boot,
/// so it never probes a document's folder for its case rule. Instead every key
/// form of `old` is looked at, and a copy found under a form moves to the same
/// form of `new`; the next open adopts it under the then-current rule.
fn run_one(recovery_dir: &Path, file: &Path, entry: &Entry, report: &mut Finished) {
    let drop_entry = || {
        if let Err(e) = std::fs::remove_file(file)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            crate::diagnostics::log_warning(format!("{e}: could not clear {}", file.display()));
        }
    };
    let mut found: Vec<(KeyForm, PathBuf)> = all_keys(recovery_dir, &entry.old, false)
        .into_iter()
        .filter(|(form, key)| {
            // A form whose key text is the new document's own is not "old".
            key_for(recovery_dir, &entry.new, *form) != *key && key.is_file()
        })
        .collect();
    // A copy under a case-merging key may belong to another file whose name
    // differs only in case: it is not carried while such a file exists. It
    // stays where it is, and its own file is offered it when it opens.
    if found.iter().any(|(form, _)| form.merges_spellings()) && has_case_variant(&entry.old) {
        found.retain(|(form, key)| {
            let keep = !form.merges_spellings();
            if !keep {
                crate::diagnostics::log_warning(format!(
                    "The recovery copy {} may belong to a file that differs from {} only in \
                     case; it was not moved.",
                    key.display(),
                    entry.old.display()
                ));
            }
            keep
        });
    }
    if found.is_empty() {
        // No copy left under any key of the old document: already moved, or
        // never existed.
        drop_entry();
        return;
    }
    // The entry may be stale: the folder came back, or moved on again. Only
    // a document that now lives at `new` (and no longer at `old`) owns the
    // move; otherwise the copy stays at the old key, where it is offered.
    if !document_moved(&entry.old, &entry.new) {
        crate::diagnostics::log_warning(format!(
            "The pending move of the recovery copy of {} was dropped: {} is not where the \
             document moved to. The copy stays under its old name.",
            entry.old.display(),
            entry.new.display()
        ));
        drop_entry();
        return;
    }
    let mut owed_still = false;
    for (form, src) in found {
        let dst = key_for(recovery_dir, &entry.new, form);
        match super::sidecar::move_no_replace(&src, &dst) {
            Ok(()) => report.moved += 1,
            // The new path already holds a copy: keep the newest under it and
            // set the other aside. The entry stays if that fails.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dst.is_file() => {
                match super::sidecar::resolve_taken(&dst, &src, &entry.new) {
                    Ok(()) => report.moved += 1,
                    Err(why) => {
                        owed_still = true;
                        crate::diagnostics::log_warning(format!(
                            "{why}: the recovery copy {} is still owed a move to {}; retried.",
                            src.display(),
                            dst.display()
                        ));
                    }
                }
            }
            Err(e) => {
                owed_still = true;
                crate::diagnostics::log_warning(format!(
                    "{e}: the recovery copy {} is still owed a move to {}; retried.",
                    src.display(),
                    dst.display()
                ));
            }
        }
    }
    if owed_still {
        report.failed.push((entry.old.clone(), entry.new.clone()));
    } else {
        drop_entry();
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

    #[test]
    fn a_capped_pass_starts_where_its_seed_says_so_no_entry_is_pinned_out() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let total = MAX_ENTRIES_PER_PASS + 3;
        let journal = |rec: &Path| {
            for i in 0..total {
                let new = tmp.path().join(format!("new/{i}.snxsch"));
                std::fs::create_dir_all(new.parent().unwrap()).unwrap();
                std::fs::write(&new, b"saved").unwrap();
                let old = tmp.path().join(format!("old/{i}.snxsch"));
                super::super::sidecar::write(rec, &old, b"x").unwrap();
                record(rec, &old, &new).unwrap();
            }
        };
        let left_after = |seed: u64| {
            let _ = std::fs::remove_dir_all(&rec);
            journal(&rec);
            assert_eq!(finish_all_from(&rec, seed).moved, MAX_ENTRIES_PER_PASS);
            let mut left: Vec<_> = owed(&rec).into_iter().map(|(old, _)| old).collect();
            left.sort();
            left
        };
        // Two seeds leave different entries behind: the pass does not always
        // skip the same tail, so entries past the cap are reached in time.
        assert_ne!(left_after(0), left_after(3));
    }

    #[test]
    fn a_replayed_move_never_takes_the_copy_of_a_case_variant_file() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        // B is the lower-case twin of A's WHOLE path: A's folded key is B's key.
        let a = tmp.path().join("Dir/Sheet.snxsch");
        let b = PathBuf::from(a.to_string_lossy().to_lowercase());
        let main = tmp.path().join("Dir/Main.snxsch");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&b, b"b").unwrap();
        std::fs::write(&a, b"a").unwrap();
        if std::fs::read(&b).unwrap() != b"b" {
            return; // a case-insensitive volume cannot hold both
        }
        super::super::sidecar::write(&rec, &b, b"b's edits").unwrap();
        let copy = super::super::sidecar::sidecar_path(&rec, &b);
        // A is renamed to Main and the move of "its" copy is owed.
        std::fs::rename(&a, &main).unwrap();
        record(&rec, &a, &main).unwrap();
        finish_all(&rec);
        assert_eq!(std::fs::read(&copy).unwrap(), b"b's edits");
        assert!(!super::super::sidecar::sidecar_path(&rec, &main).exists());
    }
}
