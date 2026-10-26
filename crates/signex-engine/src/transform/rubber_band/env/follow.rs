//! Following: a wire end goes with its pin, sliding an elbow or bending.

use signex_net::uf::Key;
use signex_types::schematic::{Point, SelectedItem, SelectedKind};
use uuid::Uuid;

use super::{Env, Plan, key, zero_length};
use crate::transform::rubber_band::Follow;

impl Env {
    /// Reroute every attached wire end to `new`; all or nothing.
    pub(super) fn try_follow(
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

    /// Move one end of wire `index` to the pin's `new` tip with the first
    /// route that touches nothing foreign; false when none does.
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
        if self.anchored_inside(&fixed, &moving, &[index], Some(key(&new))) {
            return false;
        }
        let (plans, far_keys) = self.follow_plans(index, fixed, moving, new);
        let mut allowed = vec![key(&fixed), key(&new), at];
        allowed.extend(far_keys);
        for mut plan in plans {
            plan.segs.retain(|(a, b)| !zero_length(a, b));
            let mut ignore: Vec<usize> = group.to_vec();
            ignore.extend(&plan.replace);
            if plan.segs.is_empty() {
                self.drop_if_held(index, key(&fixed), &ignore, out);
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

    /// The routes to try for a wire whose `moving` end goes to `new`, best
    /// first, and the far-end keys a slide may touch.
    fn follow_plans(
        &self,
        index: usize,
        fixed: Point,
        moving: Point,
        new: Point,
    ) -> (Vec<Plan>, Vec<Key>) {
        let (kf, km, kn) = (key(&fixed), key(&moving), key(&new));
        let horizontal = kf.1 == km.1 && kf.0 != km.0;
        let vertical = kf.0 == km.0 && kf.1 != km.1;
        let straight = |segs: Vec<(Point, Point)>| Plan {
            replace: vec![index],
            segs,
            joins: vec![kf],
        };
        if !(horizontal || vertical) {
            return (vec![straight(vec![(fixed, new)])], Vec::new());
        }
        if if horizontal {
            kn.1 == kf.1
        } else {
            kn.0 == kf.0
        } {
            return (vec![straight(vec![(fixed, new)])], Vec::new());
        }
        let mut plans = Vec::new();
        let mut far_keys = Vec::new();
        if let Some((plan, far)) = self.slide_plan(index, fixed, new, horizontal) {
            far_keys.push(key(&far));
            plans.push(plan);
        }
        let (c1, c2) = if horizontal {
            (Point::new(new.x, fixed.y), Point::new(fixed.x, new.y))
        } else {
            (Point::new(fixed.x, new.y), Point::new(new.x, fixed.y))
        };
        for corner in [c1, c2] {
            plans.push(straight(vec![(fixed, corner), (corner, new)]));
        }
        (plans, far_keys)
    }

    /// A wire that shrank to nothing goes when something else still holds its
    /// fixed end, else it stays (the pin lands on that end).
    fn drop_if_held(&mut self, index: usize, kf: Key, ignore: &[usize], out: &mut Follow) {
        let others = self.probe(kf, ignore).anything()
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
    }

    /// The other wire of a plain two-wire elbow at `kf`, with its far end.
    fn elbow_leg(&self, index: usize, kf: Key) -> Option<(usize, Point)> {
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
        let (leg, wire) = self
            .live()
            .find(|(i, w)| *i != index && (key(&w.start) == kf || key(&w.end) == kf))?;
        let far = if key(&wire.start) == kf {
            wire.end
        } else {
            wire.start
        };
        let free = !self.placed.contains(&(leg, true))
            && !self.placed.contains(&(leg, false))
            && !self.selected.contains(&wire.uuid);
        free.then_some((leg, far))
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
        let (leg, far) = self.elbow_leg(index, kf)?;
        let kfar = key(&far);
        let perpendicular = if horizontal {
            kfar.0 == kf.0 && kfar.1 != kf.1
        } else {
            kfar.1 == kf.1 && kfar.0 != kf.0
        };
        if !perpendicular || self.anchored_inside(&far, &fixed, &[leg, index], Some(key(&new))) {
            return None;
        }
        let elbow = if horizontal {
            Point::new(fixed.x, new.y)
        } else {
            Point::new(new.x, fixed.y)
        };
        let plan = Plan {
            replace: vec![leg, index],
            segs: vec![(far, elbow), (elbow, new)],
            joins: vec![kfar],
        };
        Some((plan, far))
    }

    /// Reuse the replaced wires for `plan.segs` in order, drop surplus
    /// wires, and mint wires for surplus segments (copying `template`).
    pub(super) fn apply_plan(&mut self, plan: Plan, template: usize, out: &mut Follow) {
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
    pub(super) fn place_ends(&mut self, index: usize, joins: &[Key]) {
        self.edited.insert(index);
        let wire = &self.wires[index];
        for (is_start, end) in [(true, wire.start), (false, wire.end)] {
            if !joins.contains(&key(&end)) {
                self.placed.insert((index, is_start));
            }
        }
    }
}
