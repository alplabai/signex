//! Libraries section of the Project tab (#1691): the libraries the project
//! mounts, in list order, with where each lives, its kind and whether it is
//! mounted (and read-only) now, and Move up / Move down. A move is one step of
//! the General history. Adding, removing and renaming stay in the project
//! tree, which owns the mount and the file work.

use std::path::{Path, PathBuf};

use iced::{Element, Length};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::project::{LibraryEntryKind, ListMove};
use signex_types::theme::ThemeTokens;

use super::ProjectTabMsg;
use super::general::tab_message;
use super::sheets::ACTIONS_EMS;
use super::table::{TS, cell, heading_row, move_buttons, note, section, table_row, word_cell};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, Signex};

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

pub(super) fn view<'a>(
    project_id: ProjectId,
    rows: &[LibraryRow],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let columns = [
        (ids::PROJECT_LIBRARIES_COL_NAME, Length::FillPortion(2)),
        (ids::PROJECT_LIBRARIES_COL_LOCATION, Length::FillPortion(3)),
        (ids::PROJECT_LIBRARIES_COL_KIND, Length::FillPortion(1)),
        (ids::PROJECT_LIBRARIES_COL_STATE, Length::FillPortion(1)),
    ];
    let actions = Length::Fixed(TS.body * ACTIONS_EMS);
    let mut body: Vec<Element<'a, Message>> =
        vec![heading_row(&columns, Some(actions), i18n, tokens)];
    if rows.is_empty() {
        body.push(note(t!(i18n, ids::PROJECT_LIBRARIES_EMPTY), tokens));
    }
    for (index, row) in rows.iter().enumerate() {
        let path = row.stored.clone();
        let moved = move |step: ListMove| {
            tab_message(ProjectTabMsg::LibraryMoved {
                project_id,
                path: path.clone(),
                step,
            })
        };
        body.push(table_row(vec![
            (cell(row.name(), tokens), columns[0].1),
            (cell(row.stored.display().to_string(), tokens), columns[1].1),
            (
                word_cell(t!(i18n, kind_words(row.kind)), tokens),
                columns[2].1,
            ),
            (word_cell(t!(i18n, row.mount.words()), tokens), columns[3].1),
            (
                move_buttons(index, rows.len(), moved, i18n, tokens),
                actions,
            ),
        ]));
    }
    section(
        ids::PROJECT_SECTION_LIBRARIES,
        ids::PROJECT_LIBRARIES_HELP,
        body,
        i18n,
        tokens,
    )
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
