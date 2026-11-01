//! Repair of repeated object uuids on a freshly parsed sheet (#1830).
//!
//! Selection, undo, cross-probe and the BOM address objects by uuid, and
//! the `[extras.*]` tables are keyed by it, so two objects sharing one id
//! are ambiguous and the later one's extras overwrite the earlier's on
//! save. Every uuid-carrying object kind shares ONE id space per sheet:
//!
//! - a later object whose id was already taken by an identical earlier
//!   object of the same kind is a copy-paste duplicate: it is dropped;
//! - otherwise it is a different object that collided: it keeps its
//!   content and gets a fresh UUID v7.
//!
//! Either way a sentence naming the object lands in
//! `SheetUnknown::load_repairs`, which the open path shows in Messages and
//! turns into a modified sheet (the file on disk still holds the repeat).
//! The first holder of an id is never changed.

use std::collections::HashSet;

use uuid::Uuid;

use crate::schematic::SchematicSheet;

/// The ids taken so far and the sentences written so far.
#[derive(Default)]
struct Repairs {
    seen: HashSet<Uuid>,
    sentences: Vec<String>,
}

impl Repairs {
    /// Make the ids in `items` unique; `noun` and `name` say what a
    /// repaired object is called in the sentence.
    fn check<T: PartialEq>(
        &mut self,
        items: &mut Vec<T>,
        noun: &str,
        uuid: fn(&T) -> Uuid,
        set_uuid: fn(&mut T, Uuid),
        name: fn(&T) -> String,
    ) {
        let mut kept: Vec<T> = Vec::with_capacity(items.len());
        for (index, mut item) in std::mem::take(items).into_iter().enumerate() {
            let id = uuid(&item);
            if self.seen.insert(id) {
                kept.push(item);
                continue;
            }
            let label = describe(noun, &name(&item), index + 1);
            if kept.contains(&item) {
                self.sentences.push(format!(
                    "{label} repeated uuid {id} of an identical {noun}; dropped"
                ));
                continue;
            }
            let fresh = fresh_uuid(&mut self.seen);
            set_uuid(&mut item, fresh);
            self.sentences
                .push(format!("{label} repeated uuid {id}; new uuid {fresh}"));
            kept.push(item);
        }
        *items = kept;
    }
}

/// `noun`, its `name` when it has one, and the 1-based `row` it came from.
fn describe(noun: &str, name: &str, row: usize) -> String {
    if name.is_empty() {
        format!("{noun} (row {row})")
    } else {
        format!("{noun} {name} (row {row})")
    }
}

fn fresh_uuid(seen: &mut HashSet<Uuid>) -> Uuid {
    loop {
        let candidate = Uuid::now_v7();
        if seen.insert(candidate) {
            return candidate;
        }
    }
}

/// Make every object uuid on `sheet` unique, returning one sentence per
/// repair (empty when nothing repeated).
pub(in crate::format) fn repair_duplicate_uuids(sheet: &mut SchematicSheet) -> Vec<String> {
    let mut r = Repairs::default();
    r.check(
        &mut sheet.symbols,
        "symbol",
        |s| s.uuid,
        |s, u| s.uuid = u,
        |s| s.reference.clone(),
    );
    r.check(
        &mut sheet.wires,
        "wire",
        |w| w.uuid,
        |w, u| w.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.junctions,
        "junction",
        |j| j.uuid,
        |j, u| j.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.labels,
        "label",
        |l| l.uuid,
        |l, u| l.uuid = u,
        |l| l.text.clone(),
    );
    r.check(
        &mut sheet.child_sheets,
        "sheet symbol",
        |c| c.uuid,
        |c, u| c.uuid = u,
        |c| c.name.clone(),
    );
    r.check(
        &mut sheet.no_connects,
        "no-connect",
        |n| n.uuid,
        |n, u| n.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.text_notes,
        "text note",
        |t| t.uuid,
        |t, u| t.uuid = u,
        |t| t.text.clone(),
    );
    r.check(
        &mut sheet.buses,
        "bus",
        |b| b.uuid,
        |b, u| b.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.bus_entries,
        "bus entry",
        |b| b.uuid,
        |b, u| b.uuid = u,
        |_| String::new(),
    );
    r.check(
        &mut sheet.drawings,
        "drawing",
        |d| d.uuid(),
        |d, u| d.set_uuid(u),
        |_| String::new(),
    );
    r.check(
        &mut sheet.no_erc_directives,
        "no-ERC directive",
        |n| n.uuid,
        |n, u| n.uuid = u,
        |_| String::new(),
    );
    for child in &mut sheet.child_sheets {
        let before = r.sentences.len();
        r.check(
            &mut child.pins,
            "sheet pin",
            |p| p.uuid,
            |p, u| p.uuid = u,
            |p| p.name.clone(),
        );
        for sentence in &mut r.sentences[before..] {
            sentence.push_str(&format!(" on sheet symbol {}", child.name));
        }
    }
    for symbol in &mut sheet.symbols {
        repair_pin_uuids(symbol, &mut r);
    }
    r.sentences
}

/// A symbol's per-pin ids (`pin_uuids`, by pin number). The pin cannot be
/// dropped, so a repeated id is always re-minted.
fn repair_pin_uuids(symbol: &mut crate::schematic::Symbol, r: &mut Repairs) {
    let mut numbers: Vec<String> = symbol.pin_uuids.keys().cloned().collect();
    numbers.sort();
    for number in numbers {
        let id = symbol.pin_uuids[&number];
        if r.seen.insert(id) {
            continue;
        }
        let fresh = fresh_uuid(&mut r.seen);
        symbol.pin_uuids.insert(number.clone(), fresh);
        r.sentences.push(format!(
            "pin {number} of symbol {} repeated uuid {id}; new uuid {fresh}",
            symbol.reference
        ));
    }
}
