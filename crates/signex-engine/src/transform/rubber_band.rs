//! Rubber-band wires: keep connections when a move carries pins away.
//!
//! A plain move translates the selected items and nothing else, so a wire
//! ending on a moved symbol's pin is left behind and the connection silently
//! breaks (#963). With `keep_wires` the engine runs this pass inside the same
//! command, so the move and the wire edits are one history step and one
//! patch.
//!
//! # The rule: never change connectivity or net names
//!
//! Every moved pin whose old tip was connected to something is reconnected
//! at its new tip, and nothing else changes connectivity. Per old tip `K`
//! (matched in the netlist's own 1 um key space, so "same point" cannot
//! disagree with the netlist):
//!
//! * **Anchored** - something that does not move also holds `K`: another
//!   symbol's pin tip, a net label, a sheet pin, a junction that a stationary
//!   wire passes through, a wire that already moved away from the pair. The
//!   wires and labels stay exactly where they are and a new orthogonal path
//!   `K -> corner -> new tip` is added, so the old net keeps every member and
//!   every name and the pin joins it at its new position.
//! * **Free** - only the moved pin and unselected wire ends (plus movable
//!   junction dots) sit on `K`. Each attached wire end follows the pin:
//!   - a wire between two moved pins that move alike translates whole;
//!   - an orthogonal wire stays orthogonal: if its far joint is a plain
//!     elbow (exactly two wire ends, nothing else) the elbow slides along
//!     its other leg, so repeated moves never pile up segments; otherwise a
//!     corner is inserted. The preferred corner keeps the wire's direction
//!     at the fixed end (horizontal wire: `(new.x, fixed.y)`, vertical wire:
//!     `(fixed.x, new.y)`), the alternative is the other L;
//!   - a result of zero length is dropped when something else still holds
//!     the point, and otherwise left alone (the pin lands on that end).
//!   Dots on `K` move with the tip.
//!
//! # Collisions
//!
//! A new or stretched span may not touch any foreign wire (end or interior),
//! pin tip, label, junction or no-connect, except at the points it is meant
//! to join (the fixed end, the old tip, the new tip). The candidates are
//! tried in order - slide, preferred corner, other corner, then (free tips)
//! the anchored path with both corners. If every candidate collides the pin is
//! moved without rubber-banding, and a warning names it. A wire with a label
//! or another wire's end on its interior is never shortened or translated;
//! that makes the tip anchored instead.
//!
//! Which items drag wires: pins of moved symbols (power ports included,
//! unit-filtered), pins of moved child sheets and individually moved sheet
//! pins. Labels, junctions, no-connects and text never drag wires on their
//! own; wires that are themselves selected move as they always did.

use std::collections::{BTreeMap, HashSet};

use signex_net::{point_on_segment, pt_key, uf::Key};
use signex_types::schematic::{
    Point, SchematicSheet, SelectedItem, SelectedKind, SymbolTransform, Wire,
};
use uuid::Uuid;

/// A pin that is about to move, remembered by where its tip is now.
#[derive(Debug, Clone)]
pub struct PinTip {
    owner: Uuid,
    source: TipSource,
    old: Point,
    label: String,
}

#[derive(Debug, Clone, Copy)]
enum TipSource {
    /// A library pin at this local position of the owning symbol.
    SymbolPin(Point),
    /// A child sheet's pin (owner is the pin's uuid).
    SheetPin,
}

/// What the pass changed: wires and dots for the patch, and the pins it could
/// not keep connected.
#[derive(Debug, Default)]
pub struct Follow {
    pub touched: Vec<SelectedItem>,
    pub warnings: Vec<String>,
}

/// Pin tips carried by `items`, read before they move.
pub fn moved_pin_tips(document: &SchematicSheet, items: &[SelectedItem]) -> Vec<PinTip> {
    let mut tips = Vec::new();
    for item in items {
        match item.kind {
            SelectedKind::Symbol => {
                let Some(symbol) = document.symbols.iter().find(|s| s.uuid == item.uuid) else {
                    continue;
                };
                let Some(lib) = document.lib_symbols.get(&symbol.lib_id) else {
                    continue;
                };
                let xform = SymbolTransform::from_symbol(symbol);
                for lib_pin in &lib.pins {
                    if lib_pin.unit != 0 && lib_pin.unit != symbol.unit {
                        continue;
                    }
                    let name = if lib_pin.pin.number.is_empty() {
                        &lib_pin.pin.name
                    } else {
                        &lib_pin.pin.number
                    };
                    tips.push(PinTip {
                        owner: symbol.uuid,
                        source: TipSource::SymbolPin(lib_pin.pin.position),
                        old: xform.apply(lib_pin.pin.position),
                        label: format!("{} pin {name}", symbol.reference),
                    });
                }
            }
            SelectedKind::ChildSheet => {
                if let Some(child) = document.child_sheets.iter().find(|c| c.uuid == item.uuid) {
                    for pin in &child.pins {
                        tips.push(sheet_pin_tip(&child.name, pin));
                    }
                }
            }
            SelectedKind::SheetPin => {
                for child in &document.child_sheets {
                    if let Some(pin) = child.pins.iter().find(|p| p.uuid == item.uuid) {
                        tips.push(sheet_pin_tip(&child.name, pin));
                    }
                }
            }
            _ => {}
        }
    }
    tips
}

fn sheet_pin_tip(sheet_name: &str, pin: &signex_types::schematic::SheetPin) -> PinTip {
    PinTip {
        owner: pin.uuid,
        source: TipSource::SheetPin,
        old: pin.position,
        label: format!("{sheet_name} pin {}", pin.name),
    }
}

/// One pin that really moved.
struct Move {
    old: Point,
    new: Point,
    label: String,
}

fn resolve_moves(document: &SchematicSheet, tips: &[PinTip]) -> (Vec<Move>, HashSet<Uuid>) {
    let mut moves = Vec::new();
    let mut owners = HashSet::new();
    for tip in tips {
        let new = match tip.source {
            TipSource::SymbolPin(local) => document
                .symbols
                .iter()
                .find(|s| s.uuid == tip.owner)
                .map(|s| SymbolTransform::from_symbol(s).apply(local)),
            TipSource::SheetPin => document
                .child_sheets
                .iter()
                .flat_map(|c| c.pins.iter())
                .find(|p| p.uuid == tip.owner)
                .map(|p| p.position),
        };
        let Some(new) = new else { continue };
        if pt_key(&new) == pt_key(&tip.old) {
            continue;
        }
        owners.insert(tip.owner);
        moves.push(Move {
            old: tip.old,
            new,
            label: tip.label.clone(),
        });
    }
    (moves, owners)
}

/// Drag the wires attached to `tips` along with a move that has already
/// been applied to `document`. See the module docs for the rules.
pub fn follow_moved_pins(
    document: &mut SchematicSheet,
    tips: &[PinTip],
    items: &[SelectedItem],
) -> Follow {
    let (moves, moving) = resolve_moves(document, tips);
    if moves.is_empty() {
        return Follow::default();
    }
    let selected: HashSet<Uuid> = items.iter().map(|item| item.uuid).collect();
    let mut env = Env::new(document, selected, moving);
    let mut out = Follow::default();

    let mut by_tip: BTreeMap<Key, Vec<&Move>> = BTreeMap::new();
    for mv in &moves {
        by_tip.entry(pt_key(&mv.old)).or_default().push(mv);
    }

    env.translate_whole_wires(&by_tip, &mut out);
    for (key, group) in &by_tip {
        env.reroute_tip(*key, group, &mut out);
    }
    env.write_back(document);
    out
}

fn same_delta(a: &Move, b: &Move) -> bool {
    pt_key(&Point::new(a.new.x - a.old.x, a.new.y - a.old.y))
        == pt_key(&Point::new(b.new.x - b.old.x, b.new.y - b.old.y))
}

/// Working copy of everything a tip can touch. Pin tips never change during
/// the pass; wires and dots do.
struct Env {
    wires: Vec<Wire>,
    removed: HashSet<usize>,
    /// Wire ends already put at their final place (`(index, is start)`); a
    /// later tip must not mistake them for ends still waiting on a pin.
    placed: HashSet<(usize, bool)>,
    labels: Vec<(Uuid, Point)>,
    junctions: Vec<(Uuid, Point)>,
    no_connects: Vec<Point>,
    tips: Vec<(Uuid, Key)>,
    selected: HashSet<Uuid>,
    moving: HashSet<Uuid>,
}

/// Everything on an old tip other than the wires attached to it.
#[derive(Default)]
struct Probe {
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

struct Plan {
    replace: Vec<usize>,
    segs: Vec<(Point, Point)>,
    /// Keys of the ends that stay joined to what they were joined to; every
    /// other end of an edited wire is final.
    joins: Vec<Key>,
}

fn key(p: &Point) -> Key {
    pt_key(p)
}

fn zero_length(a: &Point, b: &Point) -> bool {
    key(a) == key(b)
}

impl Env {
    fn new(document: &SchematicSheet, selected: HashSet<Uuid>, moving: HashSet<Uuid>) -> Self {
        let mut tips = Vec::new();
        for symbol in &document.symbols {
            let Some(lib) = document.lib_symbols.get(&symbol.lib_id) else {
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
            no_connects: document.no_connects.iter().map(|n| n.position).collect(),
            tips,
            selected,
            moving,
        }
    }

    fn write_back(self, document: &mut SchematicSheet) {
        let Self {
            wires,
            removed,
            junctions,
            ..
        } = self;
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

    fn live(&self) -> impl Iterator<Item = (usize, &Wire)> {
        self.wires
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.removed.contains(index))
    }

    /// Everything stationary on `at`, ignoring the wire indices in `skip`.
    fn probe(&self, at: Key, skip: &[usize]) -> Probe {
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
        probe.no_connect = self.no_connects.iter().any(|p| key(p) == at);
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
    fn collides(&self, seg: (Point, Point), allowed: &[Key], ignore: &[usize]) -> bool {
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
            || self.no_connects.iter().any(on)
            || self
                .tips
                .iter()
                .any(|(_, k)| free(*k) && point_on_segment(*k, a, b))
    }

    /// Anything other than the endpoints strictly inside the span `a`-`b`
    /// (other wires' ends, labels, dots, pins; `except` is the key of the
    /// pin the wire is following): shortening that wire would orphan it.
    fn anchored_inside(&self, a: &Point, b: &Point, skip: &[usize], except: Option<Key>) -> bool {
        let (ka, kb) = (key(a), key(b));
        let inside =
            |k: Key| k != ka && k != kb && Some(k) != except && point_on_segment(k, ka, kb);
        self.live().any(|(index, w)| {
            !skip.contains(&index) && (inside(key(&w.start)) || inside(key(&w.end)))
        }) || self.labels.iter().any(|(_, p)| inside(key(p)))
            || self.junctions.iter().any(|(_, p)| inside(key(p)))
            || self.no_connects.iter().any(|p| inside(key(p)))
            || self.tips.iter().any(|(_, k)| inside(*k))
    }

    /// Unselected, unfinished wires with an end on `at`: `(index, end is start)`.
    fn attached(&self, at: Key) -> Vec<(usize, bool)> {
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

    /// A wire whose two ends sit on moved pins that move alike translates
    /// whole, provided nothing stationary holds either tip or hangs on it.
    fn translate_whole_wires(&mut self, by_tip: &BTreeMap<Key, Vec<&Move>>, out: &mut Follow) {
        for index in 0..self.wires.len() {
            let wire = &self.wires[index];
            if self.removed.contains(&index) || self.selected.contains(&wire.uuid) {
                continue;
            }
            let (start, end) = (wire.start, wire.end);
            let (ks, ke) = (key(&start), key(&end));
            let (Some(gs), Some(ge)) = (by_tip.get(&ks), by_tip.get(&ke)) else {
                continue;
            };
            if ks == ke || !gs.iter().chain(ge.iter()).all(|m| same_delta(m, gs[0])) {
                continue;
            }
            let (ddx, ddy) = (gs[0].new.x - gs[0].old.x, gs[0].new.y - gs[0].old.y);
            let (ns, ne) = (
                Point::new(start.x + ddx, start.y + ddy),
                Point::new(end.x + ddx, end.y + ddy),
            );
            let held = |env: &Self, k: Key| {
                let p = env.probe(k, &[index]);
                p.label || p.stationary_pin || p.no_connect || p.interior
            };
            if held(self, ks)
                || held(self, ke)
                || self.anchored_inside(&start, &end, &[index], None)
                || self.collides((ns, ne), &[key(&ns), key(&ne)], &[index])
            {
                continue;
            }
            let wire = &mut self.wires[index];
            wire.start = ns;
            wire.end = ne;
            self.placed.insert((index, true));
            self.placed.insert((index, false));
            out.touched
                .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
        }
    }

    fn reroute_tip(&mut self, at: Key, group: &[&Move], out: &mut Follow) {
        let attached = self.attached(at);
        let skip: Vec<usize> = attached.iter().map(|(index, _)| *index).collect();
        let probe = self.probe(at, &skip);
        let connected = !attached.is_empty() || probe.label || !probe.junctions.is_empty();
        if !connected {
            return;
        }
        let single_target = group.iter().all(|m| key(&m.new) == key(&group[0].new));
        let pinned_dot = !probe.junctions.is_empty() && probe.interior;
        let anchored = probe.stationary_pin
            || probe.wire_ends > 0
            || probe.label
            || (probe.no_connect && !attached.is_empty())
            || pinned_dot
            || !single_target;
        let new = group[0].new;

        if !anchored && single_target && self.try_follow(at, &attached, new, out) {
            for index in probe.junctions {
                self.junctions[index].1 = new;
                out.touched.push(SelectedItem::new(
                    self.junctions[index].0,
                    SelectedKind::Junction,
                ));
            }
            return;
        }
        let stroke = attached
            .first()
            .map(|(index, _)| self.wires[*index].stroke_width)
            .unwrap_or(0.0);
        let orientation = attached.first().map(|(index, _)| {
            let w = &self.wires[*index];
            key(&w.start).1 == key(&w.end).1
        });
        let mut targets: Vec<Point> = Vec::new();
        for mv in group {
            if !targets.iter().any(|p| key(p) == key(&mv.new)) {
                targets.push(mv.new);
            }
        }
        for target in targets {
            if !self.add_path(group[0].old, target, orientation, stroke, &skip, out) {
                let names: Vec<&str> = group
                    .iter()
                    .filter(|m| key(&m.new) == key(&target))
                    .map(|m| m.label.as_str())
                    .collect();
                out.warnings.push(format!(
                    "could not keep {} connected without crossing another net",
                    names.join(", ")
                ));
            }
        }
    }

    /// Reroute every attached wire end to `new`; all or nothing.
    fn try_follow(
        &mut self,
        at: Key,
        attached: &[(usize, bool)],
        new: Point,
        out: &mut Follow,
    ) -> bool {
        let saved = (
            self.wires.clone(),
            self.removed.clone(),
            self.placed.clone(),
            out.touched.len(),
        );
        let skip: Vec<usize> = attached.iter().map(|(index, _)| *index).collect();
        for &(index, follow_start) in attached {
            if self.removed.contains(&index) {
                continue;
            }
            if !self.follow_wire(at, index, follow_start, new, &skip, out) {
                (self.wires, self.removed, self.placed, _) = saved;
                out.touched.truncate(saved.3);
                return false;
            }
        }
        true
    }

    fn follow_wire(
        &mut self,
        at: Key,
        index: usize,
        follow_start: bool,
        new: Point,
        group: &[usize],
        out: &mut Follow,
    ) -> bool {
        let wire = &self.wires[index];
        let (fixed, moving) = if follow_start {
            (wire.end, wire.start)
        } else {
            (wire.start, wire.end)
        };
        let kn = key(&new);
        if self.anchored_inside(&fixed, &moving, &[index], Some(kn)) {
            return false;
        }
        let kf = key(&fixed);
        let km = key(&moving);
        let horizontal = kf.1 == km.1 && kf.0 != km.0;
        let vertical = kf.0 == km.0 && kf.1 != km.1;
        let mut plans: Vec<Plan> = Vec::new();
        let mut extra_allowed: Vec<Key> = Vec::new();
        if horizontal || vertical {
            let stays_straight = if horizontal {
                kn.1 == kf.1
            } else {
                kn.0 == kf.0
            };
            if stays_straight {
                plans.push(Plan {
                    replace: vec![index],
                    segs: vec![(fixed, new)],
                    joins: vec![kf],
                });
            } else {
                if let Some((plan, far)) = self.slide_plan(index, fixed, new, horizontal) {
                    extra_allowed.push(key(&far));
                    plans.push(plan);
                }
                let (c1, c2) = if horizontal {
                    (Point::new(new.x, fixed.y), Point::new(fixed.x, new.y))
                } else {
                    (Point::new(fixed.x, new.y), Point::new(new.x, fixed.y))
                };
                for corner in [c1, c2] {
                    plans.push(Plan {
                        replace: vec![index],
                        segs: vec![(fixed, corner), (corner, new)],
                        joins: vec![kf],
                    });
                }
            }
        } else {
            plans.push(Plan {
                replace: vec![index],
                segs: vec![(fixed, new)],
                joins: vec![kf],
            });
        }
        for mut plan in plans {
            plan.segs.retain(|(a, b)| !zero_length(a, b));
            let mut ignore: Vec<usize> = group.to_vec();
            ignore.extend(&plan.replace);
            // `at` (the old tip) holds nothing but the group's wire ends and
            // dots that move with it, so a corner may sit there.
            let mut allowed = vec![kf, kn, at];
            allowed.extend(&extra_allowed);
            if plan.segs.is_empty() {
                let others = self.probe(kf, &ignore).anything()
                    || self
                        .live()
                        .any(|(i, w)| i != index && (key(&w.start) == kf || key(&w.end) == kf));
                if others {
                    self.removed.insert(index);
                    out.touched.push(SelectedItem::new(
                        self.wires[index].uuid,
                        SelectedKind::Wire,
                    ));
                }
                return true;
            }
            if plan
                .segs
                .iter()
                .any(|seg| self.collides(*seg, &allowed, &ignore))
            {
                continue;
            }
            self.apply_plan(plan, index, out);
            return true;
        }
        false
    }

    /// The elbow-slide plan: the far joint of `index` is a plain two-wire
    /// elbow, so it moves along its other leg and no segment is added.
    fn slide_plan(
        &self,
        index: usize,
        fixed: Point,
        new: Point,
        horizontal: bool,
    ) -> Option<(Plan, Point)> {
        let kf = key(&fixed);
        let probe = self.probe(kf, &[index]);
        if probe.wire_ends != 1
            || probe.interior
            || probe.label
            || probe.no_connect
            || probe.stationary_pin
            || !probe.junctions.is_empty()
        {
            return None;
        }
        let (leg, leg_wire) = self
            .live()
            .find(|(i, w)| *i != index && (key(&w.start) == kf || key(&w.end) == kf))?;
        let far = if key(&leg_wire.start) == kf {
            leg_wire.end
        } else {
            leg_wire.start
        };
        let kfar = key(&far);
        let perpendicular = if horizontal {
            kfar.0 == kf.0 && kfar.1 != kf.1
        } else {
            kfar.1 == kf.1 && kfar.0 != kf.0
        };
        if !perpendicular
            || self.placed.contains(&(leg, true))
            || self.placed.contains(&(leg, false))
            || self.selected.contains(&leg_wire.uuid)
        {
            return None;
        }
        if self.anchored_inside(&far, &fixed, &[leg, index], Some(key(&new))) {
            return None;
        }
        let elbow = if horizontal {
            Point::new(fixed.x, new.y)
        } else {
            Point::new(new.x, fixed.y)
        };
        Some((
            Plan {
                replace: vec![leg, index],
                segs: vec![(far, elbow), (elbow, new)],
                joins: vec![kfar],
            },
            far,
        ))
    }

    /// Reuse the replaced wires for `plan.segs` in order, drop surplus
    /// wires, and mint wires for surplus segments (copying `template`).
    fn apply_plan(&mut self, plan: Plan, template: usize, out: &mut Follow) {
        let Plan {
            replace,
            segs,
            joins,
        } = plan;
        let mut segs = segs.into_iter();
        for &index in &replace {
            match segs.next() {
                Some((a, b)) => {
                    let wire = &mut self.wires[index];
                    wire.start = a;
                    wire.end = b;
                    out.touched
                        .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
                    self.place_ends(index, &joins);
                }
                None => {
                    self.removed.insert(index);
                    out.touched.push(SelectedItem::new(
                        self.wires[index].uuid,
                        SelectedKind::Wire,
                    ));
                }
            }
        }
        for (a, b) in segs {
            let mut wire = self.wires[template].clone();
            wire.uuid = Uuid::new_v4();
            wire.start = a;
            wire.end = b;
            out.touched
                .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
            self.wires.push(wire);
            self.place_ends(self.wires.len() - 1, &joins);
        }
    }

    /// Mark the ends of `index` as final, except those still joined to
    /// their old neighbours (`joins`).
    fn place_ends(&mut self, index: usize, joins: &[Key]) {
        let wire = &self.wires[index];
        for (is_start, end) in [(true, wire.start), (false, wire.end)] {
            if !joins.contains(&key(&end)) {
                self.placed.insert((index, is_start));
            }
        }
    }

    /// Add `old -> corner -> new` as fresh wires; the old tip stays wired.
    fn add_path(
        &mut self,
        old: Point,
        new: Point,
        horizontal_first: Option<bool>,
        stroke: f64,
        group: &[usize],
        out: &mut Follow,
    ) -> bool {
        let across = Point::new(new.x, old.y);
        let down = Point::new(old.x, new.y);
        let corners = if horizontal_first == Some(false) {
            [down, across]
        } else {
            [across, down]
        };
        let allowed = [key(&old), key(&new)];
        for corner in corners {
            let mut segs = vec![(old, corner), (corner, new)];
            segs.retain(|(a, b)| !zero_length(a, b));
            if segs.is_empty() || segs.iter().any(|seg| self.collides(*seg, &allowed, group)) {
                continue;
            }
            for (a, b) in segs {
                let wire = Wire {
                    uuid: Uuid::new_v4(),
                    start: a,
                    end: b,
                    stroke_width: stroke,
                    unknown_columns: Default::default(),
                };
                out.touched
                    .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
                self.wires.push(wire);
                let index = self.wires.len() - 1;
                self.placed.insert((index, true));
                self.placed.insert((index, false));
            }
            return true;
        }
        false
    }
}
