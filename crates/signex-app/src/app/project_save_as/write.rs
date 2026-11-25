//! The Save Project As writes, on the blocking pool (never in `update`). A
//! newer-minor project's keys check runs first and can stop the copy before
//! anything is written. The target is checked again, every listed file is read from the original
//! folder and written into the new one with the atomic writer, and the
//! `.snxprj` goes last. A failure takes back what this call wrote. Nothing in
//! the original folder is ever written.

use std::path::{Path, PathBuf};

use signex_types::atomic_io::atomic_write;

use super::dropped_keys::dropped_from_file;
use super::plan::{CopyPlan, Listed};
use crate::app::handlers::document_files::new_project::{NewProjectLayout, resolve_new_project};

/// One file of the copy: where it is read, and its path in the new folder.
type CopyFile = (PathBuf, PathBuf);

/// What a Save of the copy came to when nothing failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CopyOutcome {
    /// The copy is on disk; its new `.snxprj`.
    Written(PathBuf),
    /// Nothing was written: the copy of a newer-minor project would leave
    /// out or change these keys of its `.snxprj`, and the person has not
    /// agreed to all of them.
    NotKept(Vec<String>),
}

/// Write the copy `plan` describes, unless its keys check names a key the
/// plan has not accepted; then nothing is written.
pub(crate) fn write_copy(plan: &CopyPlan) -> Result<CopyOutcome, String> {
    if let Some(check) = &plan.keys_check {
        let dropped = dropped_from_file(&check.source_file, &plan.project_bytes)?;
        if !dropped.iter().all(|key| check.accepted.contains(key)) {
            return Ok(CopyOutcome::NotKept(dropped));
        }
    }
    write_files(plan).map(CopyOutcome::Written)
}

/// The writes of the copy. Returns the new `.snxprj`.
fn write_files(plan: &CopyPlan) -> Result<PathBuf, String> {
    let chosen = plan.location.join(format!("{}.snxprj", plan.name));
    // Refuses a folder that exists and is not empty, a project folder, and a
    // place inside a project (the open one included).
    let target = resolve_new_project(&chosen, NewProjectLayout::NamedFolder)
        .map_err(|e| format!("{e:#}"))?;
    let folder = target.folder.clone();
    if folder.starts_with(&plan.source_dir) {
        return Err(format!(
            "{} is inside the project's own folder; pick a location outside it",
            folder.display()
        ));
    }
    let files = expand(&plan.source_dir, &plan.listed)?;
    let folder_existed = folder.exists();
    let mut written: Vec<PathBuf> = Vec::new();
    let result = write_all(
        &files,
        &folder,
        &target.project_path,
        &plan.project_bytes,
        &mut written,
    );
    if let Err(why) = result {
        roll_back(&written, &folder, folder_existed);
        return Err(why);
    }
    Ok(target.project_path)
}

fn write_all(
    files: &[CopyFile],
    folder: &Path,
    project_path: &Path,
    project_bytes: &[u8],
    written: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for (from, rel) in files {
        let bytes =
            std::fs::read(from).map_err(|e| format!("could not read {}: {e}", from.display()))?;
        let to = folder.join(rel);
        atomic_write(&to, &bytes).map_err(|e| format!("could not write {}: {e}", to.display()))?;
        written.push(to);
    }
    atomic_write(project_path, project_bytes)
        .map_err(|e| format!("could not write {}: {e}", project_path.display()))?;
    written.push(project_path.to_path_buf());
    Ok(())
}

/// Every file `listed` stands for, each once: a sheet or board as it is, a
/// library as its manifest plus every file under its library folders (or the
/// whole folder of a library kept as one). Refused when a listed file is not
/// on disk, so the copy never lists a file it does not hold.
fn expand(source: &Path, listed: &[Listed]) -> Result<Vec<CopyFile>, String> {
    let mut files: Vec<CopyFile> = Vec::new();
    for item in listed {
        let rel = item.path();
        let from = source.join(rel);
        if let Listed::Library(_) = item
            && is_real_dir(&from)
        {
            walk(&from, rel, &mut files)?;
            continue;
        }
        if !from.is_file() {
            return Err(format!(
                "{} is listed by the project but is not on disk; nothing was copied",
                from.display()
            ));
        }
        push(&mut files, from, rel.to_path_buf());
        if let Listed::Library(_) = item {
            let dir = rel.parent().unwrap_or(Path::new(""));
            for (sub, _) in crate::app::library_delete::known_folders() {
                let sub_rel = dir.join(sub);
                let sub_dir = source.join(&sub_rel);
                if is_real_dir(&sub_dir) {
                    walk(&sub_dir, &sub_rel, &mut files)?;
                }
            }
        }
    }
    Ok(files)
}

fn push(files: &mut Vec<CopyFile>, from: PathBuf, rel: PathBuf) {
    if !files.iter().any(|(_, known)| *known == rel) {
        files.push((from, rel));
    }
}

/// Every file under `dir`, as `rel` names it. A link is not followed: it may
/// lead outside the project, and is said in Messages.
fn walk(dir: &Path, rel: &Path, files: &mut Vec<CopyFile>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("could not read {}: {e}", dir.display()))?;
        let path = entry.path();
        let child = rel.join(entry.file_name());
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?
            .file_type();
        if kind.is_dir() {
            walk(&path, &child, files)?;
        } else if kind.is_file() {
            push(files, path, child);
        } else {
            crate::diagnostics::log_warning(format!(
                "Save Project As: {} is a link and was not copied",
                path.display()
            ));
        }
    }
    Ok(())
}

/// A real directory: not a link leading somewhere else.
fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// Take back a failed copy: the files this call wrote, then the folders that
/// emptied, deepest first, and the new folder itself when this call made it.
/// Best effort; a leftover is said in Messages.
fn roll_back(written: &[PathBuf], folder: &Path, folder_existed: bool) {
    for file in written {
        if let Err(e) = std::fs::remove_file(file) {
            crate::diagnostics::log_warning(format!(
                "Save Project As: could not remove {} after a failure: {e}",
                file.display()
            ));
        }
    }
    let mut dirs: Vec<&Path> = written
        .iter()
        .flat_map(|file| file.ancestors().skip(1))
        .filter(|dir| dir.starts_with(folder) && *dir != folder)
        .collect();
    dirs.sort_by(|a, b| {
        b.components()
            .count()
            .cmp(&a.components().count())
            .then_with(|| a.cmp(b))
    });
    dirs.dedup();
    for dir in dirs {
        // A folder that still holds something stays.
        if let Err(e) = std::fs::remove_dir(dir)
            && e.kind() != std::io::ErrorKind::DirectoryNotEmpty
        {
            crate::diagnostics::log_warning(format!(
                "Save Project As: could not remove {} after a failure: {e}",
                dir.display()
            ));
        }
    }
    if !folder_existed && let Err(e) = std::fs::remove_dir(folder) {
        crate::diagnostics::log_warning(format!(
            "Save Project As: could not remove {} after a failure: {e}",
            folder.display()
        ));
    }
}
