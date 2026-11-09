//! The working copy of the rubber-band pass and the geometry questions every
//! step asks of it; routing lives in [`route`], following in [`follow`], the
//! overlap clean-up in [`overlap`].

use std::collections::HashSet;

use signex_net::{point_on_segment, pt_key, uf::Key};
use signex_types::schematic::{Point, SchematicSheet, SymbolTransform, Wire, instance_pins};
use uuid::Uuid;

#[cfg(test)]
pub(crate) use grid::scan_all_on;
use grid::{Grid, Kind};

use super::{Move, Policy};

mod follow;
mod grid;
mod overlap;
mod route;

#[cfg(test)]
pub(crate) use overlap::wires_overlap;

pub(super) fn same_delta(a: &Move, b: &Move) -> bool {
    pt_key(&Point::new(a.new.x - a.old.x, a.new.y - a.old.y))
        == pt_key(&Point::new(b.new.x - b.old.x, b.new.y - b.old.y))
}

/// Working copy of everything a tip can touch. Pin tips never change during
/// the pass; wires and dots do.
pub(super) struct Env {
    wires: Vec<Wire>,
    removed: HashSet<usize>,
    /// Wire ends already put at their final place (`(index, is start)`); a
    /// later tip must not mistake them for ends still waiting on a pin.
    placed: HashSet<(usize, bool)>,
    labels: Vec<(Uuid, Point)>,
    junctions: Vec<(Uuid, Point)>,
    no_connects: Vec<(Uuid, Point)>,
    /// Wires this pass edited or created; only these are checked for
    /// duplicates and overlaps afterwards.
    edited: HashSet<usize>,
    policy: Policy,
    tips: Vec<(Uuid, Key)>,
    selected: HashSet<Uuid>,
    moving: HashSet<Uuid>,
    /// Where every moving pin's tip was: a joint on one of these is where a
    /// pin still has to be reconnected, so it cannot slide.
    old_tips: HashSet<Key>,
    /// Where things are, so a question reads what is near and not the sheet.
    grid: Grid,
    /// Dots and no-connects the pass moved: the only ones written back.
    moved_junctions: Vec<usize>,
    moved_no_connects: Vec<usize>,
    /// While a follow may still be undone: each wire end it moved, as it was.
    undo: Option<Vec<(usize, Point, Point)>>,
}

/// Everything on an old tip other than the wires attached to it.
#[derive(Default)]
pub(super) struct Probe {
    wire_ends: usize,
    interior: bool,
    label: bool,
    no_connect: bool,
    stationary_pin: bool,
    junctions: Vec<usize>,
}

impl Probe {
    fn anything(&self) -> bool {
        self.wire_ends > 0
            || self.interior
            || self.label
            || self.no_connect
            || self.stationary_pin
            || !self.junctions.is_empty()
    }
}

pub(super) struct Plan {
    pub(super) replace: Vec<usize>,
    pub(super) segs: Vec<(Point, Point)>,
    /// Keys of the ends that stay joined to what they were joined to; every
    /// other end of an edited wire is final.
    pub(super) joins: Vec<Key>,
}

pub(super) fn key(p: &Point) -> Key {
    pt_key(p)
}

pub(super) fn zero_length(a: &Point, b: &Point) -> bool {
    key(a) == key(b)
}

impl Env {
    pub(in crate::transform::rubber_band) fn new(
        document: &SchematicSheet,
        selected: HashSet<Uuid>,
        moving: HashSet<Uuid>,
        old_tips: HashSet<Key>,
        policy: Policy,
    ) -> Self {
        let mut tips = Vec::new();
        for symbol in &document.symbols {
            let Some(lib) = document.resolve_lib_symbol(symbol) else {
                continue;
            };
            let xform = SymbolTransform::from_symbol(symbol);
            for lib_pin in instance_pins(&lib, symbol) {
                tips.push((symbol.uuid, key(&xform.apply(lib_pin.pin.position))));
            }
        }
        for pin in document.child_sheets.iter().flat_map(|c| c.pins.iter()) {
            tips.push((pin.uuid, key(&pin.position)));
        }
        let mut env = Self {
            wires: document.wires.clone(),
            removed: HashSet::new(),
            placed: HashSet::new(),
            labels: document
                .labels
                .iter()
                .map(|l| (l.uuid, l.position))
                .collect(),
            junctions: document
                .junctions
                .iter()
                .map(|j| (j.uuid, j.position))
                .collect(),
            no_connects: document
                .no_connects
                .iter()
                .map(|n| (n.uuid, n.position))
                .collect(),
            edited: HashSet::new(),
            policy,
            tips,
            selected,
            moving,
            old_tips,
            grid: Grid::default(),
            moved_junctions: Vec::new(),
            moved_no_connects: Vec::new(),
            undo: None,
        };
        env.build_grid();
        env
    }

    fn build_grid(&mut self) {
        #[cfg(test)]
        if grid::scan_all_requested() {
            self.grid.scan_all();
        }
        for index in 0..self.wires.len() {
            self.refile_wire(index);
        }
        for (index, (_, p)) in self.labels.iter().enumerate() {
            self.grid.file(Kind::Label, index, key(p), key(p));
        }
        for (index, (_, p)) in self.junctions.iter().enumerate() {
            self.grid.file(Kind::Junction, index, key(p), key(p));
        }
        for (index, (_, p)) in self.no_connects.iter().enumerate() {
            self.grid.file(Kind::NoConnect, index, key(p), key(p));
        }
        for (index, (_, k)) in self.tips.iter().enumerate() {
            self.grid.file(Kind::Tip, index, *k, *k);
        }
    }

    /// Wire `index` has a new place (or is new): file it there too.
    pub(super) fn refile_wire(&mut self, index: usize) {
        let wire = &self.wires[index];
        self.grid
            .file(Kind::Wire, index, key(&wire.start), key(&wire.end));
    }

    /// Indices of the wires with a box over `k` (live or not).
    pub(super) fn grid_wires_at(&self, k: Key) -> Vec<usize> {
        self.grid.near(Kind::Wire, k, k, self.wires.len())
    }

    /// Indices of the no-connects that can be on `k`.
    pub(super) fn no_connects_near(&self, k: Key) -> Vec<usize> {
        self.grid
            .near(Kind::NoConnect, k, k, self.no_connects.len())
    }

    /// No-connect `index` has a new place: file it there too.
    pub(super) fn file_no_connect(&mut self, index: usize) {
        let k = key(&self.no_connects[index].1);
        self.grid.file(Kind::NoConnect, index, k, k);
    }

    /// Dot `index` has a new place: file it there too.
    pub(super) fn file_junction(&mut self, index: usize) {
        let k = key(&self.junctions[index].1);
        self.grid.file(Kind::Junction, index, k, k);
    }

    /// The live wires that can meet the box `a`-`b`, in sheet order.
    pub(super) fn wires_near(&self, a: Key, b: Key) -> Vec<(usize, &Wire)> {
        (self
            .grid
            .near(Kind::Wire, a, b, self.wires.len())
            .into_iter())
        .filter(|index| !self.removed.contains(index))
        .map(|index| (index, &self.wires[index]))
        .collect()
    }

    pub(in crate::transform::rubber_band) fn write_back(self, document: &mut SchematicSheet) {
        let Self {
            wires,
            removed,
            junctions,
            no_connects,
            moved_junctions,
            moved_no_connects,
            ..
        } = self;
        for index in moved_no_connects {
            let (uuid, position) = no_connects[index];
            if let Some(mark) = document.no_connects.iter_mut().find(|n| n.uuid == uuid) {
                mark.position = position;
            }
        }
        document.wires = wires
            .into_iter()
            .enumerate()
            .filter(|(index, _)| !removed.contains(index))
            .map(|(_, wire)| wire)
            .collect();
        for index in moved_junctions {
            let (uuid, position) = junctions[index];
            if let Some(dot) = document.junctions.iter_mut().find(|j| j.uuid == uuid) {
                dot.position = position;
            }
        }
    }

    /// Everything stationary on `at`, ignoring the wire indices in `skip`.
    pub(super) fn probe(&self, at: Key, skip: &[usize]) -> Probe {
        let mut probe = Probe::default();
        for (index, wire) in self.wires_near(at, at) {
            if skip.contains(&index) {
                continue;
            }
            let (a, b) = (key(&wire.start), key(&wire.end));
            if a == at || b == at {
                probe.wire_ends += 1;
            } else if point_on_segment(at, a, b) {
                probe.interior = true;
            }
        }
        let near = |kind, len: usize| self.grid.near(kind, at, at, len);
        probe.label = near(Kind::Label, self.labels.len())
            .into_iter()
            .map(|i| &self.labels[i])
            .any(|(uuid, p)| key(p) == at && !self.selected.contains(uuid));
        probe.no_connect = near(Kind::NoConnect, self.no_connects.len())
            .into_iter()
            .map(|i| &self.no_connects[i])
            .any(|(_, p)| key(p) == at);
        probe.stationary_pin = near(Kind::Tip, self.tips.len())
            .into_iter()
            .map(|i| &self.tips[i])
            .any(|(owner, k)| *k == at && !self.moving.contains(owner));
        probe.junctions = near(Kind::Junction, self.junctions.len())
            .into_iter()
            .filter(|&i| {
                let (uuid, p) = &self.junctions[i];
                key(p) == at && !self.selected.contains(uuid)
            })
            .collect();
        probe
    }

    /// The keys of the labels, dots, no-connects and pin tips that can meet
    /// the box `a`-`b`.
    fn points_near(&self, a: Key, b: Key) -> Vec<Key> {
        let of = |kind, list: &[(Uuid, Point)]| -> Vec<Key> {
            (self.grid.near(kind, a, b, list.len()).into_iter())
                .map(|i| key(&list[i].1))
                .collect()
        };
        let mut keys = of(Kind::Label, &self.labels);
        keys.extend(of(Kind::Junction, &self.junctions));
        keys.extend(of(Kind::NoConnect, &self.no_connects));
        keys.extend(
            (self.grid.near(Kind::Tip, a, b, self.tips.len()).into_iter()).map(|i| self.tips[i].1),
        );
        keys
    }

    /// True when `seg` would touch something foreign anywhere but at the
    /// keys in `allowed`; wires in `ignore` are part of the same net.
    pub(super) fn collides(&self, seg: (Point, Point), allowed: &[Key], ignore: &[usize]) -> bool {
        let (a, b) = (key(&seg.0), key(&seg.1));
        let free = |k: Key| !allowed.contains(&k);
        // A route lying along a locked wire would stack on it, or leave a pin
        // on its side with no wire end there: not a route.
        let probe = Wire {
            start: seg.0,
            end: seg.1,
            ..Default::default()
        };
        let near = self.wires_near(a, b);
        if near
            .iter()
            .any(|(_, wire)| wire.locked && overlap::wires_overlap(&probe, wire))
        {
            return true;
        }
        for (index, wire) in near {
            if ignore.contains(&index) {
                continue;
            }
            let (wa, wb) = (key(&wire.start), key(&wire.end));
            let touches = [wa, wb]
                .into_iter()
                .any(|end| free(end) && point_on_segment(end, a, b))
                || [a, b]
                    .into_iter()
                    .any(|end| free(end) && point_on_segment(end, wa, wb));
            if touches {
                return true;
            }
        }
        self.points_near(a, b)
            .into_iter()
            .any(|k| free(k) && point_on_segment(k, a, b))
    }

    /// Anything other than the endpoints strictly inside the span `a`-`b`
    /// (other wires' ends, labels, dots, pins; `except` is the key of the
    /// pin the wire is following): shortening that wire would orphan it.
    pub(super) fn anchored_inside(
        &self,
        a: &Point,
        b: &Point,
        skip: &[usize],
        except: Option<Key>,
    ) -> bool {
        let (ka, kb) = (key(a), key(b));
        let inside =
            |k: Key| k != ka && k != kb && Some(k) != except && point_on_segment(k, ka, kb);
        self.wires_near(ka, kb).into_iter().any(|(index, w)| {
            !skip.contains(&index) && (inside(key(&w.start)) || inside(key(&w.end)))
        }) || self.points_near(ka, kb).into_iter().any(inside)
    }

    /// Unselected, unfinished wires with an end on `at`: `(index, end is start)`.
    pub(super) fn attached(&self, at: Key) -> Vec<(usize, bool)> {
        self.wires_near(at, at)
            .into_iter()
            .filter(|(_, w)| !self.selected.contains(&w.uuid))
            .flat_map(|(index, w)| {
                let mut ends = Vec::new();
                if key(&w.start) == at && !self.placed.contains(&(index, true)) {
                    ends.push((index, true));
                }
                if key(&w.end) == at
                    && key(&w.start) != at
                    && !self.placed.contains(&(index, false))
                {
                    ends.push((index, false));
                }
                ends
            })
            .collect()
    }
}

#[cfg(test)]
mod collides_tests {
    use super::*;
    use signex_types::schematic::SchematicSheet;

    fn env_with(locked: bool) -> Env {
        let sheet = SchematicSheet {
            wires: vec![Wire {
                uuid: Uuid::now_v7(),
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
                locked,
                ..Default::default()
            }],
            ..Default::default()
        };
        Env::new(
            &sheet,
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            Policy::default(),
        )
    }

    /// A route along a wire is rejected only when that wire is locked (the
    /// ends of the route are allowed, so nothing else is in the way).
    #[test]
    fn a_route_along_a_locked_wire_collides_and_along_a_free_one_does_not() {
        let seg = (Point::new(2.0, 0.0), Point::new(8.0, 0.0));
        let allowed = [key(&seg.0), key(&seg.1)];
        assert!(env_with(true).collides(seg, &allowed, &[]));
        assert!(!env_with(false).collides(seg, &allowed, &[]));
    }
}
