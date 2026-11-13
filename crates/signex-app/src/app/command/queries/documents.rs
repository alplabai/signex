//! `list_documents` and `document_info`: which documents exist and what
//! each is.

use serde_json::{Value, json};

use super::{Answer, bool_operand, text_operand};
use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::document::{DocumentKey, UndoTarget};
use crate::app::document_ids::{DocumentEntry, DocumentKind};

/// Which documents a listing keeps: every filter given must hold.
pub(super) struct DocumentFilter {
    kind: Option<DocumentKind>,
    project: Option<String>,
    name: Option<String>,
    dirty: Option<bool>,
}

impl DocumentFilter {
    /// The filters in `args`; a `kind` that is no document kind is refused.
    pub(super) fn from_args(args: &CommandArgs) -> Result<Self, String> {
        let kind = match text_operand(args, "kind") {
            None => None,
            Some(text) => Some(DocumentKind::parse(text).ok_or_else(|| {
                let kinds: Vec<&str> = DocumentKind::ALL.iter().map(|k| k.as_str()).collect();
                format!("`{text}` is not a document kind ({})", kinds.join(", "))
            })?),
        };
        Ok(Self {
            kind,
            project: text_operand(args, "project").map(str::to_lowercase),
            name: text_operand(args, "name").map(str::to_lowercase),
            dirty: bool_operand(args, "dirty"),
        })
    }

    pub(super) fn matches(&self, entry: &DocumentEntry) -> bool {
        self.kind.is_none_or(|kind| entry.kind == kind)
            && self.dirty.is_none_or(|dirty| entry.dirty == dirty)
            && self
                .name
                .as_ref()
                .is_none_or(|name| entry.name.to_lowercase().contains(name))
            && self.project.as_ref().is_none_or(|wanted| {
                entry.project.as_ref().is_some_and(|project| {
                    project.name.to_lowercase() == *wanted
                        || project.path.display().to_string().to_lowercase() == *wanted
                        || project.id.is_some_and(|id| id.to_string() == *wanted)
                })
            })
    }
}

impl Signex {
    pub(super) fn list_documents(&self, args: &CommandArgs) -> Answer {
        let filter = DocumentFilter::from_args(args)?;
        let documents: Vec<Value> = self
            .document_entries()
            .iter()
            .filter(|entry| filter.matches(entry))
            .map(DocumentEntry::to_json)
            .collect();
        Ok(json!({ "count": documents.len(), "documents": documents }))
    }

    /// The document the required `document` operand names.
    pub(super) fn document_operand(&self, args: &CommandArgs) -> Result<DocumentEntry, String> {
        match args.operand("document") {
            Some(OperandValue::Document(target)) => self.resolve_document_target(target),
            _ => Err("`document` is needed".to_string()),
        }
    }

    pub(super) fn document_info(&self, args: &CommandArgs) -> Answer {
        let entry = self.document_operand(args)?;
        let mut info = entry.to_json();
        let details = self.document_details(&entry);
        if let (Value::Object(map), Value::Object(extra)) = (&mut info, details) {
            map.extend(extra);
        }
        Ok(info)
    }

    /// What is particular to the kind: a sheet's placements and page, a
    /// project's sheets and net classes, and whether Undo / Redo have a step.
    fn document_details(&self, entry: &DocumentEntry) -> Value {
        let history = entry.key.as_ref().map(|key| {
            let (undo, redo) = self.undo_availability(&UndoTarget::Document(key.clone()));
            json!({ "can_undo": undo, "can_redo": redo })
        });
        let mut details = json!({ "history": history });
        let kind_details = match (&entry.key, entry.kind) {
            (Some(DocumentKey::Project(id)), _) => self.project_details(*id),
            (_, DocumentKind::Schematic) => self.sheet_details(entry),
            (_, DocumentKind::Pcb) => self.board_view_details(entry),
            _ => Value::Null,
        };
        if let (Value::Object(map), Value::Object(extra)) = (&mut details, kind_details) {
            map.extend(extra);
        }
        // What saving a project writes besides its file (the libraries it has
        // pending), so a client that confines writes can check them (#1924).
        if let (Value::Object(map), DocumentKind::Project, Some(path)) =
            (&mut details, entry.kind, entry.path.as_deref())
        {
            let writes: Vec<String> = self
                .save_side_writes(path, false)
                .iter()
                .map(|file| file.display().to_string())
                .collect();
            map.insert("save_writes".into(), json!(writes));
        }
        details
    }

    /// How a board in view is turned on screen (#2316): the main window's
    /// canvas while it shows the board, else an undocked window's; `null`
    /// while no window shows it.
    fn board_view_details(&self, entry: &DocumentEntry) -> Value {
        let Some(path) = entry.path.as_deref() else {
            return Value::Null;
        };
        let shown_in_main =
            self.has_active_pcb() && self.active_tab_path().as_deref() == Some(path);
        let angle = if shown_in_main {
            Some(self.interaction_state.pcb_canvas.view_rotation_deg())
        } else {
            self.interaction_state
                .pcb_canvases
                .iter()
                .find(|(id, _)| self.undocked_pcb_path(**id).as_deref() == Some(path))
                .map(|(_, canvas)| canvas.view_rotation_deg())
        };
        json!({ "view_rotation_deg": angle })
    }

    fn sheet_details(&self, entry: &DocumentEntry) -> Value {
        let placements = self.placements_of(entry);
        let page = entry
            .path
            .as_deref()
            .and_then(|path| self.document_state.engines.get(path))
            .map(|engine| engine.document().root_sheet_page.clone());
        json!({ "placements": placements, "sheet_number": page })
    }

    fn project_details(&self, id: crate::app::state::ProjectId) -> Value {
        let Some(project) = self.document_state.project_by_id(id) else {
            return Value::Null;
        };
        let sheets: Vec<Value> = self
            .document_entries()
            .iter()
            .filter(|e| e.kind == DocumentKind::Schematic)
            .filter(|e| e.project.as_ref().is_some_and(|p| p.path == project.path))
            .map(|e| json!({ "id": e.id.map(|id| id.to_string()), "name": e.name, "path": e.path.as_ref().map(|p| p.display().to_string()), "loaded": e.loaded }))
            .collect();
        let classes: Vec<&str> = project
            .data
            .net_classes
            .iter()
            .map(|class| class.name.as_str())
            .collect();
        json!({ "sheets": sheets, "net_classes": classes })
    }
}
