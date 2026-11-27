//! `[extras.sheet.note_formats]` (#1114): the format of each rich text note,
//! by note uuid. Format: `docs/architecture/snxsch-note-format.md`.

use std::collections::BTreeMap;

use crate::schematic::{SchematicSheet, TextNote, TextNoteFormat};

/// The table a save writes: every note that is not plain, and each entry
/// this build could not read whose note is still on the sheet and plain.
pub(in crate::format) fn encode(sheet: &SchematicSheet) -> BTreeMap<String, String> {
    let mut table: BTreeMap<String, String> = sheet
        .text_notes
        .iter()
        .filter(|note| !note.format.is_plain())
        .map(|note| (note.uuid.to_string(), note.format.as_str().to_string()))
        .collect();
    for (id, word) in &sheet.unknown.note_formats {
        if is_plain_note(&sheet.text_notes, id) {
            table.insert(id.clone(), word.clone());
        }
    }
    table
}

fn is_plain_note(notes: &[TextNote], id: &str) -> bool {
    notes
        .iter()
        .any(|note| note.format.is_plain() && note.uuid.to_string() == id)
}

/// The formats read from the table, and the entries whose value this build
/// does not know (kept for the next save).
pub(in crate::format) fn decode(
    table: BTreeMap<String, String>,
) -> (BTreeMap<String, TextNoteFormat>, BTreeMap<String, String>) {
    let mut known = BTreeMap::new();
    let mut unknown = BTreeMap::new();
    for (id, word) in table {
        match TextNoteFormat::from_word(&word) {
            Some(format) => {
                known.insert(id, format);
            }
            None => {
                unknown.insert(id, word);
            }
        }
    }
    (known, unknown)
}
