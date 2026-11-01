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
//! The new uuid is derived from the sheet uuid, the kind, the position and
//! the old id, so every parse of the same file agrees (ERC findings,
//! waivers and navigation made from a separate read name the ids the open
//! tab has). The first holder of an id is never changed.
//!
//! A symbol's per-pin ids (`pin_uuids`) are scoped to the symbol, not the
//! sheet: nothing looks a pin up by that id alone, and pasting or placing a
//! copy of a symbol legitimately repeats them on another symbol.
//!
//! [`IdSpace`] and [`SheetLoadRepair`] know nothing about sheets beyond the
//! kind names; another document (a board) builds its own wiring on them.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::load_repair::{RepairedObject, SheetLoadRepair, kind};
use crate::schematic::SchematicSheet;

/// The ids taken in one document, who first took each, and the repairs
/// made so far.
pub(in crate::format) struct IdSpace {
    root: Uuid,
    taken: HashMap<Uuid, &'static str>,
    made: Vec<SheetLoadRepair>,
}

impl IdSpace {
    /// A document whose own id is `root`: no object may take it.
    pub(in crate::format) fn new(root: Uuid) -> Self {
        Self {
            root,
            taken: HashMap::from([(root, kind::SHEET)]),
            made: Vec::new(),
        }
    }

    pub(in crate::format) fn into_repairs(self) -> Vec<SheetLoadRepair> {
        self.made
    }

    /// Make the ids in `items` (all of one `kind`) unique. `owner` names the
    /// object they belong to, `name` what a repaired item is called.
    pub(in crate::format) fn check<T: PartialEq>(
        &mut self,
        items: &mut Vec<T>,
        kind: &'static str,
        owner: Option<&str>,
        access: Access<T>,
    ) {
        let mut kept: Vec<T> = Vec::with_capacity(items.len());
        // Index in `kept` of the object that first held each id, so a
        // repeat is compared with that one object, not with every earlier one.
        let mut holders: HashMap<Uuid, usize> = HashMap::new();
        for (index, mut item) in std::mem::take(items).into_iter().enumerate() {
            let id = (access.uuid)(&item);
            if let Some(&holder) = self.taken.get(&id) {
                let object = RepairedObject {
                    kind,
                    name: (access.name)(&item),
                    position: index + 1,
                };
                if holders.get(&id).is_some_and(|&held| kept[held] == item) {
                    self.made
                        .push(SheetLoadRepair::CopyDropped { object, uuid: id });
                    continue;
                }
                let new = self.fresh_uuid(kind, index + 1, id, "");
                (access.set_uuid)(&mut item, new);
                self.taken.insert(new, kind);
                self.made.push(SheetLoadRepair::UuidReminted {
                    object,
                    old: id,
                    new,
                    holder,
                    owner: owner.map(str::to_string),
                });
            } else {
                self.taken.insert(id, kind);
                holders.insert(id, kept.len());
            }
            kept.push(item);
        }
        *items = kept;
    }

    /// A uuid not yet taken, derived from the document's content. `scope`
    /// tells repairs of the same position apart.
    fn fresh_uuid(&self, kind: &str, position: usize, old: Uuid, scope: &str) -> Uuid {
        for attempt in 0u32.. {
            let candidate = derived_uuid(&format!(
                "{}|{kind}|{position}|{old}|{scope}|{attempt}",
                self.root
            ));
            if !self.taken.contains_key(&candidate) {
                return candidate;
            }
        }
        unreachable!("the attempt counter outlasts any document")
    }
}

/// How [`IdSpace::check`] reads and changes the id of a `T` and names it.
pub(in crate::format) struct Access<T> {
    pub uuid: fn(&T) -> Uuid,
    pub set_uuid: fn(&mut T, Uuid),
    pub name: fn(&T) -> String,
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

/// The name of an object that has none.
fn no_name<T>(_: &T) -> String {
    String::new()
}

/// An [`Access`] for objects whose id is a plain `uuid` field.
macro_rules! by_field {
    ($name:expr) => {
        Access {
            uuid: |x| x.uuid,
            set_uuid: |x, u| x.uuid = u,
            name: $name,
        }
    };
}

/// Make every object uuid on `sheet` unique, returning what was repaired
/// (empty when nothing repeated).
pub(in crate::format) fn repair_duplicate_uuids(
    sheet: &mut SchematicSheet,
) -> Vec<SheetLoadRepair> {
    let mut ids = IdSpace::new(sheet.uuid);
    repair_objects(&mut ids, sheet);
    repair_sheet_pins(&mut ids, sheet);
    for symbol in &mut sheet.symbols {
        repair_pin_uuids(symbol, &mut ids);
    }
    let made = ids.into_repairs();
    move_side_map_entries(sheet, &made);
    made
}

/// The objects a sheet lists, one kind after another.
fn repair_objects(ids: &mut IdSpace, sheet: &mut SchematicSheet) {
    ids.check(
        &mut sheet.symbols,
        kind::SYMBOL,
        None,
        by_field!(|s| s.reference.clone()),
    );
    ids.check(&mut sheet.wires, kind::WIRE, None, by_field!(no_name));
    ids.check(
        &mut sheet.junctions,
        kind::JUNCTION,
        None,
        by_field!(no_name),
    );
    ids.check(
        &mut sheet.labels,
        kind::LABEL,
        None,
        by_field!(|l| l.text.clone()),
    );
    ids.check(
        &mut sheet.child_sheets,
        kind::SHEET_SYMBOL,
        None,
        by_field!(|c| c.name.clone()),
    );
    ids.check(
        &mut sheet.no_connects,
        kind::NO_CONNECT,
        None,
        by_field!(no_name),
    );
    ids.check(
        &mut sheet.text_notes,
        kind::TEXT_NOTE,
        None,
        by_field!(|t| t.text.clone()),
    );
    ids.check(&mut sheet.buses, kind::BUS, None, by_field!(no_name));
    ids.check(
        &mut sheet.bus_entries,
        kind::BUS_ENTRY,
        None,
        by_field!(no_name),
    );
    let drawing = Access {
        uuid: |d: &crate::schematic::SchDrawing| d.uuid(),
        set_uuid: |d, u| d.set_uuid(u),
        name: no_name,
    };
    ids.check(&mut sheet.drawings, kind::DRAWING, None, drawing);
    ids.check(
        &mut sheet.no_erc_directives,
        kind::NO_ERC_DIRECTIVE,
        None,
        by_field!(no_name),
    );
}

/// The pins of each sheet symbol; they share the sheet's id space.
fn repair_sheet_pins(ids: &mut IdSpace, sheet: &mut SchematicSheet) {
    for child in &mut sheet.child_sheets {
        let owner = child.name.clone();
        ids.check(
            &mut child.pins,
            kind::SHEET_PIN,
            Some(&owner),
            by_field!(|p| p.name.clone()),
        );
    }
}

/// Maps keyed by object uuid (`net_colors` for wires and junctions,
/// `drawing_placements` for drawings). A re-minted object takes its old
/// id's entry: copied when the first holder is the same sort of object (the
/// entry may be both's), moved when the holder is another kind (the entry
/// was never the holder's, and must not stay behind to be written back).
fn move_side_map_entries(sheet: &mut SchematicSheet, repairs: &[SheetLoadRepair]) {
    let colored = |k: &str| k == kind::WIRE || k == kind::JUNCTION;
    for repair in repairs {
        let SheetLoadRepair::UuidReminted {
            object,
            old,
            new,
            holder,
            ..
        } = repair
        else {
            continue;
        };
        if colored(object.kind) {
            if let Some(color) = sheet.net_colors.get(old).copied() {
                sheet.net_colors.insert(*new, color);
                if !colored(holder) {
                    sheet.net_colors.remove(old);
                }
            }
        } else if object.kind == kind::DRAWING {
            if let Some(placement) = sheet.drawing_placements.get(old).copied() {
                sheet.drawing_placements.insert(*new, placement);
                if *holder != kind::DRAWING {
                    sheet.drawing_placements.remove(old);
                }
            }
        }
    }
}

/// A symbol's per-pin ids (`pin_uuids`, by pin number): two pins of one
/// symbol must not share one. The pin cannot be dropped, so the later pin
/// (in lexicographic pin-number order) is always re-minted.
fn repair_pin_uuids(symbol: &mut crate::schematic::Symbol, ids: &mut IdSpace) {
    let mut numbers: Vec<String> = symbol.pin_uuids.keys().cloned().collect();
    numbers.sort();
    let mut own: HashSet<Uuid> = HashSet::new();
    for number in numbers {
        let old = symbol.pin_uuids[&number];
        if own.insert(old) {
            continue;
        }
        let scope = format!("{}|{number}", symbol.uuid);
        let new = (0u32..)
            .map(|extra| ids.fresh_uuid("pin", extra as usize, old, &scope))
            .find(|candidate| own.insert(*candidate))
            .expect("an unused candidate exists");
        symbol.pin_uuids.insert(number.clone(), new);
        ids.made.push(SheetLoadRepair::PinUuidReminted {
            symbol: symbol.reference.clone(),
            pin: number,
            old,
            new,
        });
    }
}
