//! Repair of repeated object uuids on a freshly parsed sheet (#1830).
//!
//! Selection, undo, cross-probe and the BOM address objects by uuid, and
//! the `[extras.*]` tables are keyed by it, so two objects sharing one id
//! are ambiguous and the later one's extras overwrite the earlier's on
//! save. Every uuid-carrying object kind shares ONE id space per sheet
//! (the sheet's own uuid is part of it):
//!
//! - a later object whose id was already taken by an identical earlier
//!   object of the same kind is a copy-paste duplicate: it is dropped;
//! - otherwise it is a different object that collided: it keeps its
//!   content and gets a new uuid.
//!
//! The new uuid is derived from the sheet uuid, the kind, the row and the
//! old id, so every parse of the same file agrees (ERC findings, waivers and
//! navigation made from a separate read name the ids the open tab has).
//! The first holder of an id is never changed.
//!
//! A symbol's per-pin ids (`pin_uuids`) are scoped to the symbol, not the
//! sheet: nothing looks a pin up by that id alone, and pasting or placing a
//! copy of a symbol legitimately repeats them on another symbol.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::load_repair::{RepairedObject, SHEET_SYMBOL, SheetLoadRepair};
use crate::schematic::SchematicSheet;

/// The ids taken so far and the repairs made so far.
struct Repairs {
    sheet: Uuid,
    seen: HashSet<Uuid>,
    made: Vec<SheetLoadRepair>,
}

impl Repairs {
    /// Make the ids in `items` unique; `name` says what a repaired object is
    /// called in the repair record.
    fn check<T: PartialEq>(
        &mut self,
        items: &mut Vec<T>,
        kind: &'static str,
        owner: Option<&str>,
        uuid: fn(&T) -> Uuid,
        set_uuid: fn(&mut T, Uuid),
        name: fn(&T) -> String,
    ) {
        let mut kept: Vec<T> = Vec::with_capacity(items.len());
        // Index in `kept` of the object that first held each id, so a
        // repeat is compared with that one object, not with every earlier one.
        let mut holders: HashMap<Uuid, usize> = HashMap::new();
        for (index, mut item) in std::mem::take(items).into_iter().enumerate() {
            let id = uuid(&item);
            if self.seen.insert(id) {
                holders.insert(id, kept.len());
                kept.push(item);
                continue;
            }
            let object = RepairedObject {
                kind,
                name: name(&item),
                position: index + 1,
            };
            if holders.get(&id).is_some_and(|&held| kept[held] == item) {
                self.made
                    .push(SheetLoadRepair::CopyDropped { object, uuid: id });
                continue;
            }
            let new = self.fresh_uuid(kind, index + 1, id, "");
            set_uuid(&mut item, new);
            self.made.push(SheetLoadRepair::UuidReminted {
                object,
                old: id,
                new,
                owner: owner.map(str::to_string),
            });
            kept.push(item);
        }
        *items = kept;
    }

    /// A uuid not yet taken, derived from the file's content (see the module
    /// doc). `scope` tells repairs of the same row apart.
    fn fresh_uuid(&mut self, kind: &str, row: usize, old: Uuid, scope: &str) -> Uuid {
        for attempt in 0u32.. {
            let candidate = derived_uuid(&format!(
                "{}|{kind}|{row}|{old}|{scope}|{attempt}",
                self.sheet
            ));
            if self.seen.insert(candidate) {
                return candidate;
            }
        }
        unreachable!("the attempt counter outlasts any sheet")
    }
}

/// A version-8 UUID holding the FNV-1a 128-bit hash of `name`.
fn derived_uuid(name: &str) -> Uuid {
    let mut hash: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for byte in name.bytes() {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
    }
    let version = 0x8u128 << 76;
    let variant = 0x2u128 << 62;
    Uuid::from_u128((hash & !(0xFu128 << 76) & !(0x3u128 << 62)) | version | variant)
}

/// Make every object uuid on `sheet` unique, returning what was repaired
/// (empty when nothing repeated).
pub(in crate::format) fn repair_duplicate_uuids(
    sheet: &mut SchematicSheet,
) -> Vec<SheetLoadRepair> {
    let mut r = Repairs {
        sheet: sheet.uuid,
        seen: HashSet::from([sheet.uuid]),
        made: Vec::new(),
    };
    r.check(
        &mut sheet.symbols,
        "symbol",
        None,
        |s| s.uuid,
        |s, u| s.uuid = u,
        |s| s.reference.clone(),
    );
    r.check(
        &mut sheet.wires,
        "wire",
        None,
        |w| w.uuid,
        |w, u| w.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.junctions,
        "junction",
        None,
        |j| j.uuid,
        |j, u| j.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.labels,
        "label",
        None,
        |l| l.uuid,
        |l, u| l.uuid = u,
        |l| l.text.clone(),
    );
    r.check(
        &mut sheet.child_sheets,
        SHEET_SYMBOL,
        None,
        |c| c.uuid,
        |c, u| c.uuid = u,
        |c| c.name.clone(),
    );
    r.check(
        &mut sheet.no_connects,
        "no-connect",
        None,
        |n| n.uuid,
        |n, u| n.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.text_notes,
        "text note",
        None,
        |t| t.uuid,
        |t, u| t.uuid = u,
        |t| t.text.clone(),
    );
    r.check(
        &mut sheet.buses,
        "bus",
        None,
        |b| b.uuid,
        |b, u| b.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.bus_entries,
        "bus entry",
        None,
        |b| b.uuid,
        |b, u| b.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.drawings,
        "drawing",
        None,
        |d| d.uuid(),
        |d, u| d.set_uuid(u),
        |_| String::new(),
    );
    r.check(
        &mut sheet.no_erc_directives,
        "no-ERC directive",
        None,
        |n| n.uuid,
        |n, u| n.uuid = u,
        |_| String::new(),
    );
    for child in &mut sheet.child_sheets {
        let owner = child.name.clone();
        r.check(
            &mut child.pins,
            "sheet pin",
            Some(&owner),
            |p| p.uuid,
            |p, u| p.uuid = u,
            |p| p.name.clone(),
        );
    }
    for symbol in &mut sheet.symbols {
        repair_pin_uuids(symbol, &mut r);
    }
    move_side_map_entries(sheet, &r.made);
    r.made
}

/// Maps keyed by object uuid (`net_colors` for wires and junctions,
/// `drawing_placements` for drawings): a re-minted object takes a copy of its
/// old id's entry, so it keeps the colour or placement it was read with.
fn move_side_map_entries(sheet: &mut SchematicSheet, repairs: &[SheetLoadRepair]) {
    for repair in repairs {
        let SheetLoadRepair::UuidReminted {
            object, old, new, ..
        } = repair
        else {
            continue;
        };
        match object.kind {
            "wire" | "junction" => {
                if let Some(color) = sheet.net_colors.get(old).copied() {
                    sheet.net_colors.insert(*new, color);
                }
            }
            "drawing" => {
                if let Some(placement) = sheet.drawing_placements.get(old).copied() {
                    sheet.drawing_placements.insert(*new, placement);
                }
            }
            _ => {}
        }
    }
}

/// A symbol's per-pin ids (`pin_uuids`, by pin number): two pins of one
/// symbol must not share one. The pin cannot be dropped, so the later pin
/// (in lexicographic pin-number order) is always re-minted.
fn repair_pin_uuids(symbol: &mut crate::schematic::Symbol, r: &mut Repairs) {
    let mut numbers: Vec<String> = symbol.pin_uuids.keys().cloned().collect();
    numbers.sort();
    let mut own: HashSet<Uuid> = HashSet::new();
    for number in numbers {
        let old = symbol.pin_uuids[&number];
        if own.insert(old) {
            continue;
        }
        let scope = format!("{}|{number}", symbol.uuid);
        let new = loop {
            let candidate = r.fresh_uuid("pin", 0, old, &scope);
            if own.insert(candidate) {
                break candidate;
            }
        };
        symbol.pin_uuids.insert(number.clone(), new);
        r.made.push(SheetLoadRepair::PinUuidReminted {
            symbol: symbol.reference.clone(),
            pin: number,
            old,
            new,
        });
    }
}
