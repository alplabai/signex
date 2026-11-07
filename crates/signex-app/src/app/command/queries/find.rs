//! `resolve` and `find`: from a name a person knows to the uuid an agent acts
//! on, and back.

use serde_json::{Value, json};
use uuid::Uuid;

use super::documents::DocumentFilter;
use super::sheet_view::{ELEMENT_KINDS, elements_on_net, net_name_of_wire};
use super::{Answer, text_operand};
use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentEntry;

/// The element filters of a `find`; every one given must hold.
struct ElementFilter {
    kind: Option<String>,
    designator: Option<String>,
    value: Option<String>,
    net: Option<String>,
    text: Option<String>,
}

impl ElementFilter {
    fn from_args(args: &CommandArgs) -> Self {
        let lowered = |name| text_operand(args, name).map(str::to_lowercase);
        Self {
            kind: lowered("kind"),
            designator: lowered("designator"),
            value: lowered("value"),
            net: lowered("net"),
            text: lowered("text"),
        }
    }

    /// Whether the query asks about elements: an element filter is given, or
    /// `kind` names an element kind and no document kind.
    fn is_asked(&self) -> bool {
        self.designator.is_some()
            || self.value.is_some()
            || self.net.is_some()
            || self.text.is_some()
            || self
                .kind
                .as_deref()
                .is_some_and(|kind| ELEMENT_KINDS.contains(&kind) || is_board_kind(kind))
    }
}

fn is_board_kind(kind: &str) -> bool {
    matches!(kind, "footprint" | "track" | "via" | "zone")
}

fn contains(field: &Option<String>, wanted: &str) -> bool {
    field
        .as_deref()
        .is_some_and(|text| text.to_lowercase().contains(wanted))
}

fn equals(field: &Option<String>, wanted: &str) -> bool {
    field
        .as_deref()
        .is_some_and(|text| text.to_lowercase() == wanted)
}

impl Signex {
    /// `resolve id=<uuid>`: what a uuid points to, a document or an element.
    pub(super) fn resolve_id(&self, args: &CommandArgs) -> Answer {
        let text = text_operand(args, "id").unwrap_or_default();
        let uuid = Uuid::parse_str(text)
            .ok()
            .filter(|_| text.len() == 36)
            .ok_or_else(|| format!("`{text}` is not an id (a hyphenated uuid)"))?;
        let entries = self.document_entries();
        if let Some(entry) = entries
            .iter()
            .find(|e| e.id.map(|id| id.as_uuid()) == Some(uuid))
        {
            return Ok(json!({ "found": true, "what": "document", "document": entry.to_json() }));
        }
        let (located, _) = self.located_elements(|_| true);
        let Some(hit) = located.iter().find(|l| l.element.id == uuid) else {
            return Ok(json!({ "found": false, "what": "unknown", "id": uuid.to_string() }));
        };
        let mut element = hit.to_json(self);
        // A wire has no name of its own: the net it is on does.
        if hit.element.kind == "wire"
            && let Some(DocumentKey::Schematic(path)) = &hit.document.key
            && let Some(engine) = self.document_state.engines.get(path)
            && let Some(net) = net_name_of_wire(engine.document(), uuid)
            && let Value::Object(map) = &mut element
        {
            map.insert("net".into(), json!(net));
        }
        Ok(json!({ "found": true, "what": "element", "element": element }))
    }

    /// `find`: documents by `kind`, `project`, `name`, `dirty`; elements by
    /// `kind`, `designator`, `value`, `net`, `text`, in `document` or in
    /// `project` or in every open document.
    pub(super) fn find(&self, args: &CommandArgs) -> Answer {
        let elements = ElementFilter::from_args(args);
        if !elements.is_asked() {
            return self.find_documents(args);
        }
        let scope_document = match args.operand("document") {
            Some(OperandValue::Document(target)) => Some(self.resolve_document_target(target)?),
            _ => None,
        };
        let project = text_operand(args, "project").map(str::to_lowercase);
        let in_scope = |entry: &DocumentEntry| {
            scope_document.as_ref().is_none_or(|d| d.path == entry.path)
                && project.as_ref().is_none_or(|wanted| {
                    entry.project.as_ref().is_some_and(|p| {
                        p.name.to_lowercase() == *wanted
                            || p.id.is_some_and(|id| id.to_string() == *wanted)
                            || p.path.display().to_string().to_lowercase() == *wanted
                    })
                })
        };
        let (located, unloaded) = self.located_elements(in_scope);
        let on_net: std::collections::HashMap<std::path::PathBuf, Vec<Uuid>> = match &elements.net {
            Some(net) => self.net_members(&located, net),
            None => Default::default(),
        };
        let hits: Vec<Value> = located
            .iter()
            .filter(|l| {
                let e = &l.element;
                elements.kind.as_deref().is_none_or(|k| e.kind == k)
                    && elements
                        .designator
                        .as_ref()
                        .is_none_or(|d| equals(&e.designator, d))
                    && elements
                        .value
                        .as_ref()
                        .is_none_or(|v| contains(&e.value, v))
                    && elements.text.as_ref().is_none_or(|t| contains(&e.text, t))
                    && elements.net.as_ref().is_none_or(|_| {
                        l.document
                            .path
                            .as_ref()
                            .and_then(|p| on_net.get(p))
                            .is_some_and(|ids| ids.contains(&e.id))
                    })
            })
            .map(|l| l.to_json(self))
            .collect();
        let not_loaded: Vec<String> = unloaded
            .iter()
            .filter_map(|e| e.path.as_ref().map(|p| p.display().to_string()))
            .collect();
        Ok(json!({ "count": hits.len(), "elements": hits, "not_loaded": not_loaded }))
    }

    /// For each loaded sheet in `located`, the element ids on the net `net`.
    fn net_members(
        &self,
        located: &[super::elements::Located],
        net: &str,
    ) -> std::collections::HashMap<std::path::PathBuf, Vec<Uuid>> {
        let mut by_path = std::collections::HashMap::new();
        for l in located {
            let Some(DocumentKey::Schematic(path)) = &l.document.key else {
                continue;
            };
            let Some(engine) = self.document_state.engines.get(path) else {
                continue;
            };
            by_path
                .entry(path.clone())
                .or_insert_with(|| elements_on_net(engine.document(), net));
        }
        by_path
    }

    fn find_documents(&self, args: &CommandArgs) -> Answer {
        let filter = DocumentFilter::from_args(args)?;
        let documents: Vec<Value> = self
            .document_entries()
            .iter()
            .filter(|entry| filter.matches(entry))
            .map(DocumentEntry::to_json)
            .collect();
        Ok(json!({ "count": documents.len(), "documents": documents }))
    }
}
