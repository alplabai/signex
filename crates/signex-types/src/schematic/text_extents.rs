//! Measured text widths stored in the file (#2029).
//!
//! The live cache ([`super::TextWidths`]) answers while the face is installed.
//! A machine without the face (another computer, a headless export, an agent)
//! reads the width the author's machine measured instead of guessing with a
//! substitute face. Format and staleness rules: `docs/architecture/snxsch-text-extents.md`.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{SchematicSheet, TextField, text_height_mm};
use crate::font::{BUNDLED_DEFAULT_FAMILY, requested_family};
use crate::format::mm_as_stored_nm;

impl TextField {
    /// The field name used in the file key.
    pub fn file_key(self) -> &'static str {
        match self {
            Self::Label => "label",
            Self::Note => "note",
            Self::SymbolReference => "ref",
            Self::SymbolValue => "value",
        }
    }

    pub fn from_file_key(key: &str) -> Option<Self> {
        [
            Self::Label,
            Self::Note,
            Self::SymbolReference,
            Self::SymbolValue,
        ]
        .into_iter()
        .find(|field| field.file_key() == key)
    }
}

/// One stored width and the inputs it was measured from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredTextExtent {
    pub width_nm: i64,
    /// The measured text height (`text_height_mm`), nm.
    pub size_nm: i64,
    /// The requested family (item's, else the sheet default); `None` if none.
    pub family: Option<String>,
    /// [`text_hash`] of the exact measured text.
    pub text_hash: String,
}

impl StoredTextExtent {
    /// Whether the record was measured from these inputs.
    fn matches(&self, requested: Option<&str>, size_nm: i64, text_hash: &str) -> bool {
        self.size_nm == size_nm
            && self.family.as_deref() == requested
            && self.text_hash == text_hash
    }

    pub fn width_mm(&self) -> f64 {
        self.width_nm as f64 / 1_000_000.0
    }
}

/// 16 lowercase hex characters of the SHA-256 of `text`'s UTF-8 bytes.
pub fn text_hash(text: &str) -> String {
    Sha256::digest(text.as_bytes())[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// What a text is measured from; the inputs a record is checked against.
#[derive(Clone, Copy, Debug)]
pub struct TextProbe<'a> {
    pub id: Uuid,
    pub field: TextField,
    pub text: &'a str,
    /// The family the item asks for (own, else the sheet default).
    pub requested: Option<&'a str>,
    pub size_mm: f64,
}

/// The stored widths of one sheet. Loaded with the file, rewritten only at
/// save time on the copy being serialised; edits and the engine never touch it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextExtents(BTreeMap<(Uuid, TextField), StoredTextExtent>);

impl FromIterator<((Uuid, TextField), StoredTextExtent)> for TextExtents {
    fn from_iter<I: IntoIterator<Item = ((Uuid, TextField), StoredTextExtent)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl TextExtents {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&(Uuid, TextField), &StoredTextExtent)> {
        self.0.iter()
    }

    /// Whether a record exists for `id`'s `field`, valid or not.
    pub fn contains(&self, id: Uuid, field: TextField) -> bool {
        self.0.contains_key(&(id, field))
    }

    /// The record of `id`'s `field` if it was measured from these inputs.
    pub fn matching(&self, probe: &TextProbe<'_>) -> Option<&StoredTextExtent> {
        let size_nm = mm_as_stored_nm(probe.size_mm)?;
        self.0
            .get(&(probe.id, probe.field))
            .filter(|rec| rec.matches(probe.requested, size_nm, &text_hash(probe.text)))
    }

    /// The records still valid for `sheet`, with `measure`'s fresh widths
    /// written over them. `measure` answers only for a final measurement in
    /// an installed face; `None` keeps the loaded record. A record for a
    /// missing item or from other inputs is dropped, and a width or size the
    /// file cannot hold skips that entry rather than failing the save.
    pub fn refreshed(
        &self,
        sheet: &SchematicSheet,
        mut measure: impl FnMut(&TextProbe<'_>) -> Option<f64>,
    ) -> Self {
        let mut out = BTreeMap::new();
        for probe in probes(sheet) {
            let Some(size_nm) = mm_as_stored_nm(probe.size_mm) else {
                continue;
            };
            let fresh = measure(&probe)
                .and_then(mm_as_stored_nm)
                .filter(|width_nm| *width_nm >= 0)
                .map(|width_nm| StoredTextExtent {
                    width_nm,
                    size_nm,
                    family: probe.requested.map(str::to_string),
                    text_hash: text_hash(probe.text),
                });
            if let Some(record) = fresh.or_else(|| self.matching(&probe).cloned()) {
                out.insert((probe.id, probe.field), record);
            }
        }
        Self(out)
    }

    /// [`Self::refreshed`] with no new measurement: the valid records only.
    pub fn retained(&self, sheet: &SchematicSheet) -> Self {
        self.refreshed(sheet, |_| None)
    }
}

/// Every text of `sheet` that has a stored width, as the canvas measures it.
pub fn probes(sheet: &SchematicSheet) -> Vec<TextProbe<'_>> {
    let document = sheet.font_family.as_deref();
    let mut out = Vec::new();
    for label in &sheet.labels {
        out.push(TextProbe {
            id: label.uuid,
            field: TextField::Label,
            text: &label.text,
            requested: requested_family(label.font_family.as_deref(), document),
            size_mm: text_height_mm(label.font_size),
        });
    }
    for note in &sheet.text_notes {
        out.push(TextProbe {
            id: note.uuid,
            field: TextField::Note,
            text: &note.text,
            requested: requested_family(note.font_family.as_deref(), document),
            size_mm: text_height_mm(note.font_size),
        });
    }
    for symbol in &sheet.symbols {
        let fields = [
            (
                TextField::SymbolReference,
                symbol
                    .ref_text
                    .as_ref()
                    .filter(|_| symbol.draws_reference()),
                symbol.reference.as_str(),
            ),
            (
                TextField::SymbolValue,
                symbol.val_text.as_ref(),
                symbol.value.as_str(),
            ),
        ];
        for (field, prop, text) in fields {
            let Some(prop) = prop.filter(|prop| !prop.hidden) else {
                continue;
            };
            out.push(TextProbe {
                id: symbol.uuid,
                field,
                text,
                requested: requested_family(prop.font_family.as_deref(), document),
                size_mm: text_height_mm(prop.font_size),
            });
        }
    }
    out
}

/// Whether the face a text asked for is the one it was measured in.
pub fn face_is_available(requested: Option<&str>, resolved: Option<&str>) -> bool {
    let is_default = |name: Option<&str>| {
        name.is_none_or(|name| name.eq_ignore_ascii_case(BUNDLED_DEFAULT_FAMILY))
    };
    match (requested, resolved) {
        (req, res) if is_default(req) => is_default(res),
        (Some(req), Some(res)) => req.eq_ignore_ascii_case(res),
        _ => false,
    }
}

/// The width a text is drawn and boxed with, and whether it may be cached.
///
/// An installed face with a final measurement wins (`live`, the measurement in
/// `resolved`). Otherwise a stored record measured from these inputs, otherwise
/// `live`, which is then the substitute face's width. A stored width is never
/// cacheable: it must stay the answer until the face appears.
pub fn decide_width_mm(
    probe: &TextProbe<'_>,
    resolved: Option<&str>,
    stored: &TextExtents,
    live: impl FnOnce() -> (f64, bool),
) -> (f64, bool) {
    if face_is_available(probe.requested, resolved) {
        let measured = live();
        if measured.1 {
            return measured;
        }
        return match stored.matching(probe) {
            Some(record) => (record.width_mm(), false),
            None => measured,
        };
    }
    match stored.matching(probe) {
        Some(record) => (record.width_mm(), false),
        None => live(),
    }
}

#[cfg(test)]
#[path = "text_extents_tests.rs"]
mod tests;
