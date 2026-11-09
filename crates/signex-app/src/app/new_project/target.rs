//! Does the project the dialog describes fit on disk? Reads the disk, so it
//! runs on the blocking pool (never in `update`) and never creates anything.

use std::path::{Path, PathBuf};

use crate::app::handlers::document_files::new_project::{NewProjectLayout, resolve_new_project};

use super::state::{check_name, project_file_in};

/// What the disk check found.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetStatus {
    /// Not asked yet, or the name or location is incomplete.
    Unchecked,
    Clear,
    Problem(TargetProblem),
}

/// Why Create cannot write where the dialog points.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetProblem {
    LocationEmpty,
    LocationMissing,
    /// The `.snxprj` (or its companion sheet) is already there.
    Exists(PathBuf),
    /// The project folder is there and holds something.
    FolderNotEmpty(PathBuf),
    /// Another refusal, such as a location inside a project; the text is the
    /// reason as the writer words it.
    Other(String),
}

/// Whether the folder holds nothing but OS clutter.
fn is_unused(dir: &Path) -> std::io::Result<bool> {
    const CLUTTER: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name();
        if !CLUTTER.iter().any(|c| name.as_os_str() == *c) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Check where Create would write. `location` is the text of the field.
pub(crate) fn check_target(name: &str, location: &str, make_folder: bool) -> TargetStatus {
    let Ok(name) = check_name(name) else {
        return TargetStatus::Unchecked;
    };
    let location = location.trim();
    if location.is_empty() {
        return TargetStatus::Problem(TargetProblem::LocationEmpty);
    }
    let location = Path::new(location);
    if !location.is_dir() {
        return TargetStatus::Problem(TargetProblem::LocationMissing);
    }
    let file = project_file_in(location, name, make_folder);
    if file.exists() {
        return TargetStatus::Problem(TargetProblem::Exists(file));
    }
    if make_folder {
        let folder = location.join(name);
        if folder.exists() {
            let empty = folder.is_dir() && is_unused(&folder).unwrap_or(false);
            if !empty {
                return TargetStatus::Problem(TargetProblem::FolderNotEmpty(folder));
            }
        }
    } else {
        let sheet = location.join(format!("{name}.snxsch"));
        if sheet.exists() {
            return TargetStatus::Problem(TargetProblem::Exists(sheet));
        }
    }
    let layout = if make_folder {
        NewProjectLayout::NamedFolder
    } else {
        NewProjectLayout::InPlace
    };
    match resolve_new_project(&location.join(format!("{name}.snxprj")), layout) {
        Ok(_) => TargetStatus::Clear,
        Err(e) => TargetStatus::Problem(TargetProblem::Other(format!("{e:#}"))),
    }
}
