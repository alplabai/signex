//! Per-tip routing: what happens to the wires on one old pin tip.

use std::collections::BTreeMap;

use signex_net::uf::Key;
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

use super::{Env, key, same_delta, zero_length};
use crate::transform::rubber_band::{Follow, Move, WireJoin};

impl Env {
    /// A wire whose two ends sit on moved pins that move alike translates
    /// whole, provided nothing stationary holds either tip or hangs on it.
    pub(in crate::transform::rubber_band) fn translate_whole_wires(
        &mut self,
        by_tip: &BTreeMap<Key, Vec<&Move>>,
        out: &mut Follow,
    ) -> Vec<Key> {
        let mut translated = Vec::new();
        // Only a wire with an end on a moving pin's old tip can translate.
        let mut candidates: Vec<usize> =
            by_tip.keys().flat_map(|k| self.grid_wires_at(*k)).collect();
        candidates.sort_unstable();
        candidates.dedup();
        for index in candidates {
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
            self.refile_wire(index);
            translated.extend([ks, ke]);
        }
        translated
    }

    /// Reroute the wires on the old tip `at` for the pins in `group`.
    pub(in crate::transform::rubber_band) fn reroute_tip(
        &mut self,
        at: Key,
        group: &[&Move],
        out: &mut Follow,
    ) {
        let attached = self.attached(at);
        let skip: Vec<usize> = attached.iter().map(|(index, _)| *index).collect();
        let mut probe = self.probe(at, &skip);
        let wire = group.iter().find_map(|m| m.wire);
        if matches!(wire, Some(WireJoin::Grip(_))) {
            // A grip drags its end off whatever it sat on; only the wires
            // attached there come along.
            probe.stationary_pin = false;
            probe.label = false;
            probe.interior = false;
        }
        let single_target = group.iter().all(|m| key(&m.new) == key(&group[0].new));
        let plain = self.policy.plain.contains(&at);
        if single_target && !plain && !probe.stationary_pin {
            self.carry_no_connects(at, group[0].new, &mut probe, out);
        }
        // A child-sheet pin is anchored to the wire it sits on, interior or end.
        let on_wire = probe.interior && group.iter().any(|m| m.sheet_pin);
        // A wire end on a pin is a connection; a pin tip on a pin tip is not.
        let pin_held = probe.stationary_pin && wire.is_some();
        // A locked wire is never stretched: it keeps its place and the carried
        // point gets a new path.
        let locked = wire.is_some() && attached.iter().any(|(i, _)| self.wires[*i].locked);
        let connected = !attached.is_empty()
            || probe.label
            || !probe.junctions.is_empty()
            || on_wire
            || pin_held;
        if !connected || plain {
            return;
        }
        // An attached wire that editing would get a T dot it never had is left
        // alone (see `edit_would_dot`): the point stays as the move left it.
        if wire.is_some() && self.carried_point_bails(at) {
            return;
        }
        let new = group[0].new;
        let anchored = on_wire
            || locked
            || !single_target
            || self.holds_the_tip(at, &probe, !attached.is_empty());
        if !anchored && single_target && self.try_follow(at, &attached, new, out) {
            self.carry_dots(&probe.junctions, new, out);
            return;
        }
        // A point a dragged wire carries that another wire passes through with
        // no dot was no connection there (#107): a new path ending on it would
        // make one through a T dot, so the contact is left as the move left it.
        if wire.is_some() && probe.interior && probe.junctions.is_empty() {
            return;
        }
        self.add_paths(group, &attached, &skip, out);
    }

    /// Whether a point a dragged wire carries from `at` must be left as the
    /// move left it: editing a wire attached there, or the elbow leg at its
    /// far end that a slide would edit, could make the reconcile dot a
    /// contact that was there before (see [`Self::edit_would_dot`]).
    pub(in crate::transform::rubber_band) fn carried_point_bails(&self, at: Key) -> bool {
        self.attached(at).into_iter().any(|(index, at_start)| {
            let wire = &self.wires[index];
            let far = key(if at_start { &wire.end } else { &wire.start });
            self.edit_would_dot(index, far) || self.elbow_leg_would_dot(index, far)
        })
    }

    /// Whether editing wire `index`, whose end `far` stays put, could make the
    /// reconcile dot a T that had no dot (#107: no connection) and that the
    /// plain move would not dot: `far` inside another unselected wire, or an
    /// unselected wire's end inside this one. A selected wire's contacts are
    /// the plain move's own (it reconciles the moved wire too), and its moved
    /// ends are where the follow takes this wire.
    fn edit_would_dot(&self, index: usize, far: Key) -> bool {
        let undotted = |k: Key| self.probe(k, &[index]).junctions.is_empty();
        let foreign = |other: usize, w: &Wire| other != index && !self.selected.contains(&w.uuid);
        let far_inside = (self.wires_near(far, far).into_iter()).any(|(other, w)| {
            let (a, b) = (key(&w.start), key(&w.end));
            foreign(other, w) && far != a && far != b && signex_net::point_on_segment(far, a, b)
        });
        if far_inside && undotted(far) {
            return true;
        }
        let wire = &self.wires[index];
        let (a, b) = (key(&wire.start), key(&wire.end));
        let inside = |k: Key| k != a && k != b && signex_net::point_on_segment(k, a, b);
        (self.wires_near(a, b).into_iter())
            .filter(|(other, w)| foreign(*other, w))
            .flat_map(|(_, w)| [key(&w.start), key(&w.end)])
            .any(|k| inside(k) && undotted(k))
    }

    /// Whether the plain two-wire elbow at `far`, which a slide of wire
    /// `index` would also edit, could get such a dot.
    fn elbow_leg_would_dot(&self, index: usize, far: Key) -> bool {
        (self.wires_near(far, far).into_iter())
            .filter(|(other, w)| *other != index && (key(&w.start) == far || key(&w.end) == far))
            .any(|(leg, w)| {
                let other_end = if key(&w.start) == far {
                    key(&w.end)
                } else {
                    key(&w.start)
                };
                self.edit_would_dot(leg, other_end)
            })
    }

    /// A no-connect marks the pin: it moves with the tip.
    fn carry_no_connects(
        &mut self,
        at: Key,
        new: Point,
        probe: &mut super::Probe,
        out: &mut Follow,
    ) {
        for index in self.no_connects_near(at) {
            let (uuid, position) = &mut self.no_connects[index];
            if key(position) == at && !self.selected.contains(uuid) {
                *position = new;
                out.touched
                    .push(SelectedItem::new(*uuid, SelectedKind::NoConnect));
                probe.no_connect = false;
                self.moved_no_connects.push(index);
                self.file_no_connect(index);
            }
        }
    }

    /// Whether something that does not move also holds the old tip, so the
    /// wires on it have to stay and the pin gets a new path instead.
    fn holds_the_tip(&self, at: Key, probe: &super::Probe, has_wires: bool) -> bool {
        let pinned_dot = !probe.junctions.is_empty() && probe.interior;
        self.policy.forced.contains(&at)
            || probe.stationary_pin
            || probe.wire_ends > 0
            || probe.label
            || (probe.no_connect && has_wires)
            || pinned_dot
    }

    /// Dots on a followed tip move with it.
    fn carry_dots(&mut self, dots: &[usize], new: Point, out: &mut Follow) {
        for &index in dots {
            self.junctions[index].1 = new;
            self.moved_junctions.push(index);
            self.file_junction(index);
            out.touched.push(SelectedItem::new(
                self.junctions[index].0,
                SelectedKind::Junction,
            ));
        }
    }

    /// New paths from the old tip to each distinct new tip. A tip with no
    /// collision-free route stays as it is; the guard sees the broken net and
    /// takes it down the ladder.
    fn add_paths(
        &mut self,
        group: &[&Move],
        attached: &[(usize, bool)],
        skip: &[usize],
        out: &mut Follow,
    ) {
        // New wires copy the attached wire's stroke and unknown columns, like
        // the segments a bend adds; with none attached, the dragged wire's.
        let attached_wire = attached
            .first()
            .map(|(index, _)| self.wires[*index].clone());
        let orientation = attached_wire
            .as_ref()
            .map(|w| key(&w.start).1 == key(&w.end).1);
        let template = attached_wire.or_else(|| self.carrier(group));
        let mut targets: Vec<Point> = Vec::new();
        for mv in group {
            if !targets.iter().any(|p| key(p) == key(&mv.new)) {
                targets.push(mv.new);
            }
        }
        for target in targets {
            self.add_path(
                group[0].old,
                target,
                orientation,
                template.as_ref(),
                skip,
                out,
            );
        }
    }

    /// The selected wire whose drag carries the points of `group`, if any.
    fn carrier(&self, group: &[&Move]) -> Option<Wire> {
        let id = group.iter().find_map(|m| m.wire)?.wire();
        self.wires.iter().find(|w| w.uuid == id).cloned()
    }

    /// Add `old -> corner -> new` as fresh wires; the old tip stays wired.
    pub(super) fn add_path(
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
                    color: None,
                    uuid: Uuid::nil(),
                    start: a,
                    end: b,
                    stroke_width: 0.0,
                    unknown_columns: Default::default(),
                    locked: false,
                });
                wire.uuid = Uuid::now_v7();
                // A new leg is a fresh wire; a lock stays on the wire it was set on.
                wire.locked = false;
                wire.start = a;
                wire.end = b;
                out.touched
                    .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
                self.wires.push(wire);
                let index = self.wires.len() - 1;
                self.refile_wire(index);
                self.placed.insert((index, true));
                self.placed.insert((index, false));
                self.edited.insert(index);
            }
            return true;
        }
        false
    }
}
