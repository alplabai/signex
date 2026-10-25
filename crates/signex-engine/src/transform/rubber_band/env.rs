//! The working copy and the per-tip routing of the rubber-band pass.

use std::collections::{BTreeMap, HashSet};

use signex_net::{point_on_segment, pt_key, uf::Key};
use signex_types::schematic::{
    Point, SchematicSheet, SelectedItem, SelectedKind, SymbolTransform, Wire,
};
use uuid::Uuid;

use super::{Follow, Move, Policy};

fn same_delta(a: &Move, b: &Move) -> bool {
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
    pub(super) fn new(
        document: &SchematicSheet,
        selected: HashSet<Uuid>,
        moving: HashSet<Uuid>,
        old_tips: HashSet<Key>,
        policy: Policy,
    ) -> Self {
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

    pub(super) fn write_back(self, document: &mut SchematicSheet) {
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
            || self.no_connects.iter().any(|(_, p)| on(p))
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
            || self.no_connects.iter().any(|(_, p)| inside(key(p)))
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
    pub(super) fn translate_whole_wires(
        &mut self,
        by_tip: &BTreeMap<Key, Vec<&Move>>,
        out: &mut Follow,
    ) -> Vec<Key> {
        let mut translated = Vec::new();
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
            if [ks, ke]
                .iter()
                .any(|k| self.policy.forced.contains(k) || self.policy.plain.contains(k))
            {
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
            self.edited.insert(index);
            out.touched
                .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
            translated.extend([ks, ke]);
        }
        translated
    }

    pub(super) fn reroute_tip(&mut self, at: Key, group: &[&Move], out: &mut Follow) {
        let attached = self.attached(at);
        let skip: Vec<usize> = attached.iter().map(|(index, _)| *index).collect();
        let mut probe = self.probe(at, &skip);
        let single_target = group.iter().all(|m| key(&m.new) == key(&group[0].new));
        let plain = self.policy.plain.contains(&at);
        if single_target && !plain && !probe.stationary_pin {
            // A no-connect marks the pin: it moves with the tip.
            for (uuid, position) in &mut self.no_connects {
                if key(position) == at && !self.selected.contains(uuid) {
                    *position = group[0].new;
                    out.touched
                        .push(SelectedItem::new(*uuid, SelectedKind::NoConnect));
                    probe.no_connect = false;
                }
            }
        }
        // A child-sheet pin is anchored to the wire it sits on, interior or end.
        let on_wire = probe.interior && group.iter().any(|m| m.sheet_pin);
        let connected =
            !attached.is_empty() || probe.label || !probe.junctions.is_empty() || on_wire;
        if !connected {
            return;
        }
        if plain {
            return;
        }
        let pinned_dot = !probe.junctions.is_empty() && probe.interior;
        let anchored = self.policy.forced.contains(&at)
            || on_wire
            || probe.stationary_pin
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
        // New wires copy the attached wire's stroke and unknown columns, like
        // the segments a bend adds.
        let template = attached
            .first()
            .map(|(index, _)| self.wires[*index].clone());
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
            // A tip with no collision-free route stays as it is; the guard sees
            // the broken net and takes it down the ladder.
            self.add_path(
                group[0].old,
                target,
                orientation,
                template.as_ref(),
                &skip,
                out,
            );
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
        if self.old_tips.contains(&kf) {
            return None;
        }
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
        self.edited.insert(index);
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
        template: Option<&Wire>,
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
                let mut wire = template.cloned().unwrap_or_else(|| Wire {
                    uuid: Uuid::nil(),
                    start: a,
                    end: b,
                    stroke_width: 0.0,
                    unknown_columns: Default::default(),
                });
                wire.uuid = Uuid::new_v4();
                wire.start = a;
                wire.end = b;
                out.touched
                    .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
                self.wires.push(wire);
                let index = self.wires.len() - 1;
                self.placed.insert((index, true));
                self.placed.insert((index, false));
                self.edited.insert(index);
            }
            return true;
        }
        false
    }

    /// Drop exact duplicates and merge collinear overlaps between the wires
    /// this pass touched and any other wire; the older or untouched wire
    /// survives with the union span.
    pub(super) fn merge_overlaps(&mut self, out: &mut Follow) {
        loop {
            let mut found = None;
            let mut edited: Vec<usize> = self.edited.iter().copied().collect();
            edited.sort_unstable();
            'search: for i in edited {
                if i >= self.wires.len() || self.removed.contains(&i) {
                    continue;
                }
                for (j, other) in self.live() {
                    if j != i && overlap_span(&self.wires[i], other).is_some() {
                        found = Some((i, j));
                        break 'search;
                    }
                }
            }
            let Some((i, j)) = found else { return };
            let (survivor, loser) = match (self.edited.contains(&i), self.edited.contains(&j)) {
                (true, false) => (j, i),
                (false, true) => (i, j),
                _ => (i.min(j), i.max(j)),
            };
            // Re-cut the union at every original end, so each end that mattered
            // (a pin tip, a label, a neighbour) is still the end of a wire.
            let pieces = overlap_pieces(&self.wires[survivor], &self.wires[loser]);
            let template = self.wires[survivor].clone();
            let oriented = key(&template.start) <= key(&template.end);
            let mut pieces = pieces.into_iter();
            if let Some((lo, hi)) = pieces.next() {
                let wire = &mut self.wires[survivor];
                (wire.start, wire.end) = if oriented { (lo, hi) } else { (hi, lo) };
            }
            for (lo, hi) in pieces {
                let mut wire = template.clone();
                wire.uuid = Uuid::new_v4();
                (wire.start, wire.end) = if oriented { (lo, hi) } else { (hi, lo) };
                out.touched
                    .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
                self.wires.push(wire);
                self.edited.insert(self.wires.len() - 1);
            }
            self.removed.insert(loser);
            self.edited.insert(survivor);
            for index in [survivor, loser] {
                out.touched.push(SelectedItem::new(
                    self.wires[index].uuid,
                    SelectedKind::Wire,
                ));
            }
        }
    }
}

/// True when `a` and `b` lie on one axis-aligned line and share a stretch of
/// positive length, or are the same diagonal segment.
fn overlap_span(a: &Wire, b: &Wire) -> Option<()> {
    let (a0, a1, b0, b1) = (key(&a.start), key(&a.end), key(&b.start), key(&b.end));
    if (a0 == b0 && a1 == b1) || (a0 == b1 && a1 == b0) {
        return (a0 != a1).then_some(());
    }
    let horizontal = a0.1 == a1.1 && b0.1 == b1.1 && a0.1 == b0.1;
    let vertical = a0.0 == a1.0 && b0.0 == b1.0 && a0.0 == b0.0;
    let pick = |p: Key| if horizontal { p.0 } else { p.1 };
    if !(horizontal || vertical) {
        return None;
    }
    let (alo, ahi) = (pick(a0).min(pick(a1)), pick(a0).max(pick(a1)));
    let (blo, bhi) = (pick(b0).min(pick(b1)), pick(b0).max(pick(b1)));
    (alo.max(blo) < ahi.min(bhi)).then_some(())
}

/// The union of two overlapping wires, cut at every one of their ends: the
/// pieces (low, high) cover it exactly once.
fn overlap_pieces(a: &Wire, b: &Wire) -> Vec<(Point, Point)> {
    let horizontal = key(&a.start).1 == key(&a.end).1;
    let along = |p: &Point| if horizontal { p.x } else { p.y };
    let mut ends = vec![a.start, a.end, b.start, b.end];
    ends.sort_by(|p, q| along(p).total_cmp(&along(q)));
    ends.dedup_by(|p, q| key(p) == key(q));
    ends.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

#[cfg(test)]
pub(crate) fn wires_overlap(a: &Wire, b: &Wire) -> bool {
    overlap_span(a, b).is_some()
}
