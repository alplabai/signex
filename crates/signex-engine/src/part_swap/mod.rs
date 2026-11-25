//! The schematic half of a pin or gate swap made on the board (#2410). Pure:
//! each function reads a sheet and returns the sheet as it should be, which
//! the caller installs with `Command::ReplaceDocument` as the sheet's step of
//! the swap's undo group (the board half is `crate::pcb::pad_swap`).
//!
//! - A gate swap trades the unit numbers of two placed units of one part.
//!   Equivalent gates draw the same pins in the same places, so every wire
//!   stays on its pin and each net moves to the other gate's pin.
//! - A pin swap moves what ends on one pin's tip (wire ends, labels, no
//!   connect marks, junctions) to the other's, and back. It is refused when
//!   a moved wire would then cross a pin tip or another wire's end, which
//!   would join nets the swap does not mean to join, and when a pin tip
//!   meets something it cannot move (the middle of a wire, another pin).
//!
//! Rules and limits: `docs/PCB_TOOLS.md`, "Pin and gate swap".

use signex_net::{point_on_segment, pt_key};
use signex_types::schematic::{LibSymbol, Point, SchematicSheet, pin_tips};
use uuid::Uuid;

/// Why the schematic half of a swap cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SheetSwapRefusal {
    #[error("the symbol {0} is not on the sheet")]
    NoSymbol(Uuid),
    #[error("{designator} has no pin {pin} on the sheet")]
    NoPin { designator: String, pin: String },
    #[error("pins {a} and {b} of {designator} sit on one point")]
    SamePoint {
        designator: String,
        a: String,
        b: String,
    },
    #[error(
        "pin {pin} of {designator} meets the middle of a wire or another pin; rewire it by hand"
    )]
    Unmovable { designator: String, pin: String },
    #[error("a wire on pin {pin} of {designator} is locked")]
    LockedWire { designator: String, pin: String },
    #[error(
        "the rewired wires of {designator} would cross a pin or a wire end and join nets; rewire them by hand"
    )]
    WouldJoin { designator: String },
    #[error("{0} is locked")]
    SymbolLocked(String),
    #[error("units {a} and {b} of {designator} do not draw the same pins in the same places")]
    GatesDiffer { designator: String, a: u32, b: u32 },
}

/// One pin of a placed unit: the unit's symbol and the pin's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedPin<'a> {
    pub symbol: Uuid,
    pub number: &'a str,
}

/// The sheet with what ends on pin `a`'s tip moved to pin `b`'s tip and the
/// other way round; `None` when nothing ends on either.
pub fn rewire_pins(
    sheet: &SchematicSheet,
    a: PlacedPin<'_>,
    b: PlacedPin<'_>,
) -> Result<Option<SchematicSheet>, SheetSwapRefusal> {
    let tip_a = pin_tip(sheet, a)?;
    let tip_b = pin_tip(sheet, b)?;
    let designator = designator_of(sheet, a.symbol);
    let (key_a, key_b) = (pt_key(&tip_a), pt_key(&tip_b));
    if key_a == key_b {
        return Err(SheetSwapRefusal::SamePoint {
            designator,
            a: a.number.to_string(),
            b: b.number.to_string(),
        });
    }
    for (pin, key) in [(a, key_a), (b, key_b)] {
        check_movable(sheet, pin, key, [a, b], &designator)?;
    }
    let mut out = sheet.clone();
    // Whether `p` was on a tip and moved to the other.
    let swap = |p: &mut Point| {
        let key = pt_key(p);
        let to = if key == key_a {
            tip_b
        } else if key == key_b {
            tip_a
        } else {
            return false;
        };
        *p = to;
        true
    };
    let mut moved_wires = Vec::new();
    for wire in &mut out.wires {
        let (start, end) = (pt_key(&wire.start), pt_key(&wire.end));
        let touches = [start, end].iter().any(|k| *k == key_a || *k == key_b);
        if !touches {
            continue;
        }
        if wire.locked {
            let pin = if start == key_a || end == key_a { a } else { b };
            return Err(SheetSwapRefusal::LockedWire {
                designator,
                pin: pin.number.to_string(),
            });
        }
        swap(&mut wire.start);
        swap(&mut wire.end);
        moved_wires.push(wire.uuid);
    }
    let mut moved = !moved_wires.is_empty();
    for label in &mut out.labels {
        moved |= swap(&mut label.position);
    }
    for junction in &mut out.junctions {
        moved |= swap(&mut junction.position);
    }
    for mark in out
        .no_connects
        .iter_mut()
        .map(|n| &mut n.position)
        .chain(out.no_erc_directives.iter_mut().map(|d| &mut d.position))
    {
        moved |= swap(mark);
    }
    if !moved {
        return Ok(None);
    }
    if joins_nets(&out, &moved_wires) {
        return Err(SheetSwapRefusal::WouldJoin { designator });
    }
    Ok(Some(out))
}

/// The sheet with each `(symbol, unit)` placed as that unit. Refused for a
/// locked symbol or one not on the sheet.
pub fn swap_units(
    sheet: &SchematicSheet,
    units: &[(Uuid, u32)],
) -> Result<SchematicSheet, SheetSwapRefusal> {
    let mut out = sheet.clone();
    for (id, unit) in units {
        let symbol = out
            .symbols
            .iter_mut()
            .find(|s| s.uuid == *id)
            .ok_or(SheetSwapRefusal::NoSymbol(*id))?;
        if symbol.locked {
            return Err(SheetSwapRefusal::SymbolLocked(symbol.reference.clone()));
        }
        symbol.unit = *unit;
    }
    Ok(out)
}

/// The pin pairs a swap of units `a` and `b` of `lib` trades: each pin of
/// unit `a` with the pin of unit `b` drawn at the same place. Pins every
/// unit shares (unit 0) are not traded. Refused when the units differ in
/// their pin count or in where a pin is drawn.
pub fn gate_pin_pairs(
    lib: &LibSymbol,
    designator: &str,
    a: u32,
    b: u32,
) -> Result<Vec<(String, String)>, SheetSwapRefusal> {
    let differ = || SheetSwapRefusal::GatesDiffer {
        designator: designator.to_string(),
        a,
        b,
    };
    let pins_of = |unit: u32| {
        lib.pins
            .iter()
            .filter(move |lp| lp.unit == unit && lp.body_style == 1)
            .collect::<Vec<_>>()
    };
    let (pins_a, pins_b) = (pins_of(a), pins_of(b));
    if a == b || pins_a.is_empty() || pins_a.len() != pins_b.len() {
        return Err(differ());
    }
    let mut pairs = Vec::with_capacity(pins_a.len());
    for pin in &pins_a {
        let at = pt_key(&pin.pin.position);
        let twin = pins_b
            .iter()
            .find(|other| pt_key(&other.pin.position) == at)
            .ok_or_else(differ)?;
        pairs.push((pin.pin.number.clone(), twin.pin.number.clone()));
    }
    Ok(pairs)
}

/// The unit `number` is a pin of in `lib` (its normal body); 0 for a pin
/// every unit shares.
pub fn unit_of_pin(lib: &LibSymbol, number: &str) -> Option<u32> {
    lib.pins
        .iter()
        .find(|lp| lp.body_style == 1 && lp.pin.number == number)
        .map(|lp| lp.unit)
}

/// The world tip of `pin`.
fn pin_tip(sheet: &SchematicSheet, pin: PlacedPin<'_>) -> Result<Point, SheetSwapRefusal> {
    let symbol = sheet
        .symbols
        .iter()
        .find(|s| s.uuid == pin.symbol)
        .ok_or(SheetSwapRefusal::NoSymbol(pin.symbol))?;
    let lib = sheet.resolve_lib_symbol(symbol);
    lib.as_deref()
        .and_then(|lib| {
            pin_tips(lib, symbol)
                .find(|(lp, _)| lp.pin.number == pin.number)
                .map(|(_, tip)| tip)
        })
        .ok_or_else(|| SheetSwapRefusal::NoPin {
            designator: symbol.reference.clone(),
            pin: pin.number.to_string(),
        })
}

fn designator_of(sheet: &SchematicSheet, symbol: Uuid) -> String {
    sheet
        .symbols
        .iter()
        .find(|s| s.uuid == symbol)
        .map(|s| s.reference.clone())
        .unwrap_or_default()
}

/// Refused when `key` (the tip of `pin`) is the middle of a wire or the tip
/// of a pin other than the two swapped: neither can be moved with it.
fn check_movable(
    sheet: &SchematicSheet,
    pin: PlacedPin<'_>,
    key: signex_net::uf::Key,
    swapped: [PlacedPin<'_>; 2],
    designator: &str,
) -> Result<(), SheetSwapRefusal> {
    let unmovable = || SheetSwapRefusal::Unmovable {
        designator: designator.to_string(),
        pin: pin.number.to_string(),
    };
    for wire in &sheet.wires {
        let (start, end) = (pt_key(&wire.start), pt_key(&wire.end));
        if key != start && key != end && point_on_segment(key, start, end) {
            return Err(unmovable());
        }
    }
    for symbol in &sheet.symbols {
        let Some(lib) = sheet.resolve_lib_symbol(symbol) else {
            continue;
        };
        for (lp, tip) in pin_tips(&lib, symbol) {
            let is_swapped = swapped
                .iter()
                .any(|p| p.symbol == symbol.uuid && p.number == lp.pin.number);
            if !is_swapped && pt_key(&tip) == key {
                return Err(unmovable());
            }
        }
    }
    Ok(())
}

/// Whether a moved wire now runs through a pin tip, a wire end, a label or
/// a junction it does not end on: the netlist would join it there. Change
/// Component's pin carry asks the same (`exec::change_component_pins`).
pub(crate) fn joins_nets(sheet: &SchematicSheet, moved: &[Uuid]) -> bool {
    let mut points: Vec<signex_net::uf::Key> = Vec::new();
    for symbol in &sheet.symbols {
        if let Some(lib) = sheet.resolve_lib_symbol(symbol) {
            points.extend(pin_tips(&lib, symbol).map(|(_, tip)| pt_key(&tip)));
        }
    }
    points.extend(
        sheet
            .wires
            .iter()
            .flat_map(|w| [pt_key(&w.start), pt_key(&w.end)]),
    );
    points.extend(sheet.labels.iter().map(|l| pt_key(&l.position)));
    points.extend(sheet.junctions.iter().map(|j| pt_key(&j.position)));
    sheet
        .wires
        .iter()
        .filter(|w| moved.contains(&w.uuid))
        .any(|w| {
            let (start, end) = (pt_key(&w.start), pt_key(&w.end));
            points
                .iter()
                .any(|p| *p != start && *p != end && point_on_segment(*p, start, end))
        })
}

#[cfg(test)]
mod tests;
