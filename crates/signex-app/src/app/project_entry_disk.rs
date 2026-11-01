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
/// parsed first, so a corrupt project is reported and never overwritten.
pub(crate) fn retarget_sheet_entry(
    project_file: &Path,
    from: &Path,
    new_name: &str,
    fold: CaseFold,
) -> Result<EntryPatch, ProjectError> {
    if !project_file.exists() {
        return Ok(EntryPatch::NoProjectFile);
    }
    let mut data = parse_project(project_file)?;
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
    let mut data = parse_project(project_file)?;
    if !register_entry(&mut data, entry, fold) {
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
    signex_types::atomic_io::atomic_write(project_file, &bytes).map_err(io)?;
    Ok(EntryPatch::Patched(std::sync::Arc::new(bytes)))
}
