//! The one commit path of a block diagram edit. The canvas, Properties, the
//! toolbar and the `block_*` commands all build a [`DiagramEdit`] and run it
//! through [`Signex::edit_block_diagram`], so one edit behaves the same on
//! every surface: refused on a read-only diagram, applied to a copy that
//! replaces the diagram only when every step held, one undo step, the dirty
//! mark following the file, and a JSON answer naming what changed.

use std::path::Path;

use serde_json::{Value, json};
use signex_types::block_diagram::{
    BlockDiagram, BlockDiagramError, BlockFields, BlockPatch, Endpoint, Interface, PortDirection,
    PortSide,
};
use signex_types::object_id::{IdSource, UuidV7Ids};
use signex_types::theme::Color;
use uuid::Uuid;

use super::file_name;
use super::refusal::Refusal;
use super::state::{DiagramItem, has_item};
use crate::app::Signex;
use crate::app::state::DocumentState;

/// One edit of a block diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DiagramEdit {
    AddBlock(BlockFields),
    SetBlock(Uuid, BlockPatch),
    RemoveBlock(Uuid),
    AddPort {
        block: Uuid,
        name: String,
        side: PortSide,
        offset_mm: f64,
        direction: PortDirection,
    },
    RemovePort(Uuid),
    Connect {
        from: Endpoint,
        to: Endpoint,
        interface: Interface,
        label: String,
    },
    SetConnection {
        id: Uuid,
        interface: Option<Interface>,
        label: Option<String>,
    },
    Disconnect(Uuid),
    AddGroup {
        name: String,
        color: Option<Color>,
        blocks: Vec<Uuid>,
    },
    SetGroup {
        id: Uuid,
        name: Option<String>,
        color: Option<Option<Color>>,
    },
    RemoveGroup(Uuid),
    /// The selection deleted as one step. A block takes its ports and
    /// connections with it; an object already gone with an earlier one is
    /// passed over.
    RemoveItems(Vec<DiagramItem>),
}

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Apply `edit` to `diagram`; answers what it made or changed.
pub(crate) fn apply(
    diagram: &mut BlockDiagram,
    ids: &mut dyn IdSource,
    edit: DiagramEdit,
) -> Result<Value, BlockDiagramError> {
    Ok(match edit {
        DiagramEdit::AddBlock(fields) => {
            let id = diagram.add_block(ids, fields)?;
            json!({ "id": id, "block": to_json(&diagram.block(id)) })
        }
        DiagramEdit::SetBlock(id, patch) => {
            diagram.set_block(id, patch)?;
            json!({ "id": id, "block": to_json(&diagram.block(id)) })
        }
        DiagramEdit::RemoveBlock(id) => {
            let removed = diagram.remove_block(id)?;
            let links: Vec<Uuid> = removed.connections.iter().map(|c| c.id).collect();
            json!({ "id": id, "connections": links })
        }
        DiagramEdit::AddPort {
            block,
            name,
            side,
            offset_mm,
            direction,
        } => {
            let id = diagram.add_port(ids, block, &name, side, offset_mm, direction)?;
            json!({ "id": id, "block": block })
        }
        DiagramEdit::RemovePort(id) => {
            let removed = diagram.remove_port(id)?;
            let links: Vec<Uuid> = removed.iter().map(|c| c.id).collect();
            json!({ "id": id, "connections": links })
        }
        DiagramEdit::Connect {
            from,
            to,
            interface,
            label,
        } => {
            let id = diagram.connect(ids, from, to, interface, &label)?;
            json!({ "id": id, "connection": to_json(&diagram.connection(id)) })
        }
        DiagramEdit::SetConnection {
            id,
            interface,
            label,
        } => {
            diagram.set_connection(id, interface, label.as_deref())?;
            json!({ "id": id, "connection": to_json(&diagram.connection(id)) })
        }
        DiagramEdit::Disconnect(id) => {
            diagram.disconnect(id)?;
            json!({ "id": id })
        }
        DiagramEdit::AddGroup {
            name,
            color,
            blocks,
        } => {
            let id = diagram.add_group(ids, &name, color)?;
            for block in &blocks {
                let patch = BlockPatch {
                    group: Some(Some(id)),
                    ..BlockPatch::default()
                };
                diagram.set_block(*block, patch)?;
            }
            json!({ "id": id, "blocks": blocks })
        }
        DiagramEdit::SetGroup { id, name, color } => {
            diagram.set_group(id, name.as_deref(), color)?;
            json!({ "id": id, "group": to_json(&diagram.group(id)) })
        }
        DiagramEdit::RemoveGroup(id) => {
            let freed: Vec<Uuid> = diagram
                .blocks
                .iter()
                .filter(|b| b.group == Some(id))
                .map(|b| b.id)
                .collect();
            diagram.remove_group(id)?;
            json!({ "id": id, "blocks": freed })
        }
        DiagramEdit::RemoveItems(items) => {
            let mut removed: Vec<Uuid> = Vec::new();
            for item in items {
                if !has_item(diagram, item) {
                    continue;
                }
                let id = match item {
                    DiagramItem::Block(id) => diagram.remove_block(id).map(|_| id)?,
                    DiagramItem::Port(id) => diagram.remove_port(id).map(|_| id)?,
                    DiagramItem::Connection(id) => diagram.disconnect(id).map(|_| id)?,
                    DiagramItem::Group(id) => diagram.remove_group(id).map(|_| id)?,
                };
                removed.push(id);
            }
            json!({ "removed": removed })
        }
    })
}

impl DocumentState {
    /// Mark the diagram at `path` dirty exactly when it differs from its
    /// file, so an undo back to the saved state reads clean.
    pub(crate) fn sync_block_diagram_dirty(&mut self, path: &Path) {
        let dirty = self
            .block_diagrams
            .editors
            .get(path)
            .is_some_and(|editor| editor.is_dirty());
        if dirty {
            self.mark_document_dirty_display_only(path);
        } else {
            self.clear_document_dirty(path);
        }
    }
}

impl Signex {
    /// Run `edit` on the diagram at `path` as one undo step; answers what
    /// changed, or why nothing did. An edit that leaves the diagram as it
    /// was answers without a step.
    pub(crate) fn edit_block_diagram(
        &mut self,
        path: &Path,
        edit: DiagramEdit,
    ) -> Result<Value, Refusal> {
        let depth = self.document_state.undo_depth;
        let editor = self
            .document_state
            .block_diagrams
            .editors
            .get_mut(path)
            .ok_or_else(|| Refusal::NotOpen {
                file: file_name(path),
            })?;
        if editor.is_read_only() {
            return Err(Refusal::ReadOnly {
                file: file_name(path),
                token: editor.token.clone(),
            });
        }
        let mut next = editor.diagram.clone();
        let answer = apply(&mut next, &mut UuidV7Ids, edit)?;
        if next != editor.diagram {
            // The view stays where it is while the diagram grows.
            let view = super::view_of(&mut self.document_view.block_diagrams, path);
            view.camera.hold(&editor.diagram);
            let before = std::mem::replace(&mut editor.diagram, next);
            editor.record(before, depth);
            view.prune(&editor.diagram);
            self.document_state.sync_block_diagram_dirty(path);
        }
        Ok(answer)
    }

    /// One undo or redo step of the diagram at `path`; `false` with none.
    pub(crate) fn step_block_diagram(&mut self, path: &Path, undo: bool) -> bool {
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return false;
        };
        if editor.is_read_only() {
            return false;
        }
        let view = super::view_of(&mut self.document_view.block_diagrams, path);
        view.camera.hold(&editor.diagram);
        if !editor.step(undo) {
            return false;
        }
        view.prune(&editor.diagram);
        self.document_state.sync_block_diagram_dirty(path);
        true
    }
}
