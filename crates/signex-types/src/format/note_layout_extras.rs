//! `[extras.sheet.note_layouts]` (#1114): the Frame and Size sections of each
//! rich text note, by note uuid. Format: `docs/architecture/snxsch-note-format.md`.
//!
//! An entry is a plain key table, written in key order, so a key or a word
//! this build does not know goes back where it was read.

use std::collections::BTreeMap;

use toml::Value;

use super::UnknownFields;
use crate::schematic::{NoteFrame, NoteSize, RichNoteLayout, SchematicSheet};

/// One note's entry, key to raw value.
pub(in crate::format) type NoteLayoutEntry = BTreeMap<String, Value>;

const FRAME: &str = "frame";
const BORDER: &str = "border_mm";
const PADDING: &str = "padding_mm";
const SIZE: &str = "size";
const WIDTH: &str = "width_mm";
const HEIGHT: &str = "height_mm";

/// The keys this build reads; any other key is kept as read.
const KNOWN: [&str; 6] = [FRAME, BORDER, PADDING, SIZE, WIDTH, HEIGHT];

fn entry_of(layout: &RichNoteLayout, kept: Option<&UnknownFields>) -> NoteLayoutEntry {
    let mut entry: NoteLayoutEntry = kept
        .map(|kept| {
            kept.entries()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    let default = RichNoteLayout::default();
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            entry.insert(key.to_string(), value);
        }
    };
    put(
        FRAME,
        (layout.frame != default.frame).then(|| Value::String(layout.frame.as_str().into())),
    );
    put(BORDER, layout.border_mm.map(Value::Float));
    put(
        PADDING,
        (layout.padding_mm != default.padding_mm).then_some(Value::Float(layout.padding_mm)),
    );
    put(
        SIZE,
        (layout.size != default.size).then(|| Value::String(layout.size.as_str().into())),
    );
    put(
        WIDTH,
        (layout.width_mm != default.width_mm).then_some(Value::Float(layout.width_mm)),
    );
    put(
        HEIGHT,
        (layout.height_mm != default.height_mm).then_some(Value::Float(layout.height_mm)),
    );
    entry
}

/// The table a save writes: every note whose layout is not the default, and
/// the keys this build could not read for each note still on the sheet.
pub(in crate::format) fn encode(sheet: &SchematicSheet) -> BTreeMap<String, NoteLayoutEntry> {
    sheet
        .text_notes
        .iter()
        .filter_map(|note| {
            let id = note.uuid.to_string();
            let entry = entry_of(&note.rich, sheet.unknown.note_layouts.get(&id));
            (!entry.is_empty()).then_some((id, entry))
        })
        .collect()
}

/// The layouts read from the table, and per note the keys and words this
/// build does not know (kept for the next save). A width, height, border or
/// padding that is not a finite number at or above zero reads as unset.
pub(in crate::format) fn decode(
    table: BTreeMap<String, NoteLayoutEntry>,
) -> (
    BTreeMap<String, RichNoteLayout>,
    BTreeMap<String, UnknownFields>,
) {
    let mut known = BTreeMap::new();
    let mut unknown = BTreeMap::new();
    for (id, entry) in table {
        let mut kept = UnknownFields::default();
        let mut layout = RichNoteLayout::default();
        for (key, value) in entry {
            if !read_key(&mut layout, &key, &value) {
                kept.insert_value(&key, value);
            }
        }
        if !layout.is_default() {
            known.insert(id.clone(), layout);
        }
        if !kept.is_empty() {
            unknown.insert(id, kept);
        }
    }
    (known, unknown)
}

/// Read `key` into `layout`; false when this build does not know the key or
/// its value, which is then kept as read.
fn read_key(layout: &mut RichNoteLayout, key: &str, value: &Value) -> bool {
    if !KNOWN.contains(&key) {
        return false;
    }
    match key {
        FRAME => match value.as_str().and_then(NoteFrame::from_word) {
            Some(frame) => layout.frame = frame,
            None => return false,
        },
        SIZE => match value.as_str().and_then(NoteSize::from_word) {
            Some(size) => layout.size = size,
            None => return false,
        },
        _ => {
            // A length that is not a finite length reads as unset and is
            // not kept: the next save writes what the note holds.
            let Some(mm) = length(value) else {
                return true;
            };
            match key {
                BORDER => layout.border_mm = Some(mm),
                PADDING => layout.padding_mm = mm,
                WIDTH => layout.width_mm = mm,
                _ => layout.height_mm = mm,
            }
        }
    }
    true
}

fn length(value: &Value) -> Option<f64> {
    let mm = match value {
        Value::Float(mm) => *mm,
        Value::Integer(mm) => f64::from(i32::try_from(*mm).ok()?),
        _ => return None,
    };
    (mm.is_finite() && mm >= 0.0).then_some(mm)
}
