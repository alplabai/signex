//! Patching one sheet entry of the `.snxprj` on disk (#1821).
//!
//! A sheet rename or an Add New Schematic changes a file on disk at once, so
//! the project entry that names it has to be committed with it: otherwise
//! Discard All, a quit without saving or a crash leaves the saved project
//! naming a file that is gone (or not naming one that exists). The job reads
//! the saved project, changes only that entry and writes it back atomically,
//! so other unsaved project edits held in memory are never written.

use std::path::Path;

use signex_types::project::{ProjectData, ProjectError, encode_project, parse_project};

use crate::app::state::library_entry::{entry_resolves_to, with_new_leaf};
use crate::app::state::scope::CaseFold;

/// What a patch did to the project file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryPatch {
    /// The saved project was changed and rewritten atomically; holds the
    /// exact bytes written, so the follow-up git commit records them.
    Patched(crate::persist::SharedBytes),
    /// There is no `.snxprj` on disk yet; nothing was written.
    NoProjectFile,
    /// The saved project has no entry to change; nothing was written.
    NotListed,
    /// The `.snxprj` on disk cannot be read (damaged or edited outside
    /// Signex); it is left untouched and the reason is kept for the user. The
    /// change then waits for a save of the in-memory project, as before.
    Unreadable(String),
}

/// Point every sheet, PCB and schematic-root entry that resolves to `from`
/// at `new_name`, keeping the directory prefix (`sub/Power.snxsch` becomes
/// `sub/Power2.snxsch`, #1367). Returns whether anything changed.
pub(crate) fn repoint_entries(
    data: &mut ProjectData,
    dir: &Path,
    from: &Path,
    new_name: &str,
    fold: CaseFold,
) -> bool {
    let renamed_from = |entry: &str| {
        entry_resolves_to(dir, entry, from, fold).then(|| with_new_leaf(entry, new_name))
    };
    let mut changed = false;
    for entry in data.sheets.iter_mut() {
        if let Some(new_entry) = renamed_from(&entry.filename) {
            entry.filename = new_entry;
            changed = true;
        }
    }
    if let Some(new_entry) = data.pcb_file.as_deref().and_then(renamed_from) {
        data.pcb_file = Some(new_entry);
        changed = true;
    }
    if let Some(new_entry) = data.schematic_root.as_deref().and_then(renamed_from) {
        data.schematic_root = Some(new_entry);
        changed = true;
    }
    changed
}

/// Re-point the saved project's entries for `from` at `new_name`. The file is
/// parsed first, so an unreadable project is reported (`Unreadable`) and never
/// overwritten; only a failed write is an error.
pub(crate) fn retarget_sheet_entry(
    project_file: &Path,
    from: &Path,
    new_name: &str,
    fold: CaseFold,
) -> Result<EntryPatch, ProjectError> {
    if !project_file.exists() {
        return Ok(EntryPatch::NoProjectFile);
    }
    let mut data = match parse_project(project_file) {
        Ok(data) => data,
        Err(e) => return Ok(EntryPatch::Unreadable(e.to_string())),
    };
    let dir = project_file.parent().unwrap_or_else(|| Path::new(""));
    if !repoint_entries(&mut data, dir, from, new_name, fold) {
        return Ok(EntryPatch::NotListed);
    }
    write_patched(project_file, &data)
}

/// List the sheet `entry` (relative to the project folder) in the saved
/// project, leaving everything else as the user last saved it. A sheet the
/// saved project already lists is not listed twice (`NotListed`).
pub(crate) fn add_sheet_entry(
    project_file: &Path,
    entry: &str,
    fold: CaseFold,
) -> Result<EntryPatch, ProjectError> {
    if !project_file.exists() {
        return Ok(EntryPatch::NoProjectFile);
    }
    let mut data = match parse_project(project_file) {
        Ok(data) => data,
        Err(e) => return Ok(EntryPatch::Unreadable(e.to_string())),
    };
    if !register_entry(&mut data, entry, fold) {
        return Ok(EntryPatch::NotListed);
    }
    write_patched(project_file, &data)
}

/// Drop every sheet, PCB, schematic-root and library entry that resolves to
/// `target`. Shared by the in-memory project and the saved one. Returns
/// whether anything changed.
pub(crate) fn remove_entries(
    data: &mut ProjectData,
    dir: &Path,
    target: &Path,
    fold: CaseFold,
) -> bool {
    let is_target = |entry: &str| entry_resolves_to(dir, entry, target, fold);
    let mut changed = false;
    let before = data.sheets.len();
    data.sheets.retain(|entry| !is_target(&entry.filename));
    changed |= data.sheets.len() != before;
    if data.schematic_root.as_deref().is_some_and(is_target) {
        data.schematic_root = None;
        changed = true;
    }
    if data.pcb_file.as_deref().is_some_and(is_target) {
        data.pcb_file = None;
        changed = true;
    }
    // By resolved path, not file name: two libraries may share a name and only
    // the targeted one goes (#742).
    let keep: Vec<bool> = data
        .libraries
        .iter()
        .map(|entry| data.resolve_library_path(entry) != target)
        .collect();
    let mut keep = keep.into_iter();
    let libraries = data.libraries.len();
    data.libraries.retain(|_| keep.next().unwrap_or(true));
    changed |= data.libraries.len() != libraries;
    changed
}

/// Remove the entries for `target` from the saved project, leaving everything
/// else as the user last saved it (#1849). Unreadable projects are reported
/// and never overwritten.
pub(crate) fn remove_entry(
    project_file: &Path,
    target: &Path,
    fold: CaseFold,
) -> Result<EntryPatch, ProjectError> {
    if !project_file.exists() {
        return Ok(EntryPatch::NoProjectFile);
    }
    let mut data = match parse_project(project_file) {
        Ok(data) => data,
        Err(e) => return Ok(EntryPatch::Unreadable(e.to_string())),
    };
    let dir = project_file.parent().unwrap_or_else(|| Path::new(""));
    if !remove_entries(&mut data, dir, target, fold) {
        return Ok(EntryPatch::NotListed);
    }
    write_patched(project_file, &data)
}

/// The one place a sheet row is built, shared by the in-memory project and
/// the saved one. `true` when a row was added.
pub(crate) fn register_entry(data: &mut ProjectData, filename: &str, fold: CaseFold) -> bool {
    let key = |name: &str| crate::app::state::scope::path_key_with(Path::new(name), fold);
    let wanted = key(filename);
    if data.sheets.iter().any(|s| key(&s.filename) == wanted) {
        return false;
    }
    let name = Path::new(filename)
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    data.sheets.push(signex_types::project::SheetEntry {
        name,
        filename: filename.to_owned(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    if data.schematic_root.is_none() {
        data.schematic_root = Some(filename.to_owned());
    }
    true
}

/// Encode `data` and write exactly those bytes atomically.
fn write_patched(project_file: &Path, data: &ProjectData) -> Result<EntryPatch, ProjectError> {
    let io = |source: std::io::Error| ProjectError::Io {
        path: project_file.display().to_string(),
        source,
    };
    let bytes = encode_project(data).map_err(|e| io(std::io::Error::other(e.to_string())))?;
    let written = signex_types::atomic_io::atomic_write_existing_dir(project_file, &bytes);
    settle_write(project_file, &bytes, written).map_err(io)?;
    Ok(EntryPatch::Patched(std::sync::Arc::new(bytes)))
}

/// `atomic_write` can fail after its rename already replaced the file (the
/// parent-directory fsync). Rolling the sheet back then would leave the saved
/// project naming a file that is not there, so a write error is judged by what
/// is on disk: if the file already holds exactly `bytes`, the write counts as
/// committed and the failure is only warned about.
fn settle_write(
    project_file: &Path,
    bytes: &[u8],
    written: std::io::Result<()>,
) -> std::io::Result<()> {
    let Err(error) = written else {
        return Ok(());
    };
    if std::fs::read(project_file).is_ok_and(|on_disk| on_disk == bytes) {
        crate::diagnostics::log_warning(format!(
            "{} was updated, but flushing it to disk reported an error ({error}); the change is kept.",
            project_file
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        ));
        return Ok(());
    }
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_error_after_the_rename_landed_counts_as_committed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("P.snxprj");
        std::fs::write(&file, b"patched").expect("write");
        let fsync_failed = Err(std::io::Error::other("fsync of parent failed"));
        assert!(settle_write(&file, b"patched", fsync_failed).is_ok());
    }

    #[test]
    fn a_write_error_with_the_old_bytes_on_disk_stays_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("P.snxprj");
        std::fs::write(&file, b"old").expect("write");
        let failed = Err(std::io::Error::other("denied"));
        assert!(settle_write(&file, b"patched", failed).is_err());
        let missing = dir.path().join("Gone.snxprj");
        assert!(settle_write(&missing, b"patched", Err(std::io::Error::other("x"))).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn patching_a_symlinked_project_file_writes_through_and_keeps_the_link() {
        let dir = tempfile::tempdir().expect("tempdir");
        let real = dir.path().join("Real.snxprj");
        std::fs::write(&real, b"").expect("marker");
        let mut data = parse_project(&real).expect("parse");
        signex_types::project::write_project(&real, &data).expect("write");
        let link = dir.path().join("Link.snxprj");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        register_entry(&mut data, "a.snxsch", CaseFold::default());

        write_patched(&link, &data).expect("patch");

        assert!(
            std::fs::symlink_metadata(&link)
                .expect("meta")
                .file_type()
                .is_symlink(),
            "the link must survive"
        );
        let text = std::fs::read_to_string(&real).expect("read");
        assert!(text.contains("a.snxsch"), "the target holds the entry");
    }
}
