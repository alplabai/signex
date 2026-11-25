//! What a Save Project As copies, planned from the open project in `update`
//! without touching the disk.

use std::path::{Component, Path, PathBuf};

use signex_types::document_id::DocumentId;
use signex_types::project::{LibraryEntryKind, ProjectData, encode_project};

/// One item the project lists in its own folder, relative to that folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Listed {
    /// A sheet or the board.
    File(PathBuf),
    /// A project library: its manifest and the library folders beside it.
    Library(PathBuf),
}

impl Listed {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::File(path) | Self::Library(path) => path,
        }
    }
}

/// The copy of one project: what is read from `source_dir`, and the new
/// `.snxprj` written as `<location>/<name>/<name>.snxprj`.
#[derive(Debug, Clone)]
pub(crate) struct CopyPlan {
    /// The open project's folder: only ever read.
    pub source_dir: PathBuf,
    pub listed: Vec<Listed>,
    pub name: String,
    pub location: PathBuf,
    /// The new `.snxprj`, encoded in this build's format.
    pub project_bytes: Vec<u8>,
    /// The listed documents with unsaved edits, encoded from memory: the copy
    /// writes these bytes in place of the file on disk.
    pub unsaved: Vec<super::unsaved::UnsavedCopy>,
    /// Set for a project read from a newer minor: its `.snxprj` is checked
    /// for the keys this build would not keep before anything is written
    /// (`dropped_keys`).
    pub keys_check: Option<KeysCheck>,
}

/// The check a newer-minor copy waits for: the source `.snxprj`, and the
/// keys the person agreed the copy leaves out or changes. A key not agreed to
/// stops the copy before anything is written.
#[derive(Debug, Clone)]
pub(crate) struct KeysCheck {
    pub source_file: PathBuf,
    pub accepted: Vec<String>,
}

/// `text` as a path inside the project folder, or `None` when it would land
/// outside it (absolute, or climbing out with `..`).
fn inside(text: &str) -> Option<PathBuf> {
    let path = Path::new(text);
    let relative = !text.trim().is_empty()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    relative.then(|| path.to_path_buf())
}

fn outside(text: &str) -> String {
    format!(
        "{text} is listed by the project but lies outside its folder, so it cannot be \
         copied with it"
    )
}

/// Everything `data` lists in its own folder, each once, in list order: the
/// root sheet, the sheets, the board, then the project libraries. Shared and
/// global libraries stay where they are and are listed by the copy as they
/// are. Refused when an entry lies outside the folder: the copy would list
/// a file it does not hold.
pub(crate) fn listed(data: &ProjectData) -> Result<Vec<Listed>, String> {
    let mut files: Vec<&str> = Vec::new();
    files.extend(data.schematic_root.as_deref());
    files.extend(data.sheets.iter().map(|sheet| sheet.filename.as_str()));
    files.extend(data.pcb_file.as_deref());
    let mut out: Vec<Listed> = Vec::new();
    for file in files {
        let item = Listed::File(inside(file).ok_or_else(|| outside(file))?);
        if !out.contains(&item) {
            out.push(item);
        }
    }
    for entry in &data.libraries {
        if entry.kind != LibraryEntryKind::ProjectLocal {
            continue;
        }
        let text = entry.path.to_string_lossy();
        let item = Listed::Library(inside(&text).ok_or_else(|| outside(&text))?);
        if !out.contains(&item) {
            out.push(item);
        }
    }
    Ok(out)
}

/// The `.snxprj` of the copy: the open project's settings as they are now,
/// unsaved ones included, named `name` in `folder`, with a new id when the
/// project has one (a Save As is a new document, docs/DOCUMENT_IDS.md). A
/// project read from a newer minor is written in this build's format; the
/// keys this build does not know are kept as they were read, and what it
/// reads leniently or into an object with no bag is checked by `KeysCheck`.
pub(crate) fn copy_bytes(data: &ProjectData, name: &str, folder: &Path) -> Result<Vec<u8>, String> {
    let mut copy = data.clone();
    copy.unknown.newer_minor_source = None;
    copy.name = name.to_string();
    copy.dir = folder.to_string_lossy().into_owned();
    if copy.document_id.is_some() {
        copy.document_id = Some(DocumentId::new());
    }
    encode_project(&copy).map_err(|e| e.to_string())
}

/// Plan the copy of the project read from `source_file` (its `.snxprj`)
/// holding `data` as `name` under `location`, with the `unsaved` documents
/// encoded from memory. A newer-minor project's copy waits for its keys
/// check, with nothing accepted yet.
pub(crate) fn plan_copy(
    data: &ProjectData,
    source_file: &Path,
    name: &str,
    location: &Path,
    unsaved: Vec<super::unsaved::UnsavedCopy>,
) -> Result<CopyPlan, String> {
    let folder = location.join(name);
    let source_dir = source_file.parent().unwrap_or(Path::new("."));
    let keys_check = data
        .unknown
        .newer_minor_source
        .is_some()
        .then(|| KeysCheck {
            source_file: source_file.to_path_buf(),
            accepted: Vec::new(),
        });
    Ok(CopyPlan {
        source_dir: source_dir.to_path_buf(),
        listed: listed(data)?,
        name: name.to_string(),
        location: location.to_path_buf(),
        project_bytes: copy_bytes(data, name, &folder)?,
        unsaved,
        keys_check,
    })
}
