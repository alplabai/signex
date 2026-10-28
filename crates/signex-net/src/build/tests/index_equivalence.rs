//! The indexed anchoring / junction merge must equal the original linear
//! scans (issue #1366) on randomly generated sheets, including diagonals,
//! collinear overlaps and very long wires.
use super::*;
use std::collections::HashMap;

/// The pre-index `anchor_point`, kept verbatim as the oracle.
fn anchor_point_scan(parent: &mut HashMap<Key, Key>, pk: Key, segments: &[(Point, Point)]) {
    let keyed = || segments.iter().map(|(a, b)| (pt_key(a), pt_key(b)));
    if keyed().any(|(ak, bk)| pk == ak || pk == bk) {
        return;
    }
    let chosen = anchor_choice(
        keyed()
            .filter(|&(ak, bk)| point_on_segment(pk, ak, bk))
            .map(|(ak, bk)| ((ak, bk), ak, bk)),
    );
    if let Some((ak, bk)) = chosen {
        uf_union(parent, pk, ak.min(bk));
    }
}

/// The pre-index `from_segments`, as a parent map.
fn from_segments_scan(wires: &[(Point, Point)], junctions: &[Point]) -> HashMap<Key, Key> {
    let mut parent = HashMap::new();
    for (s, e) in wires {
        uf_union(&mut parent, pt_key(s), pt_key(e));
    }
    for jp in junctions {
        let jk = pt_key(jp);
        for (s, e) in wires {
            if point_on_segment(jk, pt_key(s), pt_key(e)) {
                uf_union(&mut parent, jk, pt_key(s));
            }
        }
    }
    parent
}

struct Rng(u64);
impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

fn pt(r: &mut Rng, span: u64) -> Point {
    Point::new(r.below(span) as f64, r.below(span) as f64)
}

/// Random wires: mostly orthogonal, some diagonal (incl. exact 45 degree so
/// lattice points land on interiors), a few spanning far beyond one grid cell.
fn random_wires(r: &mut Rng, n: usize, span: u64) -> Vec<(Point, Point)> {
    (0..n)
        .map(|_| {
            let a = pt(r, span);
            match r.below(6) {
                0 => (
                    a,
                    Point::new(a.x + r.below(8) as f64, a.y + r.below(8) as f64),
                ),
                1 => {
                    let d = r.below(10) as f64;
                    (a, Point::new(a.x + d, a.y - d))
                }
                2 => (a, Point::new(a.x + 5000.0, a.y)), // long: > cell span
                3 => (a, Point::new(a.x, a.y + r.below(40) as f64)),
                _ => (a, Point::new(a.x + r.below(40) as f64, a.y)),
            }
        })
        .collect()
}

#[test]
fn indexed_anchoring_and_junction_merge_match_the_linear_scans() {
    for seed in 1..=60u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let wires = random_wires(&mut r, 30 + (seed as usize % 4) * 40, 60);
        let junctions: Vec<Point> = (0..40).map(|_| pt(&mut r, 60)).collect();

        let mut new = SheetConnectivity::from_segments(&wires, &junctions);
        let mut old = from_segments_scan(&wires, &junctions);
        let index = SegmentIndex::new(&wires);

        let mut probes = Vec::new();
        for _ in 0..200 {
            probes.push(pt(&mut r, 60));
        }
        for p in &probes {
            let pk = pt_key(p);
            anchor_point_scan(&mut old, pk, &wires);
            let new_root = new.root_of_anchored_in(p, &index);
            assert_eq!(new_root, uf_find(&mut old, pk), "seed {seed} probe {pk:?}");
        }
        // Whole-partition identity, not just the probed points.
        let keys: Vec<Key> = old.keys().copied().collect();
        for k in keys {
            assert_eq!(
                uf_find(&mut new.parent, k),
                uf_find(&mut old, k),
                "seed {seed} key {k:?}"
            );
        }
    }
}
