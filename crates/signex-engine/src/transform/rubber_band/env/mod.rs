//! The working copy of the rubber-band pass and the geometry questions every
//! step asks of it; routing lives in [`route`], following in [`follow`], the
//! overlap clean-up in [`overlap`].

use std::collections::HashSet;

use signex_net::{point_on_segment, pt_key, uf::Key};
use signex_types::schematic::{Point, SchematicSheet, SymbolTransform, Wire};
use uuid::Uuid;

use super::{Move, Policy};

mod follow;
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
            for lib_pin in &lib.pins {
                if lib_pin.unit == 0 || lib_pin.unit == symbol.unit {
                    tips.push((symbol.uuid, key(&xform.apply(lib_pin.pin.position))));
                }
            }
        }
        for pin in document.child_sheets.iter().flat_map(|c| c.pins.iter()) {
            tips.push((pin.uuid, key(&pin.position)));
        }
        Self {
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
        }
    }

    pub(in crate::transform::rubber_band) fn write_back(self, document: &mut SchematicSheet) {
        let Self {
            wires,
            removed,
            junctions,
            no_connects,
            ..
        } = self;
        for (uuid, position) in no_connects {
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
        for (uuid, position) in junctions {
            if let Some(dot) = document.junctions.iter_mut().find(|j| j.uuid == uuid) {
                dot.position = position;
            }
        }
    }

    pub(super) fn live(&self) -> impl Iterator<Item = (usize, &Wire)> {
        self.wires
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.removed.contains(index))
    }

    /// Everything stationary on `at`, ignoring the wire indices in `skip`.
    pub(super) fn probe(&self, at: Key, skip: &[usize]) -> Probe {
        let mut probe = Probe::default();
        for (index, wire) in self.live() {
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
        probe.label = self
            .labels
            .iter()
            .any(|(uuid, p)| key(p) == at && !self.selected.contains(uuid));
        probe.no_connect = self.no_connects.iter().any(|(_, p)| key(p) == at);
        probe.stationary_pin = self
            .tips
            .iter()
            .any(|(owner, k)| *k == at && !self.moving.contains(owner));
        probe.junctions = self
            .junctions
            .iter()
            .enumerate()
            .filter(|(_, (uuid, p))| key(p) == at && !self.selected.contains(uuid))
            .map(|(index, _)| index)
            .collect();
        probe
    }

    /// True when `seg` would touch something foreign anywhere but at the
    /// keys in `allowed`; wires in `ignore` are part of the same net.
    pub(super) fn collides(&self, seg: (Point, Point), allowed: &[Key], ignore: &[usize]) -> bool {
        let (a, b) = (key(&seg.0), key(&seg.1));
        let free = |k: Key| !allowed.contains(&k);
        for (index, wire) in self.live() {
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
        let on = |p: &Point| {
            let k = key(p);
            free(k) && point_on_segment(k, a, b)
        };
        self.labels.iter().any(|(_, p)| on(p))
            || self.junctions.iter().any(|(_, p)| on(p))
            || self.no_connects.iter().any(|(_, p)| on(p))
            || self
                .tips
                .iter()
                .any(|(_, k)| free(*k) && point_on_segment(*k, a, b))
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
        self.live().any(|(index, w)| {
            !skip.contains(&index) && (inside(key(&w.start)) || inside(key(&w.end)))
        }) || self.labels.iter().any(|(_, p)| inside(key(p)))
            || self.junctions.iter().any(|(_, p)| inside(key(p)))
            || self.no_connects.iter().any(|(_, p)| inside(key(p)))
            || self.tips.iter().any(|(_, k)| inside(*k))
    }

    /// Unselected, unfinished wires with an end on `at`: `(index, end is start)`.
    pub(super) fn attached(&self, at: Key) -> Vec<(usize, bool)> {
        self.live()
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
