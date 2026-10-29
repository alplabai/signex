//! Clean-up: exact duplicates and collinear overlaps the pass made.

use signex_net::uf::Key;
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

use super::{Env, key};
use crate::transform::rubber_band::Follow;

impl Env {
    /// Drop exact duplicates and merge collinear overlaps between the wires
    /// this pass touched and any other wire; the older or untouched wire
    /// survives with the union span.
    pub(in crate::transform::rubber_band) fn merge_overlaps(&mut self, out: &mut Follow) {
        // A pair trimmed off a locked wire is looked at once, so a trim that
        // changes nothing cannot make this loop spin.
        let mut trimmed: std::collections::HashSet<(usize, usize)> = Default::default();
        loop {
            let mut found = None;
            let mut edited: Vec<usize> = self.edited.iter().copied().collect();
            edited.sort_unstable();
            'search: for i in edited {
                if i >= self.wires.len() || self.removed.contains(&i) {
                    continue;
                }
                for (j, other) in self.live() {
                    // A locked wire is never edited (defence in depth: the pass
                    // only edits free wires); an overlap with one is trimmed
                    // off the other wire below.
                    if j != i
                        && !trimmed.contains(&(i, j))
                        && !self.wires[i].locked
                        && overlap_span(&self.wires[i], other).is_some()
                    {
                        found = Some((i, j));
                        break 'search;
                    }
                }
            }
            let Some((i, j)) = found else { return };
            if self.wires[j].locked {
                trimmed.insert((i, j));
                self.trim_off_locked(i, j, out);
                continue;
            }
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
                wire.locked = false;
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

impl Env {
    /// `free` overlaps the locked wire `locked`: cut `free` back to the parts
    /// outside the locked span (the locked wire is never touched), or drop it
    /// when nothing is left. The locked wire still carries every point the
    /// dropped stretch did, so no end is left dangling.
    fn trim_off_locked(&mut self, free: usize, locked: usize, out: &mut Follow) {
        let (u, l) = (self.wires[free].clone(), self.wires[locked].clone());
        let horizontal = key(&u.start).1 == key(&u.end).1;
        let vertical = key(&u.start).0 == key(&u.end).0;
        let mut pieces: Vec<(Point, Point)> = Vec::new();
        if horizontal || vertical {
            let at = |p: &Point| {
                let k = key(p);
                if horizontal { k.0 } else { k.1 }
            };
            let ordered = |w: &Wire| {
                if at(&w.start) <= at(&w.end) {
                    (w.start, w.end)
                } else {
                    (w.end, w.start)
                }
            };
            let ((ulo, uhi), (llo, lhi)) = (ordered(&u), ordered(&l));
            if at(&ulo) < at(&llo) {
                pieces.push((ulo, llo));
            }
            if at(&uhi) > at(&lhi) {
                pieces.push((lhi, uhi));
            }
        }
        let oriented = key(&u.start) <= key(&u.end);
        let mut pieces = pieces.into_iter();
        match pieces.next() {
            Some((lo, hi)) => {
                let wire = &mut self.wires[free];
                (wire.start, wire.end) = if oriented { (lo, hi) } else { (hi, lo) };
            }
            None => {
                self.removed.insert(free);
            }
        }
        for (lo, hi) in pieces {
            let mut wire = u.clone();
            wire.uuid = Uuid::new_v4();
            (wire.start, wire.end) = if oriented { (lo, hi) } else { (hi, lo) };
            out.touched
                .push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
            self.wires.push(wire);
            self.edited.insert(self.wires.len() - 1);
        }
        out.touched
            .push(SelectedItem::new(u.uuid, SelectedKind::Wire));
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

pub(crate) fn wires_overlap(a: &Wire, b: &Wire) -> bool {
    overlap_span(a, b).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::rubber_band::Policy;
    use signex_types::schematic::SchematicSheet;
    use std::collections::HashSet;

    fn horizontal(a: f64, b: f64, locked: bool) -> Wire {
        Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(a, 0.0),
            end: Point::new(b, 0.0),
            locked,
            ..Default::default()
        }
    }

    /// An edited wire (5..15) over the stretch 0..10 of another wire.
    fn env_over(other_locked: bool) -> Env {
        let sheet = SchematicSheet {
            wires: vec![
                horizontal(5.0, 15.0, false),
                horizontal(0.0, 10.0, other_locked),
            ],
            ..Default::default()
        };
        let mut env = Env::new(
            &sheet,
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            Policy::default(),
        );
        env.edited.insert(0);
        env
    }

    #[test]
    fn an_overlap_is_merged_into_the_unlocked_survivor() {
        let mut env = env_over(false);
        env.merge_overlaps(&mut Follow::default());
        assert!(!env.removed.is_empty(), "the control merges");
    }

    #[test]
    fn a_locked_wire_is_never_re_cut_and_the_free_wire_is_trimmed_off_it() {
        let mut env = env_over(true);
        let locked_before = env.wires[1].clone();
        env.merge_overlaps(&mut Follow::default());
        assert_eq!(env.wires[1], locked_before, "the locked wire is untouched");
        let free = &env.wires[0];
        assert_eq!(
            (free.start.x, free.end.x),
            (10.0, 15.0),
            "5..15 keeps 10..15"
        );
        assert!(env.removed.is_empty());
    }

    #[test]
    fn a_free_wire_inside_a_locked_one_is_dropped() {
        let sheet = SchematicSheet {
            wires: vec![horizontal(2.0, 4.0, false), horizontal(0.0, 10.0, true)],
            ..Default::default()
        };
        let mut env = Env::new(
            &sheet,
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            Policy::default(),
        );
        env.edited.insert(0);
        env.merge_overlaps(&mut Follow::default());
        assert!(env.removed.contains(&0));
    }
}
