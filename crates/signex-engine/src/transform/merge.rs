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

fn touching_pairs(wires: &[Wire]) -> TouchingPairs {
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

impl Engine {
    /// Every pair of wires meeting end to end on one straight line.
    pub(crate) fn touching_wire_pairs(&self) -> TouchingPairs {
        touching_pairs(&self.document.wires)
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
            self.fold(keep, drop, at);
            merged = true;
        }
        merged
    }

    /// Fold wire `drop` into wire `keep` across their shared key `at`.
    /// Returns the key of the survivor's new far end.
    fn fold(&mut self, keep: usize, drop: usize, at: Key) -> Key {
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
        pt_key(&far)
    }

    /// [`Self::merge_collinear_wires`] for an edit that moved wires about:
    /// `pre` holds the wires as they were, and only the places where the wires
    /// now differ from them are weighed, not every wire of the sheet.
    ///
    /// Why this folds exactly what the full pass folds. The full pass folds a
    /// pair meeting at key `at` only when the pair was not already meeting
    /// end to end before the edit. Whether two wires meet at `at`, and
    /// whether they run straight through it, depends only on the wires that
    /// end at `at` (their uuids and geometry). If no wire that differs from
    /// `pre` (changed, added or removed) has an end at `at`, in either state,
    /// the wires ending at `at` are the same as before, so any pair there
    /// already met and the full pass skips it. So every pair the full pass
    /// can fold meets at a key in `dirty`: an end of a wire that differs. The
    /// pass takes the first foldable pair in wire order, which is a pair at a
    /// dirty key, so scanning the wires with an end at a dirty key, in the
    /// same order, finds the same pair. A fold moves the survivor's end from
    /// `at` to `far`, where the dropped wire ended: `far` joins `dirty`
    /// (that is how a chain of new neighbours cascades), the loop runs until
    /// no foldable pair is left, and the result is the full pass's result.
    /// When `pre` or the wires now cannot be told apart by uuid, the full
    /// pass runs.
    pub(crate) fn merge_collinear_wires_near(&mut self, pre: &PreWires) -> bool {
        let Some(mut dirty) = pre.changed_keys(&self.document.wires) else {
            return self.merge_collinear_wires(&touching_pairs(&pre.wires));
        };
        let mut merged = false;
        while let Some((keep, drop, at)) = self.next_merge_near(pre, &dirty) {
            dirty.insert(self.fold(keep, drop, at));
            merged = true;
        }
        merged
    }

    /// [`Self::next_merge`] among the wires with an end in `dirty`.
    fn next_merge_near(&self, pre: &PreWires, dirty: &HashSet<Key>) -> Option<(usize, usize, Key)> {
        let wires = &self.document.wires;
        let mut map: HashMap<Key, Vec<usize>> = HashMap::new();
        for (i, wire) in wires.iter().enumerate() {
            let [a, b] = keys(wire);
            if a == b {
                continue;
            }
            for key in [a, b] {
                if dirty.contains(&key) {
                    map.entry(key).or_default().push(i);
                }
            }
        }
        let mut near: Vec<usize> = map.values().flatten().copied().collect();
        near.sort_unstable();
        near.dedup();
        let occupied: OnceCell<HashSet<Key>> = OnceCell::new();
        for i in near {
            for at in keys(&wires[i]) {
                if !dirty.contains(&at) {
                    continue;
                }
                let Some(j) = partner(wires, &map, i, at) else {
                    continue;
                };
                if self.foldable(i, j, at, &occupied, |a, b| pre.met(a, b)) {
                    return Some((i, j, at));
                }
            }
        }
        None
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
                if self.foldable(i, j, at, &occupied, |a, b| before.contains(&ordered(a, b))) {
                    return Some((i, j, at));
                }
            }
        }
        None
    }

    /// Whether wires `i` and `j`, meeting straight at `at`, fold into one:
    /// `i` is the earlier, neither is locked, they match in every per-wire
    /// attribute, they did not already meet (`met`), and nothing else is
    /// at `at`.
    fn foldable(
        &self,
        i: usize,
        j: usize,
        at: Key,
        occupied: &OnceCell<HashSet<Key>>,
        met: impl Fn(Uuid, Uuid) -> bool,
    ) -> bool {
        let wires = &self.document.wires;
        let (wire, other) = (&wires[i], &wires[j]);
        !(i > j
            || wire.locked
            || other.locked
            || (wire.stroke_width - other.stroke_width).abs() > 1e-9
            // An own colour is kept per wire; folding two drops one.
            || wire.color != other.color
            // Foreign columns belong to one wire; folding would drop one set.
            || wire.unknown_columns != other.unknown_columns
            // A net colour is kept per wire; folding two colours drops one.
            || self.wire_colors.get(&wire.uuid) != self.wire_colors.get(&other.uuid)
            || met(wire.uuid, other.uuid)
            || self.wire_passes_through(at, i, j)
            || occupied.get_or_init(|| self.occupied_points()).contains(&at))
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
        let mut points = self.connection_points();
        points.extend(doc.junctions.iter().map(|j| pt_key(&j.position)));
        points.extend(doc.labels.iter().map(|l| pt_key(&l.position)));
        points.extend(doc.no_connects.iter().map(|n| pt_key(&n.position)));
        points.extend(doc.no_erc_directives.iter().map(|n| pt_key(&n.position)));
        points
    }

    /// Where a pin, sheet pin, bus end or bus-entry end sits: a point a new
    /// wire end would join to the net of that element.
    pub(crate) fn connection_points(&self) -> HashSet<Key> {
        let doc = &self.document;
        let mut points: HashSet<Key> = HashSet::new();
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

/// The wires of a sheet as they stood before an edit, so the fold after it
/// can ask "were these two already meeting end to end?" of just the pairs it
/// weighs, and find which wires the edit changed, without scanning the sheet
/// for every pair (a drag ghost builds this once per drag, not per step).
#[derive(Debug)]
pub(crate) struct PreWires {
    wires: Vec<Wire>,
    map: HashMap<Key, Vec<usize>>,
    index: HashMap<Uuid, usize>,
    /// Every wire has its own uuid; when two share one, [`TouchingPairs`]
    /// (keyed by uuid) cannot be answered pair by pair.
    unique: bool,
}

impl PreWires {
    pub(crate) fn new(wires: &[Wire]) -> Self {
        let index: HashMap<Uuid, usize> =
            wires.iter().enumerate().map(|(i, w)| (w.uuid, i)).collect();
        Self {
            unique: index.len() == wires.len(),
            map: endpoint_map(wires),
            wires: wires.to_vec(),
            index,
        }
    }

    /// Whether `a` and `b` met end to end on a straight line: whether
    /// `touching_wire_pairs` of these wires holds the pair.
    fn met(&self, a: Uuid, b: Uuid) -> bool {
        let Some(&i) = self.index.get(&a) else {
            return false;
        };
        keys(&self.wires[i]).into_iter().any(|at| {
            partner(&self.wires, &self.map, i, at).is_some_and(|j| self.wires[j].uuid == b)
        })
    }

    /// The endpoint keys of every wire that differs between these wires and
    /// `now` (as it was and as it is), added or removed. `None` when `now`
    /// has two wires of one uuid, or these do.
    fn changed_keys(&self, now: &[Wire]) -> Option<HashSet<Key>> {
        if !self.unique {
            return None;
        }
        let mut dirty = HashSet::new();
        let mut seen = vec![false; self.wires.len()];
        for (n, wire) in now.iter().enumerate() {
            // Moves keep the order, so the wire at the same place is the
            // usual match and costs no lookup.
            let slot = match self.wires.get(n) {
                Some(was) if was.uuid == wire.uuid => n,
                _ => match self.index.get(&wire.uuid) {
                    Some(&slot) => slot,
                    None => {
                        dirty.extend(keys(wire));
                        continue;
                    }
                },
            };
            if std::mem::replace(&mut seen[slot], true) {
                return None;
            }
            let was = &self.wires[slot];
            if was.start != wire.start || was.end != wire.end {
                dirty.extend(keys(was));
                dirty.extend(keys(wire));
            }
        }
        for (was, _) in self.wires.iter().zip(&seen).filter(|(_, seen)| !**seen) {
            dirty.extend(keys(was));
        }
        Some(dirty)
    }
}
