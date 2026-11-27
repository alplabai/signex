//! The pin mapping of `Command::ChangeComponent` given by hand (#2954): the
//! check of the mapping, each placed unit's share of it, the table that shows
//! it (old pin, new pin, net) and the carry of a connection to its new pin.
//!
//! A hand mapping is followed: what ends on an old pin's tip (wire ends,
//! labels, junctions, no-connect and No-ERC marks) moves to the tip of the
//! new pin it is mapped to, as a pin swap moves them (`crate::part_swap`).
//! Rules and refusals: `docs/CLI.md`, "Changing a part's component".

use std::collections::{HashMap, HashSet};

use signex_net::{ConnectedPoints, net_signatures, point_on_segment, pt_key};
use signex_types::schematic::{
    LibPin, PinMapping, Point, SchematicSheet, Symbol, SymbolTransform, map_pins, pin_key, pin_tips,
};
use uuid::Uuid;

use super::change_component::{ComponentReplacement, placed_units};
use super::pin_changes::{has_connection, shown_pins};
use crate::EngineError;

/// One old pin of a placed part, as the pin mapping shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinMapRow {
    /// The old pin's key ([`pin_key`]) and name.
    pub old: String,
    pub old_name: String,
    /// The new pin that takes it over, its key and name; `None` when none.
    pub new: Option<(String, String)>,
    /// The net on the old pin: `None` when nothing is connected to it, an
    /// empty name for a net no label or power port names.
    pub net: Option<String>,
}

/// A pin of the new part an old pin can be mapped to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinChoice {
    pub key: String,
    pub name: String,
}

/// One old pin's connection carried to the tip of its new pin.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Carry {
    pub pin: String,
    pub from: Point,
    pub to: Point,
}

fn refused(detail: String) -> EngineError {
    EngineError::ChangeComponentPins { detail }
}

/// The mapping of one placed unit: by number, then name ([`map_pins`])
/// without a hand mapping; else the hand pairs whose two pins this unit
/// shows, every other old pin unmatched.
pub(super) fn unit_mapping(
    old_pins: &[&LibPin],
    new_pins: &[&LibPin],
    hand: Option<&PinMapping>,
) -> PinMapping {
    let Some(hand) = hand else {
        return map_pins(old_pins, new_pins);
    };
    let mut mapping = PinMapping::default();
    for old in old_pins {
        let key = pin_key(old);
        let pair = hand
            .pairs
            .iter()
            .find(|p| p.old == key && new_pins.iter().any(|np| pin_key(np) == p.new));
        match pair {
            Some(pair) => mapping.pairs.push(pair.clone()),
            None => mapping.unmatched.push(key.to_string()),
        }
    }
    mapping
}

/// Refused when a hand pair names a pin the part does not show, a new pin
/// the unit of its old pin does not show, or a new pin taken twice.
pub(super) fn check_hand_map(
    doc: &SchematicSheet,
    units: &[&Symbol],
    replacement: &ComponentReplacement,
    hand: &PinMapping,
) -> Result<(), EngineError> {
    let drawing = &replacement.lib_symbol;
    let mut taken = HashSet::new();
    for pair in &hand.pairs {
        if !taken.insert(pair.new.as_str()) {
            return Err(refused(format!(
                "new pin {} is mapped from more than one pin",
                pair.new
            )));
        }
        let shows_old = |sym: &Symbol| {
            shown_pins(doc.body_of(sym), sym)
                .iter()
                .any(|p| pin_key(p) == pair.old)
        };
        if !units.iter().any(|sym| shows_old(sym)) {
            return Err(refused(format!("the part has no pin {}", pair.old)));
        }
        let pairs_up = units.iter().filter(|sym| shows_old(sym)).any(|sym| {
            shown_pins(Some(drawing), sym)
                .iter()
                .any(|p| pin_key(p) == pair.new)
        });
        if !pairs_up {
            return Err(refused(format!(
                "the new part has no pin {} where pin {} is",
                pair.new, pair.old
            )));
        }
    }
    Ok(())
}

/// The connections of `sym` its mapping carries: each connected old pin
/// whose new pin's tip is somewhere else.
pub(super) fn unit_carries(
    doc: &SchematicSheet,
    gate: &ConnectedPoints,
    sym: &Symbol,
    pins: (&[&LibPin], &[&LibPin]),
    mapping: &PinMapping,
) -> Vec<Carry> {
    let (old_pins, new_pins) = pins;
    let xform = SymbolTransform::from_symbol(sym);
    mapping
        .pairs
        .iter()
        .filter_map(|pair| {
            let old = old_pins.iter().find(|p| pin_key(p) == pair.old)?;
            let new = new_pins.iter().find(|p| pin_key(p) == pair.new)?;
            let from = xform.apply(old.pin.position);
            let to = xform.apply(new.pin.position);
            (pt_key(&from) != pt_key(&to) && has_connection(doc, gate, &from)).then(|| Carry {
                pin: pair.old.clone(),
                from,
                to,
            })
        })
        .collect()
}

/// Pin tips of every placed symbol on `sheet` but those of `part`.
fn other_pin_tips(sheet: &SchematicSheet, part: &[Uuid]) -> Vec<signex_net::uf::Key> {
    let mut tips = Vec::new();
    for symbol in sheet.symbols.iter().filter(|s| !part.contains(&s.uuid)) {
        if let Some(lib) = sheet.resolve_lib_symbol(symbol) {
            tips.extend(pin_tips(&lib, symbol).map(|(_, tip)| pt_key(&tip)));
        }
    }
    tips
}

/// Refused when a carried tip is the middle of a wire, another part's pin
/// tip or the end of a locked wire: none of them can be moved with the pin.
fn check_movable(
    sheet: &SchematicSheet,
    carries: &[Carry],
    others: &[signex_net::uf::Key],
    designator: &str,
) -> Result<(), String> {
    for carry in carries {
        let key = pt_key(&carry.from);
        let mid_wire = sheet.wires.iter().any(|w| {
            let (start, end) = (pt_key(&w.start), pt_key(&w.end));
            key != start && key != end && point_on_segment(key, start, end)
        });
        if mid_wire || others.contains(&key) {
            return Err(format!(
                "pin {} of {designator} meets the middle of a wire or another pin; rewire it by hand",
                carry.pin
            ));
        }
        let locked = sheet
            .wires
            .iter()
            .any(|w| w.locked && (pt_key(&w.start) == key || pt_key(&w.end) == key));
        if locked {
            return Err(format!(
                "a wire on pin {} of {designator} is locked",
                carry.pin
            ));
        }
    }
    Ok(())
}

/// What the carry moved: per collection, which items.
#[derive(Default)]
struct Moved {
    wires: Vec<Uuid>,
    labels: HashSet<Uuid>,
    junctions: HashSet<usize>,
    no_connects: HashSet<usize>,
    no_ercs: HashSet<usize>,
}

/// Whether something the carry did not move sits on `key`: it would join
/// the carried net.
fn stays_on(
    sheet: &SchematicSheet,
    moved: &Moved,
    others: &[signex_net::uf::Key],
    key: signex_net::uf::Key,
) -> bool {
    let wire = sheet.wires.iter().any(|w| {
        !moved.wires.contains(&w.uuid) && point_on_segment(key, pt_key(&w.start), pt_key(&w.end))
    });
    let label = sheet
        .labels
        .iter()
        .any(|l| !moved.labels.contains(&l.uuid) && pt_key(&l.position) == key);
    let at = |set: &HashSet<usize>, i: usize, p: &Point| !set.contains(&i) && pt_key(p) == key;
    let junction = sheet
        .junctions
        .iter()
        .enumerate()
        .any(|(i, j)| at(&moved.junctions, i, &j.position));
    let no_connect = sheet
        .no_connects
        .iter()
        .enumerate()
        .any(|(i, n)| at(&moved.no_connects, i, &n.position));
    let no_erc = sheet
        .no_erc_directives
        .iter()
        .enumerate()
        .any(|(i, d)| at(&moved.no_ercs, i, &d.position));
    let entry = sheet
        .child_sheets
        .iter()
        .flat_map(|c| c.pins.iter())
        .any(|p| pt_key(&p.position) == key);
    wire || label || junction || no_connect || no_erc || entry || others.contains(&key)
}

/// `sheet` (the part `part` already swapped) with what ends on each carry's
/// old tip moved to its new tip; `None` when nothing ended on any. Refused,
/// with why, when it cannot be moved or would join another net: the new tip
/// holds something that stays, or a moved wire would cross a pin tip, a
/// wire end, a label or a junction. A wire both of whose ends land on one
/// point is dropped.
pub(super) fn carry_connections(
    sheet: &SchematicSheet,
    part: &[Uuid],
    carries: &[Carry],
    designator: &str,
) -> Result<Option<SchematicSheet>, String> {
    let mut moves: HashMap<signex_net::uf::Key, Point> = HashMap::new();
    for carry in carries {
        let to = *moves.entry(pt_key(&carry.from)).or_insert(carry.to);
        if pt_key(&to) != pt_key(&carry.to) {
            return Err(format!(
                "pin {} of {designator} shares its tip with another pin; rewire it by hand",
                carry.pin
            ));
        }
    }
    let others = other_pin_tips(sheet, part);
    check_movable(sheet, carries, &others, designator)?;
    let mut out = sheet.clone();
    let carry = |p: &mut Point| match moves.get(&pt_key(p)) {
        Some(to) => {
            *p = *to;
            true
        }
        None => false,
    };
    let mut moved = Moved::default();
    for wire in &mut out.wires {
        let start = carry(&mut wire.start);
        if carry(&mut wire.end) || start {
            moved.wires.push(wire.uuid);
        }
    }
    for label in &mut out.labels {
        if carry(&mut label.position) {
            moved.labels.insert(label.uuid);
        }
    }
    for (i, junction) in out.junctions.iter_mut().enumerate() {
        if carry(&mut junction.position) {
            moved.junctions.insert(i);
        }
    }
    for (i, mark) in out.no_connects.iter_mut().enumerate() {
        if carry(&mut mark.position) {
            moved.no_connects.insert(i);
        }
    }
    for (i, mark) in out.no_erc_directives.iter_mut().enumerate() {
        if carry(&mut mark.position) {
            moved.no_ercs.insert(i);
        }
    }
    let nothing = moved.wires.is_empty()
        && moved.labels.is_empty()
        && moved.junctions.is_empty()
        && moved.no_connects.is_empty()
        && moved.no_ercs.is_empty();
    if nothing {
        return Ok(None);
    }
    let joins = moves
        .values()
        .any(|to| stays_on(&out, &moved, &others, pt_key(to)));
    if joins || crate::part_swap::joins_nets(&out, &moved.wires) {
        return Err(format!(
            "the carried wires of {designator} would join another net; rewire them by hand"
        ));
    }
    out.wires.retain(|w| pt_key(&w.start) != pt_key(&w.end));
    Ok(Some(out))
}

/// The pin mapping the swap of the part `symbol_id` for `replacement` uses
/// (the hand mapping when it has one): every old pin the placed units show,
/// each once, with its new pin and the net on it.
pub fn pin_table(
    doc: &SchematicSheet,
    symbol_id: Uuid,
    replacement: &ComponentReplacement,
) -> Vec<PinMapRow> {
    let nets = net_signatures(doc);
    let mut rows: Vec<PinMapRow> = Vec::new();
    for sym in placed_units(doc, symbol_id) {
        let old_pins = shown_pins(doc.body_of(sym), sym);
        let new_pins = shown_pins(Some(&replacement.lib_symbol), sym);
        let mapping = unit_mapping(&old_pins, &new_pins, replacement.pin_map.as_ref());
        for old in &old_pins {
            let key = pin_key(old);
            if rows.iter().any(|r| r.old == key) {
                continue;
            }
            let new = mapping.new_of(key).and_then(|new| {
                let pin = new_pins.iter().find(|np| pin_key(np) == new)?;
                Some((new.to_string(), pin.pin.name.clone()))
            });
            let terminal = (sym.uuid, key.to_string());
            let net = nets
                .iter()
                .find(|n| n.terminals.contains(&terminal))
                .map(|n| n.name.clone());
            rows.push(PinMapRow {
                old: key.to_string(),
                old_name: old.pin.name.clone(),
                new,
                net,
            });
        }
    }
    rows
}

/// The pins of the new part the placed units of `symbol_id` would show,
/// each once, in drawing order: what an old pin can be mapped to.
pub fn pin_choices(
    doc: &SchematicSheet,
    symbol_id: Uuid,
    replacement: &ComponentReplacement,
) -> Vec<PinChoice> {
    let mut choices: Vec<PinChoice> = Vec::new();
    for sym in placed_units(doc, symbol_id) {
        for pin in shown_pins(Some(&replacement.lib_symbol), sym) {
            let key = pin_key(pin);
            if !choices.iter().any(|c| c.key == key) {
                choices.push(PinChoice {
                    key: key.to_string(),
                    name: pin.pin.name.clone(),
                });
            }
        }
    }
    choices
}
