//! [`split_ghost`](super::drag_layers::split_ghost) without hashing the sheet
//! on every cursor step.
//!
//! The split compares the ghost with the pre-move sheet item by item. Done
//! from scratch it indexes both lists by id on every step. The pre-move sheet
//! is the same for the whole drag, so [`BaseIds`] indexes it once, and the
//! ghost, which is the pre-move list with a few items replaced, dropped or
//! appended (rubber band, junction dots, wire folds), is read in one ordered
//! walk against it.
//!
//! The walk gives exactly the full split's answer when every id is met once
//! per list and the ghost keeps the pre-move order:
//!
//! * a ghost item with the id of the next unmatched pre-move item (or of a
//!   later one, which makes the ones skipped over gone) is that item,
//!   changed or not;
//! * a ghost item with an id the pre-move list lacks is new, and moves;
//! * an id met twice in the pre-move list, a ghost id that repeats, a new id
//!   that repeats, or a ghost item that goes back to an earlier slot would
//!   make the full split's per-id maps differ from the walk, so the walk
//!   stops and the split runs in full.
//!
//! What the walk decides per item is the full split's rule: a pre-move id is
//! skipped when its ghost counterpart is gone or differs; a ghost item moves
//! when its id is skipped or selected or is new. With each id met once, no
//! other item can change that answer, and the order of `moving` is the
//! ghost's, as in the full split.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use signex_types::schematic::{ChildSheet, SchDrawing, SchematicSheet};
use uuid::Uuid;

use super::drag_layers::{DragFrame, split_ghost};

/// The ids of one kind of item of the pre-move sheet, by position.
#[derive(Debug)]
struct KindIds {
    at: HashMap<Uuid, usize>,
    /// Every id once.
    unique: bool,
    /// How many items, and the sum and xor of their ids: the walk checks the
    /// sheet it is given against these, so an index of another sheet is
    /// refused instead of read as "this id is new".
    len: usize,
    sum: u128,
    xor: u128,
}

/// The sum and xor of the ids, as the fingerprint of a list.
fn fingerprint(ids: impl Iterator<Item = Uuid>) -> (u128, u128) {
    ids.fold((0u128, 0u128), |(sum, xor), id| {
        (sum.wrapping_add(id.as_u128()), xor ^ id.as_u128())
    })
}

impl KindIds {
    fn of<T>(items: &[T], id: impl Fn(&T) -> Uuid) -> Self {
        let mut at = HashMap::with_capacity(items.len());
        for (slot, item) in items.iter().enumerate() {
            at.entry(id(item)).or_insert(slot);
        }
        let unique = at.len() == items.len();
        let (sum, xor) = fingerprint(items.iter().map(&id));
        Self {
            at,
            unique,
            len: items.len(),
            sum,
            xor,
        }
    }
}

/// The ids of the pre-move sheet, found once per drag.
#[derive(Debug)]
pub(super) struct BaseIds {
    symbols: KindIds,
    wires: KindIds,
    junctions: KindIds,
    labels: KindIds,
    child_sheets: KindIds,
    no_connects: KindIds,
    text_notes: KindIds,
    buses: KindIds,
    bus_entries: KindIds,
    drawings: KindIds,
    no_erc_directives: KindIds,
}

impl BaseIds {
    pub(super) fn of(base: &SchematicSheet) -> Self {
        Self {
            symbols: KindIds::of(&base.symbols, |s| s.uuid),
            wires: KindIds::of(&base.wires, |w| w.uuid),
            junctions: KindIds::of(&base.junctions, |j| j.uuid),
            labels: KindIds::of(&base.labels, |l| l.uuid),
            child_sheets: KindIds::of(&base.child_sheets, |c: &ChildSheet| c.uuid),
            no_connects: KindIds::of(&base.no_connects, |n| n.uuid),
            text_notes: KindIds::of(&base.text_notes, |t| t.uuid),
            buses: KindIds::of(&base.buses, |b| b.uuid),
            bus_entries: KindIds::of(&base.bus_entries, |b| b.uuid),
            drawings: KindIds::of(&base.drawings, SchDrawing::uuid),
            no_erc_directives: KindIds::of(&base.no_erc_directives, |d| d.uuid),
        }
    }
}

/// One item kind of the walk; `false` when the lists are not ones the walk
/// answers for (see the module docs). `skip` and `moving` are only complete
/// when it returns `true`.
fn walk_kind<T: PartialEq + Clone>(
    base: &[T],
    ghost: &[T],
    id: impl Fn(&T) -> Uuid,
    ids: &KindIds,
    skip: &mut HashSet<Uuid>,
    moving: &mut Vec<T>,
) -> bool {
    if !ids.unique || ids.len != base.len() {
        return false;
    }
    if fingerprint(base.iter().map(&id)) != (ids.sum, ids.xor) {
        return false;
    }
    // The next pre-move item no ghost item has matched yet.
    let mut next = 0;
    let mut fresh: HashSet<Uuid> = HashSet::new();
    for item in ghost {
        let uuid = id(item);
        // Most steps keep the order, so the next slot is the likely match.
        let slot = match base.get(next) {
            Some(b) if id(b) == uuid => Some(next),
            _ => ids.at.get(&uuid).copied(),
        };
        let Some(slot) = slot else {
            if !fresh.insert(uuid) {
                return false;
            }
            moving.push(item.clone());
            continue;
        };
        // An index of another sheet (or a repeated id) is not trusted.
        if slot < next || slot >= base.len() || id(&base[slot]) != uuid {
            return false;
        }
        for gone in &base[next..slot] {
            skip.insert(id(gone));
        }
        next = slot + 1;
        if *item != base[slot] {
            skip.insert(uuid);
        }
        if skip.contains(&uuid) {
            moving.push(item.clone());
        }
    }
    for gone in &base[next..] {
        skip.insert(id(gone));
    }
    true
}

/// [`split_ghost`] for a drag that has indexed its pre-move sheet; the very
/// same frame, found by walking the ghost instead of indexing it.
pub(super) fn split_ghost_walk(
    base: &SchematicSheet,
    ghost: &SchematicSheet,
    selected: &HashSet<Uuid>,
    ids: &BaseIds,
) -> DragFrame {
    try_split_ghost_walk(base, ghost, selected, ids)
        .unwrap_or_else(|| split_ghost(base, ghost, selected))
}

/// The walk's frame, or `None` when the sheets are not ones it answers for
/// (see the module docs).
pub(super) fn try_split_ghost_walk(
    base: &SchematicSheet,
    ghost: &SchematicSheet,
    selected: &HashSet<Uuid>,
    ids: &BaseIds,
) -> Option<DragFrame> {
    let mut skip = selected.clone();
    let mut moving = SchematicSheet {
        uuid: ghost.uuid,
        font_family: ghost.font_family.clone(),
        lib_symbols: ghost.lib_symbols.clone(),
        drawing_placements: ghost.drawing_placements.clone(),
        ..SchematicSheet::default()
    };
    macro_rules! kind {
        ($field:ident, $id:expr) => {
            if !walk_kind(
                &base.$field,
                &ghost.$field,
                $id,
                &ids.$field,
                &mut skip,
                &mut moving.$field,
            ) {
                return None;
            }
        };
    }
    kind!(symbols, |s: &signex_types::schematic::Symbol| s.uuid);
    kind!(wires, |w: &signex_types::schematic::Wire| w.uuid);
    kind!(junctions, |j: &signex_types::schematic::Junction| j.uuid);
    kind!(labels, |l: &signex_types::schematic::Label| l.uuid);
    kind!(child_sheets, |c: &ChildSheet| c.uuid);
    kind!(no_connects, |n: &signex_types::schematic::NoConnect| n.uuid);
    kind!(text_notes, |t: &signex_types::schematic::TextNote| t.uuid);
    kind!(buses, |b: &signex_types::schematic::Bus| b.uuid);
    kind!(bus_entries, |b: &signex_types::schematic::BusEntry| b.uuid);
    kind!(drawings, SchDrawing::uuid);
    kind!(
        no_erc_directives,
        |d: &signex_types::schematic::NoConnect| d.uuid
    );
    Some(DragFrame {
        moving: Rc::new(moving),
        skip: Rc::new(skip),
    })
}
