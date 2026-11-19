//! Libraries section of the Project tab (#1691): the libraries the project
//! mounts, in list order, with where each lives, its kind and whether it is
//! mounted (and read-only) now, and Move up / Move down on row hover
//! (Graphite mockup m33). A move is one step of the General history. Adding,
//! removing and renaming stay in the project tree, which owns the mount and
//! the file work; a line under the table says so.

mod view;

#[cfg(test)]
mod tests;

pub(super) use view::view;

use std::path::{Path, PathBuf};

use signex_i18n::{MessageId, ids};
use signex_types::project::{LibraryEntryKind, ListMove};

use crate::app::Signex;
use crate::app::state::{LoadedProject, ProjectId};

/// Whether a project's library is mounted now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryMount {
    Mounted,
    ReadOnly,
    /// The last probe found no file at its path.
    Missing,
    NotMounted,
}

impl LibraryMount {
    /// The token of the `project_settings` answer.
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Mounted => "mounted",
            Self::ReadOnly => "read_only",
            Self::Missing => "missing",
            Self::NotMounted => "not_mounted",
        }
    }

    fn words(self) -> MessageId {
        match self {
            Self::Mounted => ids::PROJECT_LIBRARIES_STATE_MOUNTED,
            Self::ReadOnly => ids::PROJECT_LIBRARIES_STATE_READ_ONLY,
            Self::Missing => ids::PROJECT_LIBRARIES_STATE_MISSING,
            Self::NotMounted => ids::PROJECT_LIBRARIES_STATE_NOT_MOUNTED,
        }
    }

    /// The tone of the state pill: ok when mounted, error when missing,
    /// neutral otherwise (read-only adds a lock).
    fn tone(self) -> crate::styles::PillTone {
        use crate::styles::PillTone;
        match self {
            Self::Mounted => PillTone::Ok,
            Self::Missing => PillTone::Err,
            Self::ReadOnly | Self::NotMounted => PillTone::Neutral,
        }
    }
}

/// One row of the section: the entry as stored and what the app knows of it.
#[derive(Debug, Clone)]
pub(crate) struct LibraryRow {
    /// The path as the project stores it (relative inside the project).
    pub stored: PathBuf,
    /// The path it resolves to.
    pub resolved: PathBuf,
    pub kind: LibraryEntryKind,
    pub mount: LibraryMount,
}

impl LibraryRow {
    /// The library's file name without `.snxlib`.
    pub(crate) fn name(&self) -> String {
        self.resolved.file_stem().map_or_else(
            || self.stored.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        )
    }
}

fn kind_words(kind: LibraryEntryKind) -> MessageId {
    match kind {
        LibraryEntryKind::ProjectLocal => ids::PROJECT_LIBRARIES_KIND_PROJECT,
        LibraryEntryKind::Shared => ids::PROJECT_LIBRARIES_KIND_SHARED,
        LibraryEntryKind::Global => ids::PROJECT_LIBRARIES_KIND_GLOBAL,
    }
}

/// `path` cut from the start to at most `max_chars` characters, so the file
/// name at its end stays visible: `…` and the tail. A path that fits is
/// returned whole.
pub(super) fn path_tail(path: &str, max_chars: usize) -> String {
    let count = path.chars().count();
    if count <= max_chars {
        return path.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let tail: String = path.chars().skip(count - keep).collect();
    format!("…{tail}")
}

impl Signex {
    /// The project's libraries in list order, each with its mount state read
    /// from the Model (the mounted set and the last filesystem probe).
    pub(crate) fn project_library_rows(&self, project: &LoadedProject) -> Vec<LibraryRow> {
        let facts = &self.document_state.project_fs.facts;
        project
            .data
            .libraries
            .iter()
            .map(|entry| {
                let resolved = project.data.resolve_library_path(entry);
                let mount = self.library_mount(&resolved, facts.is_missing(&resolved));
                LibraryRow {
                    stored: entry.path.clone(),
                    resolved,
                    kind: entry.kind,
                    mount,
                }
            })
            .collect()
    }

    fn library_mount(&self, resolved: &Path, missing: bool) -> LibraryMount {
        match self.library.library_at(resolved) {
            Some(_) if self.library.is_read_only(resolved) => LibraryMount::ReadOnly,
            Some(_) => LibraryMount::Mounted,
            None if missing => LibraryMount::Missing,
            None => LibraryMount::NotMounted,
        }
    }

    /// Move up / Move down in the Libraries table: one step of its own.
    pub(super) fn move_project_library(
        &mut self,
        project_id: ProjectId,
        path: PathBuf,
        step: ListMove,
    ) {
        let command = signex_engine::ProjectSectionCommand::MoveLibrary { path, step };
        // A move never refuses; at the end of the list it changes nothing.
        if let Err(problem) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }
}
