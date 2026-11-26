//! `query_library library=<id or path> [part=<row id or part number>]
//! [text=<text>]` (#2944): the rows of an open library as the app holds
//! them, every table in name order, narrowed to one row or to the rows whose
//! part numbers, manufacturer, class or description hold `text` (case
//! folded). Reads no file. Shape: `docs/CLI.md`, "Library rows".

use serde_json::{Map, Value, json};
use signex_library::ComponentRow;

use super::{Answer, text_operand};
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::command::part_operands::library_row;

impl Signex {
    pub(super) fn query_library(&self, args: &CommandArgs) -> Answer {
        let Some(named) = text_operand(args, "library") else {
            return Err("`query_library` needs the operand `library`".to_string());
        };
        let library = self.open_library_named(named)?;
        let one = match text_operand(args, "part") {
            Some(part) => Some(library_row(library, part)?),
            None => None,
        };
        let search = text_operand(args, "text").map(str::to_lowercase);
        let mut tables: Vec<&String> = library.tables.keys().collect();
        tables.sort_unstable();
        let mut rows = Vec::new();
        for table in tables {
            for row in &library.tables[table] {
                let wanted = match &one {
                    Some((t, id)) => t == table && *id == row.row_id,
                    None => search.as_deref().is_none_or(|text| matches(row, text)),
                };
                if wanted {
                    rows.push(row_json(table, row));
                }
            }
        }
        Ok(json!({
            "library": {
                "id": library.library_id.to_string(),
                "name": library.display_name,
                "path": library.root.display().to_string(),
            },
            "count": rows.len(),
            "rows": rows,
        }))
    }
}

/// Whether `row` holds `text` (already lower case) in a part number, its
/// manufacturer, its class or its description.
fn matches(row: &ComponentRow, text: &str) -> bool {
    [
        row.internal_pn.as_str(),
        row.primary_mpn.mpn.as_str(),
        row.primary_mpn.manufacturer.as_str(),
        row.class.as_str(),
        row.description.as_str(),
    ]
    .iter()
    .any(|field| field.to_lowercase().contains(text))
}

/// One row as `query_library` answers it, and as a library edit command
/// answers the row it left.
pub(crate) fn row_json(table: &str, row: &ComponentRow) -> Value {
    let primitive = |library: uuid::Uuid, id: uuid::Uuid, version: &str| json!({ "library": library.to_string(), "id": id.to_string(), "version": version });
    let parameters: Map<String, Value> = row
        .parameters
        .iter()
        .map(|(key, value)| {
            let value = serde_json::to_value(value).unwrap_or(Value::Null);
            (key.clone(), value)
        })
        .collect();
    let alternates: Vec<Value> = row
        .alternates
        .iter()
        .map(|part| json!({ "manufacturer": part.manufacturer, "mpn": part.mpn }))
        .collect();
    json!({
        "id": row.row_id.to_string(),
        "table": table,
        "internal_pn": row.internal_pn.as_str(),
        "class": row.class.as_str(),
        "description": row.description,
        "state": row.state.as_str(),
        "version": row.version,
        "released": row.released,
        "manufacturer": row.primary_mpn.manufacturer,
        "mpn": row.primary_mpn.mpn,
        "alternates": alternates,
        "symbol": primitive(row.symbol_ref.library_id, row.symbol_ref.uuid, &row.symbol_version),
        "footprint": row
            .footprint_ref
            .as_ref()
            .map(|f| primitive(f.library_id, f.uuid, &row.footprint_version)),
        "parameters": parameters,
        "revision": row.server_revision,
    })
}
