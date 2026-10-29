//! Per-tip routing: what happens to the wires on one old pin tip.

use std::collections::BTreeMap;

use signex_net::uf::Key;
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

use super::{Env, key, same_delta, zero_length};
use crate::transform::rubber_band::{Follow, Move};

impl Env {
    /// A wire whose two ends sit on moved pins that move alike translates
    /// whole, provided nothing stationary holds either tip or hangs on it.
    pub(in crate::transform::rubber_band) fn translate_whole_wires(
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
        let single_target = group.iter().all(|m| key(&m.new) == key(&group[0].new));
        let plain = self.policy.plain.contains(&at);
        if single_target && !plain && !probe.stationary_pin {
            self.carry_no_connects(at, group[0].new, &mut probe, out);
        }
        // A child-sheet pin is anchored to the wire it sits on, interior or end.
        let on_wire = probe.interior && group.iter().any(|m| m.sheet_pin);
        let connected =
            !attached.is_empty() || probe.label || !probe.junctions.is_empty() || on_wire;
        if !connected || plain {
            return;
        }
        let new = group[0].new;
        let anchored =
            on_wire || !single_target || self.holds_the_tip(at, &probe, !attached.is_empty());
        if !anchored && single_target && self.try_follow(at, &attached, new, out) {
            self.carry_dots(&probe.junctions, new, out);
            return;
        }
        self.add_paths(group, &attached, &skip, out);
    }

    /// A no-connect marks the pin: it moves with the tip.
    fn carry_no_connects(
        &mut self,
        at: Key,
        new: Point,
        probe: &mut super::Probe,
        out: &mut Follow,
    ) {
        for (uuid, position) in &mut self.no_connects {
            if key(position) == at && !self.selected.contains(uuid) {
                *position = new;
                out.touched
                    .push(SelectedItem::new(*uuid, SelectedKind::NoConnect));
                probe.no_connect = false;
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
        // the segments a bend adds.
        let template = attached
            .first()
            .map(|(index, _)| self.wires[*index].clone());
        let orientation = template.as_ref().map(|w| key(&w.start).1 == key(&w.end).1);
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
                    uuid: Uuid::nil(),
                    start: a,
                    end: b,
                    stroke_width: 0.0,
                    unknown_columns: Default::default(),
                    locked: false,
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
}
