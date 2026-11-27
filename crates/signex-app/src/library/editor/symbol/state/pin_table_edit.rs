//! The Pin table tab's edits of the symbol (mockup row 122b): a new row,
//! Delete selected, Apply to selected and Apply paste. Each runs on the
//! symbol as the caller's one undo step.

use std::collections::BTreeSet;

use signex_library::{PinDirection, PinOrientation, Symbol, SymbolPin};

use super::pin_table_ui::{BulkEdit, PasteMode, PastedRow};
use super::{DEFAULT_PIN_LENGTH_MM, PartView, SymbolSelection, add_pin};

/// Drop a selection that names pins by index: after a delete or a paste
/// the indices may name other pins.
pub fn drop_pin_selection(selected: &mut Option<SymbolSelection>) {
    if matches!(
        selected,
        Some(SymbolSelection::Pin(_) | SymbolSelection::Multiple { .. } | SymbolSelection::All)
    ) {
        *selected = None;
    }
}

/// The pitch of rows added from the table, one grid step below the lowest
/// pin.
const ROW_PITCH_MM: f64 = 2.54;

/// Where a pin added from the table goes: one pitch below the lowest pin,
/// in its column; the origin for a symbol with no pins.
fn next_row_position(sym: &Symbol) -> [f64; 2] {
    sym.pins
        .iter()
        .map(|p| p.position)
        .min_by(|a, b| a[1].total_cmp(&b[1]))
        .map_or([0.0, 0.0], |[x, y]| [x, y - ROW_PITCH_MM])
}

/// `+ Pin` in the table: a pin as the Add Pin tool places it (the next
/// number, the shown part and body), below the lowest pin. Its index.
pub fn add_table_pin(sym: &mut Symbol, view: PartView) -> usize {
    let [x, y] = next_row_position(sym);
    add_pin(sym, x, y, view)
}

/// Delete the pins at `rows`. Whether any went.
pub fn remove_pins(sym: &mut Symbol, rows: &BTreeSet<usize>) -> bool {
    let before = sym.pins.len();
    let mut idx = 0;
    sym.pins.retain(|_| {
        let keep = !rows.contains(&idx);
        idx += 1;
        keep
    });
    sym.pins.len() != before
}

/// Write the set fields of `bulk` to the pins at `rows`; `length` is the
/// bulk length already checked by the caller. A part that is not a whole
/// number keeps each pin's part.
pub fn apply_bulk(
    sym: &mut Symbol,
    rows: &BTreeSet<usize>,
    bulk: &BulkEdit,
    length: Option<f64>,
) -> bool {
    let part = bulk.part.trim().parse::<u8>().ok();
    let mut touched = false;
    for idx in rows {
        let Some(pin) = sym.pins.get_mut(*idx) else {
            continue;
        };
        touched = true;
        if let Some(electrical) = bulk.electrical {
            pin.electrical = electrical;
        }
        if let Some(part) = part {
            pin.part_number = part;
        }
        if let Some(length) = length {
            pin.length = length;
        }
        if let Some(orientation) = bulk.orientation {
            pin.orientation = orientation;
        }
        if let Some(hidden) = bulk.hidden {
            pin.hidden = hidden;
        }
    }
    touched
}

/// Apply pasted `rows` in `mode`. A new pin takes the row's fields over
/// the table's defaults (passive, part 1, pointing left) and goes one row
/// below the lowest pin. `length_ok` checks a pasted length; a refused one
/// keeps the pin's length.
pub fn apply_paste(
    sym: &mut Symbol,
    rows: &[PastedRow],
    mode: PasteMode,
    view: PartView,
    length_ok: impl Fn(f64) -> bool,
) -> bool {
    if rows.is_empty() {
        return false;
    }
    if mode == PasteMode::ReplaceAll {
        sym.pins.clear();
    }
    for row in rows {
        let existing = match (mode, row.number.as_deref()) {
            (PasteMode::UpdateByNumber, Some(n)) if !n.is_empty() => {
                sym.pins.iter().position(|p| p.number == n)
            }
            _ => None,
        };
        let idx = existing.unwrap_or_else(|| new_pasted_pin(sym, view));
        if let Some(pin) = sym.pins.get_mut(idx) {
            write_row(pin, row, &length_ok);
        }
    }
    true
}

fn new_pasted_pin(sym: &mut Symbol, view: PartView) -> usize {
    let idx = add_table_pin(sym, view);
    if let Some(pin) = sym.pins.get_mut(idx) {
        pin.name.clear();
        pin.electrical = PinDirection::Passive;
        pin.part_number = 1;
        pin.orientation = PinOrientation::Left;
        pin.length = DEFAULT_PIN_LENGTH_MM;
    }
    idx
}

fn write_row(pin: &mut SymbolPin, row: &PastedRow, length_ok: &impl Fn(f64) -> bool) {
    if let Some(number) = &row.number {
        pin.number.clone_from(number);
    }
    if let Some(name) = &row.name {
        pin.name.clone_from(name);
    }
    if let Some(electrical) = row.electrical {
        pin.electrical = electrical;
    }
    if let Some(part) = row.part {
        pin.part_number = part;
    }
    if let Some(length) = row.length.filter(|l| length_ok(*l)) {
        pin.length = length;
    }
    if let Some(description) = &row.description {
        pin.description.clone_from(description);
    }
}

#[cfg(test)]
#[path = "pin_table_edit_tests.rs"]
mod tests;
