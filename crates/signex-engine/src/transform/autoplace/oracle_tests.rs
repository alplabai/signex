//! The indexed junction predicates must answer exactly like the original
//! full-scan implementations (kept here as the oracle) on dense sheets with
//! crossings, Ts, stars, diagonals and off-grid endpoints (issue #1364).

use super::*;
use signex_types::schematic::{Junction, Point, Wire};

const TOL: f64 = crate::JUNCTION_TOLERANCE_MM;

// ---- the pre-index implementations, verbatim in behaviour ----------------

fn scan_endpoint_count(point: Point, wires: &[Wire]) -> usize {
    wires
        .iter()
        .filter(|wire| {
            let at_start =
                (wire.start.x - point.x).abs() < TOL && (wire.start.y - point.y).abs() < TOL;
            let at_end = (wire.end.x - point.x).abs() < TOL && (wire.end.y - point.y).abs() < TOL;
            at_start || at_end
        })
        .count()
}

fn scan_honoured(point: Point, wires: &[Wire]) -> bool {
    let k = signex_net::pt_key(&point);
    wires
        .iter()
        .filter(|w| {
            signex_net::point_on_segment(
                k,
                signex_net::pt_key(&w.start),
                signex_net::pt_key(&w.end),
            )
        })
        .count()
        >= 2
}

fn scan_justifies(point: Point, wires: &[Wire]) -> bool {
    let endpoint_count = scan_endpoint_count(point, wires);
    if endpoint_count == 0 {
        return false;
    }
    let interior = wires
        .iter()
        .any(|wire| point_on_wire_interior(point, wire, TOL));
    (interior || endpoint_count >= 3) && scan_honoured(point, wires)
}

fn scan_needed(point: Point, wires: &[Wire], junctions: &[Junction]) -> Option<Point> {
    let present = junctions
        .iter()
        .any(|j| (j.position.x - point.x).abs() < TOL && (j.position.y - point.y).abs() < TOL);
    (!present && scan_justifies(point, wires)).then_some(point)
}

fn scan_under_new_wire(wire: &Wire, wires: &[Wire], junctions: &[Junction]) -> Vec<Point> {
    let mut placed: Vec<Point> = Vec::new();
    for point in wires
        .iter()
        .flat_map(|w| [w.start, w.end])
        .filter(|p| point_on_wire_interior(*p, wire, TOL))
        .filter(|p| scan_justifies(*p, wires))
    {
        let already = junctions
            .iter()
            .map(|j| j.position)
            .chain(placed.iter().copied())
            .any(|q| (q.x - point.x).abs() < TOL && (q.y - point.y).abs() < TOL);
        if !already {
            placed.push(point);
        }
    }
    placed
}

fn scan_for_wire(wire: &Wire, wires: &[Wire], junctions: &[Junction]) -> Vec<Point> {
    let mut placed: Vec<Point> = [wire.start, wire.end]
        .into_iter()
        .filter_map(|p| scan_needed(p, wires, junctions))
        .collect();
    placed.extend(scan_under_new_wire(wire, wires, junctions));
    placed
}

// ---- deterministic random sheets ------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: uuid::Uuid::now_v7(),
        start: a,
        end: b,
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    }
}

/// Snap-grid points on a small lattice (dense Ts / crossings / stars), a
/// quarter of them nudged off-grid by up to 12 um so the tolerance and key
/// metrics diverge, and some diagonals.
fn random_sheet(seed: u64, wire_count: usize) -> (Vec<Wire>, Vec<Junction>) {
    let mut rng = Lcg(seed);
    let pt = |rng: &mut Lcg| {
        let nudge = if rng.next(4) == 0 {
            (rng.next(25) as f64 - 12.0) / 1000.0
        } else {
            0.0
        };
        Point::new(
            (rng.next(12) as f64 - 6.0) * 1.27 + nudge,
            (rng.next(12) as f64 - 6.0) * 1.27 + nudge,
        )
    };
    let mut wires = Vec::new();
    for _ in 0..wire_count {
        let a = pt(&mut rng);
        let b = match rng.next(6) {
            0 => pt(&mut rng),
            1 => Point::new(a.x, pt(&mut rng).y),
            _ => Point::new(pt(&mut rng).x, a.y),
        };
        wires.push(wire(a, b));
    }
    // One diagonal wider than 64 cells: indexed as oversized, and it forces
    // `endpoint_candidates_near_wire`'s scan-everything fallback.
    wires.push(wire(Point::new(-7.62, -7.62), Point::new(80.0, 80.0)));
    let mut junctions = Vec::new();
    for _ in 0..wire_count {
        let p = pt(&mut rng);
        junctions.push(Junction {
            unknown_columns: Default::default(),
            uuid: uuid::Uuid::now_v7(),
            position: p,
            diameter: 0.0,
            minted: rng.next(2) == 0,
        });
    }
    (wires, junctions)
}

fn same(a: &[Point], b: &[Junction]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, j)| *p == j.position)
}

#[test]
fn indexed_predicates_match_the_full_scan_on_random_dense_sheets() {
    for seed in 0..40u64 {
        let (wires, junctions) = random_sheet(seed, 10 + (seed as usize % 5) * 20);
        let index = WireIndex::new(&wires, TOL);

        let mut probes: Vec<Point> = wires.iter().flat_map(|w| [w.start, w.end]).collect();
        probes.extend(junctions.iter().map(|j| j.position));
        let mut rng = Lcg(seed ^ 0xABCD);
        for _ in 0..60 {
            probes.push(Point::new(
                rng.next(12 * 127) as f64 / 100.0 - 7.62,
                rng.next(12 * 127) as f64 / 100.0 - 7.62,
            ));
        }

        for p in probes {
            assert_eq!(
                wire_meeting_justifies_junction(p, &index),
                scan_justifies(p, &wires),
                "seed {seed} point {p:?}"
            );
        }
        for w in &wires {
            let got = junctions_for_wire(w, &junctions, &index);
            let want = scan_for_wire(w, &wires, &junctions);
            assert!(same(&want, &got), "seed {seed} wire {:?}", w.uuid);
        }
    }
}

#[test]
fn long_diagonal_wire_is_still_found_through_the_oversized_list() {
    let wires = vec![
        wire(Point::new(0.0, 0.0), Point::new(300.0, 300.0)),
        wire(Point::new(150.0, 150.0), Point::new(150.0, 200.0)),
    ];
    let index = WireIndex::new(&wires, TOL);
    let p = Point::new(150.0, 150.0);
    assert!(scan_justifies(p, &wires));
    assert!(wire_meeting_justifies_junction(p, &index));
}

#[test]
fn enormous_or_non_finite_wires_neither_overflow_nor_hang() {
    let wires = vec![
        wire(Point::new(-1e300, -1e300), Point::new(1e300, 1e300)),
        wire(
            Point::new(f64::NEG_INFINITY, 0.0),
            Point::new(f64::INFINITY, 1.0),
        ),
        wire(Point::new(f64::NAN, f64::NAN), Point::new(1.0, 1.0)),
        wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
    ];
    let index = WireIndex::new(&wires, TOL);
    for w in &wires {
        let _ = index.endpoint_candidates_near_wire(w);
        let _ = junctions_for_wire(w, &[], &index);
    }
    let _ = wire_meeting_justifies_junction(Point::new(5.0, 0.0), &index);
}
