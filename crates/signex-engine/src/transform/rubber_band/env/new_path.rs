//! New paths: the wires a pin gets from its old tip when the wires there
//! stay, and the order the tips of one move are rerouted in.

use std::collections::BTreeMap;

use signex_net::{point_on_segment, uf::Key};
use signex_types::corner::DragWiringMode;
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

use super::{Env, key, zero_length};
use crate::transform::rubber_band::{Follow, Move};

/// The new paths one old tip gets, kept until they are laid.
pub(in crate::transform::rubber_band) struct NewPath {
    at: Key,
    old: Point,
    targets: Vec<Point>,
    /// Whether the wire on the old tip is horizontal; Auto turns along it.
    horizontal_first: Option<bool>,
    template: Option<Wire>,
    /// The wires attached on the old tip: the same net, not obstacles.
    skip: Vec<usize>,
    /// A pin on a junction (#2540): the dots that follow it when no path
    /// fits and its wires follow instead.
    follow_back: Option<Vec<usize>>,
    is_pin: bool,
}

impl Env {
    /// Reroute every old tip of `by_tip`, after the wires that translate
    /// whole. A pin's new path that crosses or ends on the old tip of a pin
    /// not rerouted yet is laid last: the wires still on that tip may be
    /// about to follow their pin away, and a path laid first would block
    /// them or tie them to it (#2540).
    pub(in crate::transform::rubber_band) fn reroute_all(
        &mut self,
        by_tip: &BTreeMap<Key, Vec<&Move>>,
        out: &mut Follow,
    ) {
        let translated = self.translate_whole_wires(by_tip, out);
        out.edited_tips.extend(translated);
        self.pending = by_tip.keys().copied().collect();
        for (key, group) in by_tip {
            self.pending.remove(key);
            let before = out.touched.len();
            self.reroute_tip(*key, group, out);
            if out.touched.len() > before {
                out.edited_tips.push(*key);
            }
        }
        for path in std::mem::take(&mut self.deferred) {
            let (at, before) = (path.at, out.touched.len());
            self.lay(path, out);
            if out.touched.len() > before && !out.edited_tips.contains(&at) {
                out.edited_tips.push(at);
            }
        }
        self.merge_overlaps(out);
    }

    /// New paths from the old tip `at` to each distinct new tip of `group`.
    /// A tip with no collision-free route stays as it is; the guard sees the
    /// broken net and takes it down the ladder.
    pub(super) fn add_paths(
        &mut self,
        at: Key,
        group: &[&Move],
        attached: &[(usize, bool)],
        skip: &[usize],
        follow_back: Option<Vec<usize>>,
        out: &mut Follow,
    ) {
        // New wires copy the attached wire's stroke and unknown columns, like
        // the segments a bend adds; with none attached, the dragged wire's.
        let attached_wire = attached
            .first()
            .map(|(index, _)| self.wires[*index].clone());
        let horizontal_first = attached_wire
            .as_ref()
            .map(|w| key(&w.start).1 == key(&w.end).1);
        let template = attached_wire.or_else(|| self.carrier(group));
        let mut targets: Vec<Point> = Vec::new();
        for mv in group {
            if !targets.iter().any(|p| key(p) == key(&mv.new)) {
                targets.push(mv.new);
            }
        }
        let path = NewPath {
            at,
            old: group[0].old,
            targets,
            horizontal_first,
            template,
            skip: skip.to_vec(),
            follow_back,
            is_pin: group.iter().all(|m| m.wire.is_none()),
        };
        if path.is_pin && self.meets_a_pending_tip(&path) {
            self.deferred.push(path);
        } else {
            self.lay(path, out);
        }
    }

    /// Whether a route `path` may take touches an old tip still to be
    /// rerouted.
    fn meets_a_pending_tip(&self, path: &NewPath) -> bool {
        (path.targets.iter())
            .flat_map(|target| self.routes(path.old, *target, path.horizontal_first))
            .flatten()
            .any(|(a, b)| {
                let (ka, kb) = (key(&a), key(&b));
                (self.pending.iter()).any(|k| point_on_segment(*k, ka, kb))
            })
    }

    /// Lay `path`; a pin on a junction with no path that fits lets its
    /// wires follow it instead, as a free tip's do.
    fn lay(&mut self, path: NewPath, out: &mut Follow) {
        let mut laid = true;
        for target in &path.targets {
            laid &= self.add_path(
                path.old,
                *target,
                path.horizontal_first,
                path.template.as_ref(),
                &path.skip,
                out,
            );
        }
        let (Some(dots), false) = (path.follow_back, laid) else {
            return;
        };
        let new = path.targets[0];
        let attached = self.attached(path.at);
        if self.try_follow(path.at, &attached, new, out) {
            self.carry_dots(&dots, new, out);
        }
    }

    /// The selected wire whose drag carries the points of `group`, if any.
    fn carrier(&self, group: &[&Move]) -> Option<Wire> {
        let id = group.iter().find_map(|m| m.wire)?.wire();
        self.wires.iter().find(|w| w.uuid == id).cloned()
    }

    /// The routes from `old` to `new` to try, best first, as legs (a leg of
    /// zero length is dropped by the caller). Auto turns along the wire on
    /// the old tip; a chosen drag wiring mode bends by its drawing rule
    /// (`CornerRule::legs`), then its flip.
    fn routes(
        &self,
        old: Point,
        new: Point,
        horizontal_first: Option<bool>,
    ) -> Vec<Vec<(Point, Point)>> {
        if self.policy.wiring != DragWiringMode::Auto {
            let rules = self.policy.wiring.rules().into_iter();
            return rules.map(|rule| rule.legs(old, new)).collect();
        }
        let across = Point::new(new.x, old.y);
        let down = Point::new(old.x, new.y);
        let corners = if horizontal_first == Some(false) {
            [down, across]
        } else {
            [across, down]
        };
        corners
            .into_iter()
            .map(|corner| vec![(old, corner), (corner, new)])
            .collect()
    }

    /// Add the first route from `old` to `new` that touches nothing foreign
    /// as fresh wires; the old tip stays wired.
    pub(super) fn add_path(
        &mut self,
        old: Point,
        new: Point,
        horizontal_first: Option<bool>,
        template: Option<&Wire>,
        group: &[usize],
        out: &mut Follow,
    ) -> bool {
        let allowed = [key(&old), key(&new)];
        for mut segs in self.routes(old, new, horizontal_first) {
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
