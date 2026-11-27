//! The rows `edit_symbol_pins` sets (mockup row 122b, #2817): each names a
//! pin by its number and gives the fields to write, or removes the pin. The
//! Pin table's bulk edit and Paste rows for a caller with no table. Reading
//! and applying are pure, so the command applies exactly what was read and
//! refuses the whole edit before it touches the symbol.

use std::collections::BTreeSet;

use serde_json::{Map, Value};
use signex_library::{PinDirection, PinOrientation, Symbol, SymbolPin};

use super::pin_table_edit::remove_pins;
use super::pin_table_ui::{PasteField, PasteMode};

/// The fields a row may set, in the order an error lists them.
pub const ROW_FIELDS: [&str; 8] = [
    "number",
    "name",
    "electrical",
    "part",
    "length",
    "orientation",
    "hidden",
    "description",
];

/// The orientation ids, as Properties names the four sides.
const ORIENTATIONS: [(PinOrientation, &str); 4] = [
    (PinOrientation::Left, "left"),
    (PinOrientation::Right, "right"),
    (PinOrientation::Up, "up"),
    (PinOrientation::Down, "down"),
];

/// The paste column ids, for `fields`.
const PASTE_FIELDS: [(PasteField, &str); 7] = [
    (PasteField::Number, "number"),
    (PasteField::Name, "name"),
    (PasteField::Electrical, "electrical"),
    (PasteField::Part, "part"),
    (PasteField::Length, "length"),
    (PasteField::Description, "description"),
    (PasteField::Ignore, "ignore"),
];

/// The paste mode ids, for `mode`.
const PASTE_MODES: [(PasteMode, &str); 3] = [
    (PasteMode::UpdateByNumber, "update_by_number"),
    (PasteMode::Append, "append"),
    (PasteMode::ReplaceAll, "replace_all"),
];

/// What one row writes; a field left out keeps the pin's value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PinFields {
    pub number: Option<String>,
    pub name: Option<String>,
    pub electrical: Option<PinDirection>,
    pub part: Option<u8>,
    pub length: Option<f64>,
    pub orientation: Option<PinOrientation>,
    pub hidden: Option<bool>,
    pub description: Option<String>,
}

/// What a row does to its pin.
#[derive(Debug, Clone, PartialEq)]
pub enum PinRowChange {
    Set(PinFields),
    Remove,
}

/// One row: the number the pin has before the edit, and the change.
#[derive(Debug, Clone, PartialEq)]
pub struct PinRow {
    pub number: String,
    pub change: PinRowChange,
}

/// What [`apply_rows`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowsApplied {
    pub updated: usize,
    /// The removed pins, by their index before the edit.
    pub removed: BTreeSet<usize>,
}

/// The id an answer gives `electrical`, the one `query_pins` answers with.
pub fn electrical_id(electrical: PinDirection) -> String {
    serde_json::to_value(electrical)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn electrical_of(id: &str) -> Option<PinDirection> {
    PinDirection::ALL
        .into_iter()
        .find(|d| electrical_id(*d) == id.trim())
}

/// The id an answer gives `orientation`.
pub fn orientation_id(orientation: PinOrientation) -> &'static str {
    ORIENTATIONS
        .iter()
        .find(|(o, _)| *o == orientation)
        .map_or("right", |(_, id)| *id)
}

fn orientation_of(id: &str) -> Option<PinOrientation> {
    ORIENTATIONS
        .iter()
        .find(|(_, name)| *name == id.trim())
        .map(|(o, _)| *o)
}

fn ids_of<T>(table: &[(T, &'static str)]) -> String {
    let names: Vec<&str> = table.iter().map(|(_, id)| *id).collect();
    names.join(", ")
}

/// `text` read as rows: a JSON object of pin number to the fields to set,
/// or to `null` to remove the pin. `length_ok` says whether a length can be
/// stored; one it refuses refuses the whole text.
pub fn parse_pin_rows(text: &str, length_ok: impl Fn(f64) -> bool) -> Result<Vec<PinRow>, String> {
    const SHAPE: &str = "`pins` must be a JSON object of pin number to the fields to set, or to null to remove the pin";
    let Ok(Value::Object(rows)) = serde_json::from_str::<Value>(text) else {
        return Err(SHAPE.to_string());
    };
    if rows.is_empty() {
        return Err(format!("{SHAPE}; it names no pin"));
    }
    rows.into_iter()
        .map(|(number, value)| {
            let change = match value {
                Value::Null => PinRowChange::Remove,
                Value::Object(fields) => PinRowChange::Set(
                    read_fields(&fields, &length_ok)
                        .map_err(|why| format!("pin `{number}`: {why}"))?,
                ),
                _ => return Err(format!("pin `{number}`: {SHAPE}")),
            };
            Ok(PinRow { number, change })
        })
        .collect()
}

fn read_fields(
    fields: &Map<String, Value>,
    length_ok: &impl Fn(f64) -> bool,
) -> Result<PinFields, String> {
    let mut out = PinFields::default();
    for (field, value) in fields {
        let text = || {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("`{field}` is text"))
        };
        match field.as_str() {
            "number" => {
                let number = text()?;
                if number.trim().is_empty() {
                    return Err("`number` must not be empty".into());
                }
                out.number = Some(number);
            }
            "name" => out.name = Some(text()?),
            "description" => out.description = Some(text()?),
            "electrical" => {
                let id = text()?;
                out.electrical = Some(electrical_of(&id).ok_or_else(|| {
                    let ids: Vec<String> =
                        PinDirection::ALL.into_iter().map(electrical_id).collect();
                    format!(
                        "`electrical` `{id}` is not a type (one of {})",
                        ids.join(", ")
                    )
                })?);
            }
            "orientation" => {
                let id = text()?;
                out.orientation =
                    Some(orientation_of(&id).ok_or_else(|| {
                        format!("`orientation` is one of {}", ids_of(&ORIENTATIONS))
                    })?);
            }
            "part" => {
                let part = value
                    .as_u64()
                    .and_then(|p| u8::try_from(p).ok())
                    .ok_or("`part` is a whole number from 0 to 255")?;
                out.part = Some(part);
            }
            "length" => {
                let length = value
                    .as_f64()
                    .filter(|l| l.is_finite() && length_ok(*l))
                    .ok_or("`length` is not a length a pin can have")?;
                out.length = Some(length);
            }
            "hidden" => out.hidden = Some(value.as_bool().ok_or("`hidden` is true or false")?),
            other => {
                return Err(format!(
                    "`{other}` is not a pin field (one of {})",
                    ROW_FIELDS.join(", ")
                ));
            }
        }
    }
    Ok(out)
}

/// The index of the one pin numbered `number`; refused when none has it or
/// several do.
fn pin_numbered(sym: &Symbol, number: &str) -> Result<usize, String> {
    let mut found = sym
        .pins
        .iter()
        .enumerate()
        .filter(|(_, pin)| pin.number == number)
        .map(|(idx, _)| idx);
    let first = found
        .next()
        .ok_or_else(|| format!("no pin is numbered `{number}`"))?;
    if found.next().is_some() {
        return Err(format!(
            "more than one pin is numbered `{number}`; give them their own numbers first"
        ));
    }
    Ok(first)
}

/// Apply `rows` to `sym`. Every row's pin is found by the number it has
/// before the edit, so a renumbering of one pin does not move another row;
/// a row that names no pin, or a number two pins share, refuses the whole
/// edit before anything is written.
pub fn apply_rows(sym: &mut Symbol, rows: &[PinRow]) -> Result<RowsApplied, String> {
    let found: Vec<(usize, &PinRowChange)> = rows
        .iter()
        .map(|row| pin_numbered(sym, &row.number).map(|idx| (idx, &row.change)))
        .collect::<Result<_, String>>()?;
    let mut applied = RowsApplied::default();
    for (idx, change) in found {
        match change {
            PinRowChange::Remove => {
                applied.removed.insert(idx);
            }
            PinRowChange::Set(fields) => {
                if let Some(pin) = sym.pins.get_mut(idx) {
                    write_fields(pin, fields);
                    applied.updated += 1;
                }
            }
        }
    }
    remove_pins(sym, &applied.removed);
    Ok(applied)
}

fn write_fields(pin: &mut SymbolPin, fields: &PinFields) {
    if let Some(number) = &fields.number {
        pin.number.clone_from(number);
    }
    if let Some(name) = &fields.name {
        pin.name.clone_from(name);
    }
    if let Some(electrical) = fields.electrical {
        pin.electrical = electrical;
    }
    if let Some(part) = fields.part {
        pin.part_number = part;
    }
    if let Some(length) = fields.length {
        pin.length = length;
    }
    if let Some(orientation) = fields.orientation {
        pin.orientation = orientation;
    }
    if let Some(hidden) = fields.hidden {
        pin.hidden = hidden;
    }
    if let Some(description) = &fields.description {
        pin.description.clone_from(description);
    }
}

/// The column map `fields` names, comma separated (`number,name,ignore`).
pub fn parse_paste_fields(text: &str) -> Result<Vec<PasteField>, String> {
    text.split(',')
        .map(|id| {
            PASTE_FIELDS
                .iter()
                .find(|(_, name)| *name == id.trim())
                .map(|(field, _)| *field)
                .ok_or_else(|| {
                    format!(
                        "`{}` is not a paste column (one of {})",
                        id.trim(),
                        ids_of(&PASTE_FIELDS)
                    )
                })
        })
        .collect()
}

/// The paste mode `id` names.
pub fn parse_paste_mode(id: &str) -> Result<PasteMode, String> {
    PASTE_MODES
        .iter()
        .find(|(_, name)| *name == id.trim())
        .map(|(mode, _)| *mode)
        .ok_or_else(|| format!("`mode` is one of {}", ids_of(&PASTE_MODES)))
}

/// Pasted text as an operand carries it: a control character cannot travel
/// in one, so `\n` stands for a line break, `\t` for a tab and `\\` for a
/// backslash. Any other backslash is kept.
pub fn unescape_paste(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
#[path = "pin_rows_tests.rs"]
mod tests;
