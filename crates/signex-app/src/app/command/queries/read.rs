//! `read_document`: a loaded document's data as JSON, read through the
//! engine and in the domain model its file uses (the same serialisation the
//! types carry), narrowed by filters and paged. The answer carries the
//! document's revision, so an agent can tell whether what it read is still
//! current. Reading changes nothing: not the UI, not the dirty mark.

use serde_json::{Map, Value, json};

use super::{Answer, text_operand};
use crate::app::command::CommandArgs;
use crate::app::document::DocumentKey;
use crate::app::{Signex, TabDocument};

/// Elements returned when the call names no `limit`.
const DEFAULT_LIMIT: usize = 500;

/// The element arrays of a sheet or board, by the `kinds=` word that selects
/// each, in the order paging walks them.
const SHEET_ARRAYS: &[(&str, &str)] = &[
    ("symbol", "symbols"),
    ("wire", "wires"),
    ("bus", "buses"),
    ("bus_entry", "bus_entries"),
    ("junction", "junctions"),
    ("label", "labels"),
    ("no_connect", "no_connects"),
    ("text", "text_notes"),
    ("sheet_entry", "child_sheets"),
    ("drawing", "drawings"),
    ("no_erc", "no_erc_directives"),
];
const BOARD_ARRAYS: &[(&str, &str)] = &[
    ("footprint", "footprints"),
    ("track", "segments"),
    ("via", "vias"),
    ("zone", "zones"),
    ("graphic", "graphics"),
    ("text", "texts"),
];

/// A rectangle in mm: `x1,y1,x2,y2`, corners in any order.
#[derive(Debug, Clone, Copy)]
struct Region {
    min: (f64, f64),
    max: (f64, f64),
}

impl Region {
    fn parse(text: &str) -> Result<Self, String> {
        let parts: Vec<f64> = text
            .split(',')
            .map(|p| p.trim().parse::<f64>().ok().filter(|n| n.is_finite()))
            .collect::<Option<_>>()
            .filter(|p: &Vec<f64>| p.len() == 4)
            .ok_or_else(|| format!("`{text}` is not a region (x1,y1,x2,y2 in mm)"))?;
        Ok(Self {
            min: (parts[0].min(parts[2]), parts[1].min(parts[3])),
            max: (parts[0].max(parts[2]), parts[1].max(parts[3])),
        })
    }

    fn holds(&self, x: f64, y: f64) -> bool {
        (self.min.0..=self.max.0).contains(&x) && (self.min.1..=self.max.1).contains(&y)
    }

    /// Whether an element has an anchor inside: its `position`, or either end.
    fn touches(&self, element: &Value) -> bool {
        ["position", "start", "end"].iter().any(|key| {
            element.get(key).is_some_and(|p| {
                matches!((p.get("x").and_then(Value::as_f64), p.get("y").and_then(Value::as_f64)),
                    (Some(x), Some(y)) if self.holds(x, y))
            })
        })
    }
}

/// The uuid of an element: its own `uuid`, or the one inside a tagged
/// variant (`{"Line": {"uuid": ...}}`).
fn uuid_of(element: &Value) -> Option<String> {
    let direct = element.get("uuid").and_then(Value::as_str);
    let nested = element
        .as_object()
        .and_then(|map| map.values().find_map(|v| v.get("uuid")?.as_str()));
    direct.or(nested).map(str::to_lowercase)
}

/// The narrowing a call asked for.
struct Narrowing {
    ids: Option<Vec<String>>,
    kinds: Option<Vec<String>>,
    region: Option<Region>,
    fields: Option<Vec<String>>,
    offset: usize,
    limit: usize,
}

impl Narrowing {
    fn from_args(args: &CommandArgs) -> Result<Self, String> {
        let list = |name| {
            text_operand(args, name).map(|text| {
                text.split(',')
                    .map(|part| part.trim().to_lowercase())
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
            })
        };
        let count = |name: &str, default: usize| -> Result<usize, String> {
            text_operand(args, name).map_or(Ok(default), |text| {
                text.trim()
                    .parse::<usize>()
                    .map_err(|_| format!("`{name}` must be a whole number, got `{text}`"))
            })
        };
        let limit = count("limit", DEFAULT_LIMIT)?;
        if limit == 0 {
            return Err("`limit` must be at least 1".into());
        }
        Ok(Self {
            ids: list("ids"),
            kinds: list("kinds"),
            region: text_operand(args, "region")
                .map(Region::parse)
                .transpose()?,
            fields: list("fields"),
            offset: count("offset", 0)?,
            limit,
        })
    }

    fn is_narrowed(&self) -> bool {
        self.ids.is_some() || self.kinds.is_some() || self.region.is_some() || self.fields.is_some()
    }

    fn keeps(&self, element: &Value) -> bool {
        self.ids
            .as_ref()
            .is_none_or(|ids| uuid_of(element).is_some_and(|id| ids.contains(&id)))
            && self.region.is_none_or(|region| region.touches(element))
    }

    fn project(&self, element: &Value) -> Value {
        let (Some(fields), Value::Object(map)) = (&self.fields, element) else {
            return element.clone();
        };
        let kept: Map<String, Value> = map
            .iter()
            .filter(|(key, _)| key.as_str() == "uuid" || fields.contains(&key.to_lowercase()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Value::Object(kept)
    }
}

/// `data` split into its header (everything that is not an element array)
/// and the narrowed, paged element arrays.
fn narrow(mut data: Value, arrays: &[(&str, &str)], narrowing: &Narrowing) -> Value {
    let Value::Object(map) = &mut data else {
        return data;
    };
    let mut elements = Map::new();
    let mut total = 0usize;
    let mut skipped = 0usize;
    let mut taken = 0usize;
    for (kind, key) in arrays {
        let Some(Value::Array(items)) = map.remove(*key) else {
            continue;
        };
        if narrowing
            .kinds
            .as_ref()
            .is_some_and(|kinds| !kinds.iter().any(|k| k == kind))
        {
            continue;
        }
        let mut page = Vec::new();
        for item in items.iter().filter(|item| narrowing.keeps(item)) {
            total += 1;
            if skipped < narrowing.offset {
                skipped += 1;
            } else if taken < narrowing.limit {
                taken += 1;
                page.push(narrowing.project(item));
            }
        }
        elements.insert((*key).to_string(), Value::Array(page));
    }
    // A narrowed read is about the elements; the shared library table is
    // heavy and is part of the page only in a whole read.
    if narrowing.is_narrowed() {
        map.remove("lib_symbols");
    }
    let next = (narrowing.offset + taken < total).then_some(narrowing.offset + taken);
    json!({
        "header": data,
        "elements": elements,
        "total": total,
        "offset": narrowing.offset,
        "limit": narrowing.limit,
        "next_offset": next,
    })
}

impl Signex {
    pub(super) fn read_document(&self, args: &CommandArgs) -> Answer {
        let entry = self.document_operand(args)?;
        let narrowing = Narrowing::from_args(args)?;
        let docs = &self.document_state;
        let (data, arrays, revision): (Value, &[(&str, &str)], Value) = match &entry.key {
            Some(DocumentKey::Schematic(path)) => {
                let engine = docs.engines.get(path).ok_or("the sheet is not loaded")?;
                (
                    serde_json::to_value(engine.document()).map_err(|e| e.to_string())?,
                    SHEET_ARRAYS,
                    json!(engine.revision()),
                )
            }
            Some(DocumentKey::Pcb(path)) => {
                let board = docs
                    .tabs
                    .iter()
                    .find(|tab| &tab.path == path)
                    .and_then(|tab| tab.cached_document.as_ref())
                    .map(|TabDocument::Pcb(board)| board)
                    .ok_or("the board is not loaded")?;
                (
                    serde_json::to_value(board).map_err(|e| e.to_string())?,
                    BOARD_ARRAYS,
                    json!(docs.pcb_revisions.get(path)),
                )
            }
            Some(DocumentKey::Symbol(path)) => {
                let file = &docs
                    .symbol_editors
                    .get(path)
                    .ok_or("the symbol file is not loaded")?
                    .file;
                (
                    serde_json::to_value(file).map_err(|e| e.to_string())?,
                    &[],
                    Value::Null,
                )
            }
            Some(DocumentKey::Footprint(path)) => {
                let file = &docs
                    .footprint_editors
                    .get(path)
                    .ok_or("the footprint file is not loaded")?
                    .file;
                (
                    serde_json::to_value(file).map_err(|e| e.to_string())?,
                    &[],
                    Value::Null,
                )
            }
            Some(DocumentKey::Project(id)) => {
                let project = docs.project_by_id(*id).ok_or("the project is not loaded")?;
                (
                    serde_json::to_value(&project.data).map_err(|e| e.to_string())?,
                    &[],
                    Value::Null,
                )
            }
            Some(DocumentKey::Row(_)) => {
                return Err("a component row has no document data to read yet".into());
            }
            None => {
                return Err(format!(
                    "{} is referenced by a project but not loaded; open it first",
                    entry
                        .path
                        .as_ref()
                        .map_or_else(String::new, |p| p.display().to_string())
                ));
            }
        };
        let mut answer = narrow(data, arrays, &narrowing);
        if let Value::Object(map) = &mut answer {
            map.insert("document".into(), entry.to_json());
            map.insert("revision".into(), revision);
        }
        Ok(answer)
    }
}
