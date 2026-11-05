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

pub(in crate::format) type ExtentTable = BTreeMap<String, ExtentEntry>;

pub(in crate::format) fn encode(extents: &TextExtents) -> ExtentTable {
    extents
        .iter()
        .map(|((id, field), rec)| {
            (
                format!("{id}:{}", field.file_key()),
                ExtentEntry {
                    width_nm: rec.width_nm,
                    size_nm: rec.size_nm,
                    family: rec.family.clone(),
                    text: rec.text_hash.clone(),
                },
            )
        })
        .collect()
}

/// A key that is not `<uuid>:<field>` or a negative width matches nothing and
/// is dropped, like an entry for an item that is gone.
pub(in crate::format) fn decode(table: &ExtentTable) -> TextExtents {
    table
        .iter()
        .filter(|(_, entry)| entry.width_nm >= 0)
        .filter_map(|(key, entry)| {
            let (id, field) = key.split_once(':')?;
            Some((
                (Uuid::parse_str(id).ok()?, TextField::from_file_key(field)?),
                StoredTextExtent {
                    width_nm: entry.width_nm,
                    size_nm: entry.size_nm,
                    family: entry.family.clone(),
                    text_hash: entry.text.clone(),
                },
            ))
        })
        .collect()
}
