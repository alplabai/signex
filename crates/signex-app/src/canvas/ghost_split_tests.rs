//! The ordered walk (`ghost_split.rs`) gives the full split's frame, exactly:
//! the moving sheet, field by field, and the skip set. Seeded random sheets
//! are edited the ways a drag step edits them (items changed, dropped, added
//! in the middle or at the end) and the ways it must not be trusted with
//! (reordered, an id repeated in either sheet, a new id repeated), and the
//! walk is held to [`split_ghost`] in every one.

use std::collections::HashSet;

use signex_types::schematic::{
    Bus, Junction, Label, LabelType, NoConnect, Point, SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

use super::drag_layers::{DragFrame, split_ghost};
use super::ghost_split::{BaseIds, split_ghost_walk, try_split_ghost_walk};

/// A small deterministic generator.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % bound
    }
}

fn point(rng: &mut Lcg) -> Point {
    Point::new(rng.next(200) as f64, rng.next(200) as f64)
}

fn wire(rng: &mut Lcg) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: point(rng),
        end: point(rng),
        ..Wire::default()
    }
}

fn base_sheet(rng: &mut Lcg) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    for _ in 0..rng.next(40) {
        sheet.wires.push(wire(rng));
    }
    for _ in 0..rng.next(12) {
        sheet.symbols.push(Symbol {
            uuid: Uuid::now_v7(),
            lib_id: "A".to_string(),
            position: point(rng),
            ..Symbol::default()
        });
    }
    for _ in 0..rng.next(8) {
        sheet.junctions.push(Junction {
            uuid: Uuid::now_v7(),
            position: point(rng),
            ..Junction::default()
        });
        sheet
            .labels
            .push(Label::new("N", point(rng), LabelType::Net));
    }
    for _ in 0..rng.next(4) {
        sheet.no_connects.push(NoConnect {
            uuid: Uuid::now_v7(),
            position: point(rng),
        });
        sheet.buses.push(Bus {
            uuid: Uuid::now_v7(),
            start: point(rng),
            end: point(rng),
        });
    }
    sheet
}

/// Edit one list the way a step might: change, drop, add, and now and then
/// break the walk's assumptions. Returns the ids of items it changed.
fn edit<T: Clone>(
    rng: &mut Lcg,
    items: &mut Vec<T>,
    change: impl Fn(&mut T, &mut Lcg),
    fresh: impl Fn(&mut Lcg) -> T,
    set_id: impl Fn(&mut T, Uuid),
    hostile: bool,
) {
    for _ in 0..rng.next(5) {
        if items.is_empty() {
            break;
        }
        let at = rng.next(items.len() as u64) as usize;
        match rng.next(4) {
            0 | 1 => change(&mut items[at], rng),
            2 => {
                items.remove(at);
            }
            _ => {
                let new = fresh(rng);
                let place = rng.next(items.len() as u64 + 1) as usize;
                items.insert(place, new);
            }
        }
    }
    if rng.next(3) == 0 {
        items.push(fresh(rng));
    }
    if hostile && items.len() >= 2 {
        match rng.next(4) {
            // Reordered.
            0 => {
                let (a, b) = (0, items.len() - 1);
                items.swap(a, b);
            }
            // An id met twice in the ghost.
            1 => {
                let copy = items[rng.next(items.len() as u64) as usize].clone();
                items.push(copy);
            }
            // A new id met twice.
            2 => {
                let mut one = fresh(rng);
                let id = Uuid::now_v7();
                set_id(&mut one, id);
                let two = one.clone();
                items.push(one);
                items.push(two);
            }
            _ => {}
        }
    }
}

fn ghost_sheet(rng: &mut Lcg, base: &SchematicSheet, hostile: bool) -> SchematicSheet {
    let mut ghost = base.clone();
    edit(
        rng,
        &mut ghost.wires,
        |w, rng| w.end = point(rng),
        wire,
        |w, id| w.uuid = id,
        hostile,
    );
    edit(
        rng,
        &mut ghost.symbols,
        |s, rng| s.position = point(rng),
        |rng| Symbol {
            uuid: Uuid::now_v7(),
            lib_id: "A".to_string(),
            position: point(rng),
            ..Symbol::default()
        },
        |s, id| s.uuid = id,
        hostile,
    );
    edit(
        rng,
        &mut ghost.junctions,
        |j, rng| j.position = point(rng),
        |rng| Junction {
            uuid: Uuid::now_v7(),
            position: point(rng),
            ..Junction::default()
        },
        |j, id| j.uuid = id,
        hostile,
    );
    edit(
        rng,
        &mut ghost.labels,
        |l, rng| l.position = point(rng),
        |rng| Label::new("N", point(rng), LabelType::Net),
        |l, id| l.uuid = id,
        hostile,
    );
    ghost
}

/// The frames as bytes: the moving sheet field by field and the skip set.
fn exact(frame: &DragFrame) -> (String, Vec<Uuid>) {
    let mut skip: Vec<Uuid> = frame.skip.iter().copied().collect();
    skip.sort();
    (
        serde_json::to_string(&*frame.moving).expect("sheet serialises"),
        skip,
    )
}

fn selection(rng: &mut Lcg, base: &SchematicSheet) -> HashSet<Uuid> {
    let mut ids: Vec<Uuid> = base.symbols.iter().map(|s| s.uuid).collect();
    ids.extend(base.wires.iter().map(|w| w.uuid));
    ids.extend(base.labels.iter().map(|l| l.uuid));
    let mut chosen: HashSet<Uuid> = HashSet::new();
    for _ in 0..rng.next(6) {
        if !ids.is_empty() {
            chosen.insert(ids[rng.next(ids.len() as u64) as usize]);
        }
    }
    // An id the sheet does not hold.
    if rng.next(8) == 0 {
        chosen.insert(Uuid::now_v7());
    }
    chosen
}

#[test]
fn the_walk_equals_the_full_split() {
    let (mut walked, mut fell_back, mut moved, mut skipped, mut selected) = (0, 0, 0, 0, 0);
    for seed in 0..3000u64 {
        let mut rng = Lcg(seed + 11);
        let mut base = base_sheet(&mut rng);
        let hostile = seed % 3 == 0;
        // An id repeated in the pre-move sheet.
        if seed % 17 == 0 && base.wires.len() >= 2 {
            base.wires[1].uuid = base.wires[0].uuid;
        }
        let ghost = ghost_sheet(&mut rng, &base, hostile);
        let chosen = selection(&mut rng, &base);
        let ids = BaseIds::of(&base);
        let full = split_ghost(&base, &ghost, &chosen);
        let frame = split_ghost_walk(&base, &ghost, &chosen, &ids);
        assert_eq!(exact(&frame), exact(&full), "seed {seed}");
        // An index of another sheet is refused, never read.
        let other = base_sheet(&mut Lcg(seed + 90_001));
        let foreign = BaseIds::of(&other);
        assert!(
            try_split_ghost_walk(&base, &ghost, &chosen, &foreign).is_none()
                || other.wires.len() + other.symbols.len() == 0,
            "seed {seed}: a foreign index was used"
        );
        assert_eq!(
            exact(&split_ghost_walk(&base, &ghost, &chosen, &foreign)),
            exact(&full),
            "seed {seed}: a foreign index"
        );
        match try_split_ghost_walk(&base, &ghost, &chosen, &ids) {
            Some(walk) => {
                walked += 1;
                assert_eq!(exact(&walk), exact(&full), "seed {seed}");
            }
            None => fell_back += 1,
        }
        moved += usize::from(!full.moving.wires.is_empty() || !full.moving.symbols.is_empty());
        skipped += usize::from(full.skip.len() > chosen.len());
        selected += usize::from(!chosen.is_empty());
    }
    eprintln!(
        "walk: {walked} walked, {fell_back} fell back, {moved} with moving items, \
         {skipped} with changed items, {selected} with a selection"
    );
    assert!(walked >= 1500, "only {walked} walked");
    assert!(fell_back >= 400, "only {fell_back} fell back");
    assert!(moved >= 1500 && skipped >= 1500 && selected >= 1500);
}
