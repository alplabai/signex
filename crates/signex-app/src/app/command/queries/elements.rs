//! The loaded documents' elements, with the document each lives in: the
//! rows behind `resolve` and `find_items`.

use serde_json::{Value, json};
use signex_types::pcb::PcbBoard;

use super::sheet_view::{Element, sheet_elements};
use crate::app::document::DocumentKey;
use crate::app::document_ids::{DocumentEntry, DocumentKind};
use crate::app::{Signex, TabDocument};

/// An element and where it lives.
pub(super) struct Located {
    pub element: Element,
    pub document: DocumentEntry,
}

impl Located {
    /// `{ document: {id, path, name}, placements, ..element }`.
    pub(super) fn to_json(&self, app: &Signex) -> Value {
        let mut value = self.element.to_json();
        if let Value::Object(map) = &mut value {
            map.insert(
                "document".into(),
                json!({
                    "id": self.document.id.map(|id| id.to_string()),
                    "path": self.document.path.as_ref().map(|p| p.display().to_string()),
                    "name": self.document.name,
                }),
            );
            map.insert(
                "placements".into(),
                json!(app.placements_of(&self.document)),
            );
        }
        value
    }
}

/// The objects of a board that carry a uuid.
fn pcb_elements(board: &PcbBoard) -> Vec<Element> {
    let mut out = Vec::new();
    for footprint in &board.footprints {
        let mut element = Element::new(footprint.uuid, "footprint")
            .at(footprint.position.x, footprint.position.y)
            .field("layer", footprint.layer.clone());
        element.designator = Some(footprint.reference.clone());
        element.value = Some(footprint.value.clone());
        out.push(element);
    }
    for segment in &board.segments {
        out.push(
            Element::new(segment.uuid, "track")
                .at(segment.start.x, segment.start.y)
                .field("end", json!([segment.end.x, segment.end.y]))
                .field("layer", segment.layer.clone()),
        );
    }
    for via in &board.vias {
        out.push(Element::new(via.uuid, "via").at(via.position.x, via.position.y));
    }
    for zone in &board.zones {
        let mut element = Element::new(zone.uuid, "zone").field("layer", zone.layer.clone());
        element.own_net = Some(zone.net_name.clone());
        out.push(element);
    }
    out
}

impl Signex {
    /// The placements of a sheet document within its project, as instance
    /// paths from the top-level page; empty for anything else.
    pub(super) fn placements_of(&self, entry: &DocumentEntry) -> Vec<String> {
        let docs = &self.document_state;
        entry
            .path
            .as_deref()
            .zip(entry.project.as_ref())
            .and_then(|(path, project)| {
                let project = docs.projects.iter().find(|p| p.path == project.path)?;
                let hierarchy = docs.project_fs.facts.hierarchy_of(project.id)?;
                Some(
                    hierarchy
                        .placements_of(path)
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                )
            })
            .unwrap_or_default()
    }

    /// Every element of every loaded sheet and board in `scope`, with its
    /// document. A document that is only referenced has nothing to read yet
    /// and is named in the second list.
    pub(super) fn located_elements(
        &self,
        scope: impl Fn(&DocumentEntry) -> bool,
    ) -> (Vec<Located>, Vec<DocumentEntry>) {
        let docs = &self.document_state;
        let mut found = Vec::new();
        let mut unloaded = Vec::new();
        for entry in self.document_entries().into_iter().filter(|e| scope(e)) {
            let elements = match (&entry.key, entry.kind) {
                (Some(DocumentKey::Schematic(path)), _) => docs
                    .engines
                    .get(path)
                    .map(|engine| sheet_elements(engine.document())),
                (Some(DocumentKey::Pcb(path)), _) => docs
                    .tabs
                    .iter()
                    .find(|tab| &tab.path == path)
                    .and_then(|tab| tab.cached_document.as_ref())
                    .map(|TabDocument::Pcb(board)| pcb_elements(board)),
                (None, DocumentKind::Schematic | DocumentKind::Pcb) => {
                    unloaded.push(entry);
                    continue;
                }
                _ => continue,
            };
            for element in elements.unwrap_or_default() {
                found.push(Located {
                    element,
                    document: entry.clone(),
                });
            }
        }
        (found, unloaded)
    }
}
