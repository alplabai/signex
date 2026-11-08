//! Every document, with its metadata: the answer behind `list_documents`,
//! `document_info`, `find` and the `Document` operand.
//!
//! A document is listed when it is held in memory (a sheet engine, a board,
//! a symbol or footprint editor, a component row, a project) and when an open
//! project references its file without any of that. The second kind has no
//! engine and no tab; its id was read from the file head in the background
//! (`headers`). Only [`DocumentState`](crate::app::state::DocumentState) and
//! the library are read; no window, tab order, focus or selection.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_types::document_id::DocumentId;

use crate::app::document::{Document as _, DocumentKey};
use crate::app::state::LoadedProject;
use crate::app::state::scope::path_key_with;
use crate::app::{Signex, TabDocument, TabKind};

/// What a document is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentKind {
    Schematic,
    Pcb,
    Symbol,
    Footprint,
    Project,
    Component,
}

impl DocumentKind {
    pub(crate) const ALL: [Self; 6] = [
        Self::Schematic,
        Self::Pcb,
        Self::Symbol,
        Self::Footprint,
        Self::Project,
        Self::Component,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Schematic => "schematic",
            Self::Pcb => "pcb",
            Self::Symbol => "symbol",
            Self::Footprint => "footprint",
            Self::Project => "project",
            Self::Component => "component",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }
}

/// The project a document belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectRef {
    pub id: Option<DocumentId>,
    pub path: PathBuf,
    pub name: String,
}

/// One listed document.
#[derive(Debug, Clone)]
pub(crate) struct DocumentEntry {
    /// `None` for a referenced file whose head holds no id (or could not be
    /// read yet).
    pub id: Option<DocumentId>,
    pub kind: DocumentKind,
    /// The key it is held under; `None` while it is only referenced.
    pub key: Option<DocumentKey>,
    /// `None` for a document with no file (a component row).
    pub path: Option<PathBuf>,
    pub name: String,
    pub project: Option<ProjectRef>,
    pub loaded: bool,
    pub dirty: bool,
    pub read_only: bool,
    pub saving: bool,
    /// Shown in a window of its own. Informational: never a default target.
    pub open_in_window: bool,
}

impl DocumentEntry {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "id": self.id.map(|id| id.to_string()),
            "kind": self.kind.as_str(),
            "path": self.path.as_ref().map(|p| p.display().to_string()),
            "name": self.name,
            "project": self.project.as_ref().map(|p| json!({
                "id": p.id.map(|id| id.to_string()),
                "path": p.path.display().to_string(),
                "name": p.name,
            })),
            "loaded": self.loaded,
            "dirty": self.dirty,
            "read_only": self.read_only,
            "saving": self.saving,
            "open_in_window": self.open_in_window,
        })
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

impl Signex {
    fn project_ref_of(&self, path: &Path) -> Option<ProjectRef> {
        self.document_state.document_project(path).map(project_ref)
    }

    fn entry_of_held(
        &self,
        key: DocumentKey,
        id: Option<DocumentId>,
        kind: DocumentKind,
        path: Option<PathBuf>,
        name: String,
        read_only: bool,
    ) -> DocumentEntry {
        let docs = &self.document_state;
        let saving = path
            .as_deref()
            .is_some_and(|p| docs.document_saves.is_saving(p));
        let open_in_window = path.as_deref().is_some_and(|p| {
            self.ui_state.windows.values().any(|kind| {
                matches!(kind, crate::app::state::WindowKind::UndockedTab { path: shown, .. } if shown == p)
            })
        });
        let loaded = !path
            .as_deref()
            .is_some_and(|p| docs.document_ids.is_transient(p));
        let project = match (&key, path.as_deref()) {
            (DocumentKey::Project(_), _) | (_, None) => None,
            (_, Some(path)) => self.project_ref_of(path),
        };
        DocumentEntry {
            id,
            kind,
            dirty: self.document(&key).is_some_and(|doc| doc.is_dirty()),
            key: Some(key),
            path,
            name,
            project,
            loaded,
            read_only,
            saving,
            open_in_window,
        }
    }

    /// Every document: the ones held in memory, then the files open projects
    /// reference that are not. Ordered by kind, then path, so the answer
    /// does not depend on a map's iteration order.
    pub(crate) fn document_entries(&self) -> Vec<DocumentEntry> {
        let docs = &self.document_state;
        let mut entries = Vec::new();
        for (path, engine) in &docs.engines {
            let sheet = engine.document();
            entries.push(self.entry_of_held(
                DocumentKey::Schematic(path.clone()),
                DocumentId::from_uuid(sheet.uuid),
                DocumentKind::Schematic,
                Some(path.clone()),
                file_stem(path),
                sheet.is_read_only(),
            ));
        }
        for tab in &docs.tabs {
            if let (TabKind::Pcb, Some(TabDocument::Pcb(board))) = (&tab.kind, &tab.cached_document)
            {
                entries.push(self.entry_of_held(
                    DocumentKey::Pcb(tab.path.clone()),
                    DocumentId::from_uuid(board.uuid),
                    DocumentKind::Pcb,
                    Some(tab.path.clone()),
                    tab.title.clone(),
                    board.unknown.newer_minor_source.is_some(),
                ));
            }
        }
        for (path, editor) in &docs.symbol_editors {
            entries.push(self.entry_of_held(
                DocumentKey::Symbol(path.clone()),
                DocumentId::from_uuid(editor.file.file_uuid),
                DocumentKind::Symbol,
                Some(path.clone()),
                file_stem(path),
                false,
            ));
        }
        for (path, editor) in &docs.footprint_editors {
            entries.push(self.entry_of_held(
                DocumentKey::Footprint(path.clone()),
                DocumentId::from_uuid(editor.file.file_uuid),
                DocumentKind::Footprint,
                Some(path.clone()),
                file_stem(path),
                false,
            ));
        }
        for project in &docs.projects {
            entries.push(self.entry_of_held(
                DocumentKey::Project(project.id),
                project.data.document_id,
                DocumentKind::Project,
                Some(project.path.clone()),
                project.data.name.clone(),
                false,
            ));
        }
        for address in self.library.editors.keys() {
            let name = docs
                .tabs
                .iter()
                .find(|tab| {
                    DocumentKey::of(tab)
                        .is_some_and(|key| matches!(key, DocumentKey::Row(a) if a == *address))
                })
                .map_or_else(|| address.table.clone(), |tab| tab.title.clone());
            entries.push(self.entry_of_held(
                DocumentKey::Row(address.clone()),
                DocumentId::from_uuid(address.row_id.0),
                DocumentKind::Component,
                None,
                name,
                false,
            ));
        }
        self.push_referenced_files(&mut entries);
        entries.sort_by(|a, b| {
            (a.kind as u8, a.path.as_deref(), a.name.as_str()).cmp(&(
                b.kind as u8,
                b.path.as_deref(),
                b.name.as_str(),
            ))
        });
        entries
    }

    /// The files an open project references that no loaded document covers,
    /// as entries with the id read from their head.
    fn push_referenced_files(&self, entries: &mut Vec<DocumentEntry>) {
        let docs = &self.document_state;
        let mut taken: HashSet<DocumentId> = entries.iter().filter_map(|e| e.id).collect();
        for project in &docs.projects {
            let known: HashSet<String> = entries
                .iter()
                .filter_map(|e| e.path.as_deref())
                .map(|p| path_key_with(p, project.case_fold))
                .collect();
            for (path, kind) in referenced_files(project) {
                if known.contains(&path_key_with(&path, project.case_fold)) {
                    continue;
                }
                // A head id another document already holds is not claimed
                // twice: it is listed without one until a copy is renamed.
                let id = docs
                    .document_ids
                    .header(&path)
                    .flatten()
                    .filter(|id| taken.insert(*id));
                entries.push(DocumentEntry {
                    id,
                    kind,
                    key: None,
                    name: file_stem(&path),
                    project: Some(project_ref(project)),
                    loaded: false,
                    dirty: false,
                    read_only: false,
                    saving: docs.document_saves.is_saving(&path),
                    open_in_window: false,
                    path: Some(path),
                });
            }
        }
    }
}

fn project_ref(project: &LoadedProject) -> ProjectRef {
    ProjectRef {
        id: project.data.document_id,
        path: project.path.clone(),
        name: project.data.name.clone(),
    }
}

/// The files `project` names: its root sheet, its sheet list and its board,
/// resolved against the project folder.
pub(crate) fn referenced_files(project: &LoadedProject) -> Vec<(PathBuf, DocumentKind)> {
    let dir = project.dir();
    let data = &project.data;
    let sheets = data
        .schematic_root
        .iter()
        .chain(data.sheets.iter().map(|sheet| &sheet.filename));
    let mut files: Vec<(PathBuf, DocumentKind)> = Vec::new();
    for name in sheets {
        let path = dir.join(name);
        if !files.iter().any(|(held, _)| *held == path) {
            files.push((path, DocumentKind::Schematic));
        }
    }
    if let Some(pcb) = &data.pcb_file {
        files.push((dir.join(pcb), DocumentKind::Pcb));
    }
    files
}
