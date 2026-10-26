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
