//! The disk side of Rename Project (#1221): move the `.snxprj` and, when the
//! project folder is named after the project, the folder with it.
//!
//! Blocking and free of app state, so it runs on `spawn_blocking` and is
//! tested against a real directory.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::app::ProjectRenameJob;
use crate::app::state::CaseFold;
use crate::app::state::scope::path_key_with;

/// The new folder for a project whose folder is named after it: the old
/// folder's name equals `old_stem` (ASCII-case-insensitively), so it follows
/// the project to `new_stem`. `None` for a folder with another name (or a
/// root with no parent): only the file is renamed then.
pub(super) fn folder_target(
    old_dir: &Path,
    old_stem: &str,
    new_stem: &str,
) -> Option<std::path::PathBuf> {
    let name = old_dir.file_name()?.to_str()?;
    if !name.eq_ignore_ascii_case(old_stem) {
        return None;
    }
    Some(old_dir.parent()?.join(new_stem))
}

/// Whether `a` and `b` are one path on disk (a case-only rename on a
/// case-insensitive volume names the same file or folder twice).
pub(super) fn same_path_on_disk(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Whether moving `old_dir` to `new_dir` would land on another folder.
pub(super) fn folder_collides(old_dir: &Path, new_dir: &Path) -> bool {
    new_dir.exists() && !same_path_on_disk(old_dir, new_dir)
}

/// After the folder moved from `old_dir` to `new_dir`: move the recovery
/// copies of documents under it that are not open (`open`, as spelled under
/// `old_dir`) to the keys of their new paths. A copy is keyed by its
/// document's full path, so without this a crash copy of a closed sheet
/// would stay under the old key and never be offered again. The copies of
/// open documents move through the re-key. A copy that cannot move stays
/// where it is, is logged, and is journalled so the next start finishes the
/// move; one whose new key is taken is left alone.
///
/// The copy is looked for under every spelling a writer may have keyed it by
/// ([`copy_sources`]).
pub(crate) fn carry_closed_copies(
    recovery: &Path,
    old_dir: &Path,
    new_dir: &Path,
    open: &HashSet<PathBuf>,
    fold: CaseFold,
) {
    use crate::autosave::sidecar::{rename, sidecar_path};
    let owed = crate::autosave::pending_move::owed(recovery);
    for (old, new) in closed_documents(new_dir, old_dir, new_dir, open) {
        let dst = sidecar_path(recovery, &new);
        let Some(spelled) = find_copy(recovery, &old, fold, &owed) else {
            continue;
        };
        let src = sidecar_path(recovery, &spelled);
        let failure = if dst.exists() {
            Some("a copy already sits under the new name".to_string())
        } else {
            rename(recovery, &spelled, &new).err()
        };
        if let Some(why) = failure {
            crate::diagnostics::log_warning(format!(
                "The recovery copy of {} was not carried to {}: {why}. It was kept as {}.",
                old.display(),
                new.display(),
                src.display()
            ));
        }
    }
}

/// Before the folder moves: journal the move of every closed document's copy
/// to where the document is about to be, so a crash or a failing move cannot
/// strand it. The startup pass applies an entry only once the document is
/// really at the new path. Returns the spellings journalled, for
/// [`unjournal_closed_copies`] when the folder move fails.
pub(crate) fn journal_closed_copies(
    recovery: &Path,
    old_dir: &Path,
    new_dir: &Path,
    open: &HashSet<PathBuf>,
    fold: CaseFold,
) -> Vec<PathBuf> {
    let mut journalled = Vec::new();
    let owed = crate::autosave::pending_move::owed(recovery);
    for (old, new) in closed_documents(old_dir, old_dir, new_dir, open) {
        let Some(spelled) = find_copy(recovery, &old, fold, &owed) else {
            continue;
        };
        match crate::autosave::pending_move::record(recovery, &spelled, &new) {
            Ok(()) => journalled.push(spelled),
            Err(why) => crate::diagnostics::log_warning(why),
        }
    }
    journalled
}

/// The folder move failed: the copies stay where they are, no move is owed.
pub(crate) fn unjournal_closed_copies(recovery: &Path, journalled: &[PathBuf]) {
    for old in journalled {
        crate::autosave::pending_move::clear(recovery, old);
    }
}

/// The spelling of `old` its recovery copy is keyed by, after adopting a copy
/// from a build before #1510 that sits under the raw key. A copy whose move
/// to `old` is still owed (`owed`, from the journal) is found at the key it
/// still sits under, so a second rename carries it on instead of stranding it.
fn find_copy(
    recovery: &Path,
    old: &Path,
    fold: CaseFold,
    owed: &[(PathBuf, PathBuf)],
) -> Option<PathBuf> {
    use crate::autosave::sidecar::{adopt_legacy, sidecar_path};
    let direct = copy_sources(old, fold).into_iter().find(|s| {
        if let Err(why) = adopt_legacy(recovery, s) {
            crate::diagnostics::log_warning(why);
        }
        sidecar_path(recovery, s).is_file()
    });
    direct.or_else(|| {
        let target = sidecar_path(recovery, old);
        owed.iter()
            .find(|(source, next)| {
                sidecar_path(recovery, next) == target && sidecar_path(recovery, source).is_file()
            })
            .map(|(source, _)| source.clone())
    })
}

/// The autosaved documents found by walking `root` (the old folder before the
/// move, the new one after), as (path under `old_dir`, path under `new_dir`),
/// leaving out the open ones.
fn closed_documents(
    root: &Path,
    old_dir: &Path,
    new_dir: &Path,
    open: &HashSet<PathBuf>,
) -> Vec<(PathBuf, PathBuf)> {
    let mut found = Vec::new();
    let mut dirs = vec![PathBuf::new()];
    while let Some(rel_dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(root.join(&rel_dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let rel = rel_dir.join(entry.file_name());
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    if entry.file_name() != ".git" {
                        dirs.push(rel);
                    }
                    continue;
                }
                Ok(_) => {}
                Err(_) => continue,
            }
            let old = old_dir.join(&rel);
            if is_autosaved_document(&rel) && !open.contains(&old) {
                found.push((old, new_dir.join(&rel)));
            }
        }
    }
    found
}

/// The spellings of `old` a recovery copy may be keyed by: as given, and the
/// volume's own key spelling (case-folded on a case-insensitive volume), the
/// way another open of the same file spells it. Dot segments are resolved by
/// the sidecar key itself, so they need no candidate.
fn copy_sources(old: &Path, fold: CaseFold) -> Vec<PathBuf> {
    let mut spellings = vec![old.to_path_buf()];
    let keyed = PathBuf::from(path_key_with(old, fold));
    if keyed != old {
        spellings.push(keyed);
    }
    spellings
}

/// The documents autosave keeps a recovery copy of.
fn is_autosaved_document(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["snxsch", "snxsym", "snxfpt"]
            .iter()
            .any(|d| e.eq_ignore_ascii_case(d))
    })
}

/// Move the project on disk: first the `.snxprj` inside its folder, then the
/// folder. If the folder cannot move (a file open elsewhere, a permission),
/// the `.snxprj` is put back, so a failure leaves the disk as it was. Both
/// targets are checked again here, right before the move.
pub(crate) fn rename_project_on_disk(job: &ProjectRenameJob) -> Result<(), String> {
    let old_file = job.old_project_file();
    let renamed_file = job.old_dir.join(format!("{}.snxprj", job.new_stem));
    if renamed_file != old_file
        && renamed_file.exists()
        && !same_path_on_disk(&old_file, &renamed_file)
    {
        return Err("A project with that name already exists.".to_string());
    }
    let moves_folder = job.old_dir != job.new_dir;
    if moves_folder && folder_collides(&job.old_dir, &job.new_dir) {
        return Err("A folder with that name already exists.".to_string());
    }
    std::fs::rename(&old_file, &renamed_file).map_err(|e| format!("Rename failed: {e}"))?;
    if !moves_folder {
        return Ok(());
    }
    if let Err(e) = std::fs::rename(&job.old_dir, &job.new_dir) {
        return Err(match std::fs::rename(&renamed_file, &old_file) {
            Ok(()) => format!("The project folder could not be renamed: {e}"),
            Err(restore) => format!(
                "The project folder could not be renamed ({e}) and {} could not be restored ({restore}); it is now {}.",
                old_file.display(),
                renamed_file.display()
            ),
        });
    }
    Ok(())
}
