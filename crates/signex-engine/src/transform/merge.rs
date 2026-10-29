//! Break a wire in two, and merge collinear wire segments back together (#1254).
//!
//! Merging runs after an edit that can leave two wires meeting end to end on
//! one straight line (a wire drawn on from another, a part or wire moved so
//! two segments line up). It folds only an adjacency the edit *created*, so
//! the Break Wire tool's own split, and any split the user kept while moving
//! both halves, are not put back together.
//!
//! A merge never changes connectivity. The pair must meet at a point where
//! nothing else is: no third wire end, no wire passing through, no junction,
//! pin, label, no-connect, bus end or entry. Both segments must be
//! axis-aligned in the netlist's 1 um key space, where "on the wire" is
//! exact, so every point that was on either old wire is on the merged one.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

use signex_net::point_on_segment;
use signex_net::pt_key;
use signex_net::uf::Key;
use signex_types::schematic::{SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

use crate::Engine;

/// Pairs of wires (smaller uuid first) that meet end to end on one straight
/// axis-aligned line, taken before an edit so only new adjacencies merge.
pub(crate) type TouchingPairs = HashSet<(Uuid, Uuid)>;

fn ordered(a: Uuid, b: Uuid) -> (Uuid, Uuid) {
    if a <= b { (a, b) } else { (b, a) }
}

fn keys(wire: &Wire) -> [Key; 2] {
    [pt_key(&wire.start), pt_key(&wire.end)]
}

/// Wire indices by the endpoint key they touch. A wire with both ends in one
/// bucket has no direction and is left out.
fn endpoint_map(wires: &[Wire]) -> HashMap<Key, Vec<usize>> {
    let mut map: HashMap<Key, Vec<usize>> = HashMap::new();
    for (i, wire) in wires.iter().enumerate() {
        let [a, b] = keys(wire);
        if a == b {
            continue;
        }
        map.entry(a).or_default().push(i);
        map.entry(b).or_default().push(i);
    }
    map
}

/// The end of `wire` that is not at `at`.
fn far_end(wire: &Wire, at: Key) -> Key {
    let [a, b] = keys(wire);
    if a == at { b } else { a }
}

/// Whether `a` and `b`, both ending at `at`, run along one horizontal or
/// vertical line with `at` between their far ends.
fn straight_through(a: &Wire, b: &Wire, at: Key) -> bool {
    let (fa, fb) = (far_end(a, at), far_end(b, at));
    let horizontal = fa.1 == at.1 && fb.1 == at.1;
    let vertical = fa.0 == at.0 && fb.0 == at.0;
    if horizontal {
        return (fa.0 < at.0) != (fb.0 < at.0) && fa.0 != at.0 && fb.0 != at.0;
    }
    vertical && (fa.1 < at.1) != (fb.1 < at.1) && fa.1 != at.1 && fb.1 != at.1
}

/// The index of the one other wire ending at `at` when exactly two do and
/// they run straight through it.
fn partner(wires: &[Wire], map: &HashMap<Key, Vec<usize>>, from: usize, at: Key) -> Option<usize> {
    let ends = map.get(&at)?;
    let [x, y] = ends.as_slice() else {
        return None;
    };
    let other = if *x == from { *y } else { *x };
    (other != from && straight_through(&wires[from], &wires[other], at)).then_some(other)
}

impl Engine {
    /// Every pair of wires meeting end to end on one straight line.
    pub(crate) fn touching_wire_pairs(&self) -> TouchingPairs {
        let wires = &self.document.wires;
        let map = endpoint_map(wires);
        let mut pairs = TouchingPairs::new();
        for (i, wire) in wires.iter().enumerate() {
            for at in keys(wire) {
                if let Some(j) = partner(wires, &map, i, at) {
                    pairs.insert(ordered(wire.uuid, wires[j].uuid));
                }
            }
        }
        pairs
    }

    /// Merge each pair of wires that now meets end to end on a straight line
    /// and was not already doing so in `before`, as long as nothing else sits
    /// at the shared point. Returns whether anything merged. The wire earlier
    /// in the sheet survives, so its uuid (and the net colour kept against it)
    /// stays.
    ///
    /// No junction work follows a merge: the merged wire has the endpoints
    /// its two halves had, so no point gains or loses a meeting. Reconciling
    /// the survivor's far ends would instead mint dots for junction-less
    /// crossings the user left unconnected.
    pub(crate) fn merge_collinear_wires(&mut self, before: &TouchingPairs) -> bool {
        let mut merged = false;
        // Each merge frees one wire, so the pass ends after at most one pass
        // per wire.
        while let Some((keep, drop, at)) = self.next_merge(before) {
            let far = end_at(
                &self.document.wires[drop],
                far_end(&self.document.wires[drop], at),
            );
            let wire = &mut self.document.wires[keep];
            if pt_key(&wire.start) == at {
                wire.start = far;
            } else {
                wire.end = far;
            }
            self.document.wires.remove(drop);
            merged = true;
        }
        merged
    }

    /// The next mergeable pair as `(keep index, drop index, shared key)`.
    fn next_merge(&self, before: &TouchingPairs) -> Option<(usize, usize, Key)> {
        let wires = &self.document.wires;
        let map = endpoint_map(wires);
        let occupied: OnceCell<HashSet<Key>> = OnceCell::new();
        for (i, wire) in wires.iter().enumerate() {
            for at in keys(wire) {
                let Some(j) = partner(wires, &map, i, at) else {
                    continue;
                };
                let other = &wires[j];
                if i > j
                    || wire.locked
                    || other.locked
                    || (wire.stroke_width - other.stroke_width).abs() > 1e-9
                    // Foreign columns belong to one wire; folding would drop one set.
                    || wire.unknown_columns != other.unknown_columns
                    // A net colour is kept per wire; folding two colours drops one.
                    || self.wire_colors.get(&wire.uuid) != self.wire_colors.get(&other.uuid)
                    || before.contains(&ordered(wire.uuid, other.uuid))
                    || self.wire_passes_through(at, i, j)
                    || occupied
                        .get_or_init(|| self.occupied_points())
                        .contains(&at)
                {
                    continue;
                }
                return Some((i, j, at));
            }
        }
        None
    }

    /// Whether a wire other than `a` and `b` runs through `at`.
    fn wire_passes_through(&self, at: Key, a: usize, b: usize) -> bool {
        self.document
            .wires
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != a && *i != b)
            .any(|(_, w)| {
                let [s, e] = keys(w);
                point_on_segment(at, s, e)
            })
    }

    /// Where something other than a wire attaches: junctions, pins, labels,
    /// no-connects, no-ERC directives, bus ends and bus entries.
    fn occupied_points(&self) -> HashSet<Key> {
        let doc = &self.document;
        let mut points: HashSet<Key> = HashSet::new();
        points.extend(doc.junctions.iter().map(|j| pt_key(&j.position)));
        points.extend(doc.labels.iter().map(|l| pt_key(&l.position)));
        points.extend(doc.no_connects.iter().map(|n| pt_key(&n.position)));
        points.extend(doc.no_erc_directives.iter().map(|n| pt_key(&n.position)));
        for bus in &doc.buses {
            points.extend([pt_key(&bus.start), pt_key(&bus.end)]);
        }
        for entry in &doc.bus_entries {
            let far = signex_types::schematic::Point::new(
                entry.position.x + entry.size.0,
                entry.position.y + entry.size.1,
            );
            points.extend([pt_key(&entry.position), pt_key(&far)]);
        }
        let parts: Vec<SelectedItem> = doc
            .symbols
            .iter()
            .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
            .chain(
                doc.child_sheets
                    .iter()
                    .map(|c| SelectedItem::new(c.uuid, SelectedKind::ChildSheet)),
            )
            .collect();
        points.extend(
            crate::moved_pin_tips(doc, &parts)
                .iter()
                .map(crate::PinTip::old_key),
        );
        points.extend(
            doc.child_sheets
                .iter()
                .flat_map(|c| c.pins.iter().map(|p| pt_key(&p.position))),
        );
        points
    }
}

/// The endpoint of `wire` that sits at `key`.
fn end_at(wire: &Wire, key: Key) -> signex_types::schematic::Point {
    if pt_key(&wire.start) == key {
        wire.start
    } else {
        wire.end
    }
}
