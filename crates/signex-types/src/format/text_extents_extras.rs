//! `[extras.sheet.text_extents]` (#2029): the file shape of the stored text
//! widths. Key `"<uuid>:<field>"`; integers only. Format:
//! `docs/architecture/snxsch-text-extents.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schematic::{StoredTextExtent, TextExtents, TextField};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::format) struct ExtentEntry {
    width_nm: i64,
    size_nm: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    family: Option<String>,
    /// [`crate::schematic::text_hash`] of the measured text.
    text: String,
}

/// Values stay raw: derived data must never fail the parse of a design, so a
/// malformed entry is dropped by [`decode`], not rejected by serde.
pub(in crate::format) type ExtentTable = BTreeMap<String, toml::Value>;

pub(in crate::format) fn encode(extents: &TextExtents) -> ExtentTable {
    extents
        .iter()
        .map(|((id, field), rec)| {
            let entry = ExtentEntry {
                width_nm: rec.width_nm,
                size_nm: rec.size_nm,
                family: rec.family.clone(),
                text: rec.text_hash.clone(),
            };
            let value = toml::Value::try_from(entry).expect("integers and strings encode");
            (format!("{id}:{}", field.file_key()), value)
        })
        .collect()
}

/// The records of `table` and how many entries were dropped. A key that is
/// not `<uuid>:<field>`, a value that is not a well-typed entry and a negative
/// width match nothing and are dropped, like an entry for an item that is gone.
pub(in crate::format) fn decode(table: &ExtentTable) -> (TextExtents, usize) {
    let records: TextExtents = table
        .iter()
        .filter_map(|(key, value)| decode_one(key, value))
        .collect();
    (records.clone(), table.len() - records.len())
}

fn decode_one(key: &str, value: &toml::Value) -> Option<((Uuid, TextField), StoredTextExtent)> {
    let (id, field) = key.split_once(':')?;
    let entry: ExtentEntry = value.clone().try_into().ok()?;
    if entry.width_nm < 0 {
        return None;
    }
    let record = StoredTextExtent {
        width_nm: entry.width_nm,
        size_nm: entry.size_nm,
        family: entry.family,
        text_hash: entry.text,
    };
    Some((
        (Uuid::parse_str(id).ok()?, TextField::from_file_key(field)?),
        record,
    ))
}
