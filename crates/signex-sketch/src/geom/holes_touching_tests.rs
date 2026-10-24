//! Holes that touch the outline or each other (#897): the bridged ring must
//! still partition `outline - holes` exactly, so the GPU fill never has to
//! fall back to the solid outline for them.

use super::Point2;
use super::point_in_polygon;
use super::predicates::signed_area;
use super::triangulate::ear_clip_with_holes;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point2> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

/// Why a triangulation is not a partition of `outer - holes`, if it is not.
/// Beyond the total area, every unit cell of the integer grid is sampled at a
/// generic offset: it must be covered exactly once outside the holes and not
/// at all inside them, which catches overlaps hidden by compensating gaps.
fn partition_error(outer: &[Point2], holes: &[Vec<Point2>]) -> Option<String> {
    let (merged, tris) = ear_clip_with_holes(outer, holes);
    let expected =
        signed_area(outer).abs() - holes.iter().map(|h| signed_area(h).abs()).sum::<f64>();
    let tri_pts: Vec<[Point2; 3]> = tris
        .iter()
        .map(|&[a, b, c]| [merged[a], merged[b], merged[c]])
        .collect();
    let total: f64 = tri_pts.iter().map(|t| signed_area(t).abs()).sum();
    if tris.is_empty() || (total - expected).abs() > expected.abs() * 1e-9 + 1e-9 {
        return Some(format!("triangle area {total}, want {expected}"));
    }
    let (max_x, max_y) = outer
        .iter()
        .fold((0.0_f64, 0.0_f64), |(mx, my), q| (mx.max(q.x), my.max(q.y)));
    for gx in 0..max_x as usize {
        for gy in 0..max_y as usize {
            let s = p(gx as f64 + 0.37, gy as f64 + 0.61);
            let covered = tri_pts
                .iter()
                .filter(|t| point_in_polygon(s, &t[..]))
                .count();
            let want = usize::from(
                point_in_polygon(s, outer) && holes.iter().all(|h| !point_in_polygon(s, h)),
            );
            if covered != want {
                return Some(format!("sample {s:?} covered {covered}x, want {want}x"));
            }
        }
    }
    None
}

#[test]
fn two_holes_on_the_same_outer_edge() {
    let outer = rect(0.0, 0.0, 10.0, 20.0);
    let holes = [rect(6.0, 4.0, 10.0, 6.0), rect(6.0, 12.0, 10.0, 14.0)];
    assert_eq!(partition_error(&outer, &holes), None);
}

/// Small deterministic generator (SplitMix64) so the layouts are fixed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> i64 {
        (self.next() % n) as i64
    }
}

type Cell = (i64, i64, i64, i64);

/// Whether the closed rectangles share an interior point.
fn overlap(a: Cell, b: Cell) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// Whether the closed rectangles touch (share at least a point) without
/// overlapping.
fn touch(a: Cell, b: Cell) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3 && !overlap(a, b)
}

/// Whether `r` lies on the boundary of `0..size` squared.
fn on_edge(r: Cell, size: i64) -> bool {
    r.0 == 0 || r.1 == 0 || r.2 == size || r.3 == size
}

/// Random interior-disjoint integer-grid rectangles in `size` x `size`.
/// `outline_touch` lets them reach the outline, `hole_touch` lets them touch
/// each other. Each mode also keeps the free area connected enough to be a
/// single weakly simple region: a rectangle that would seal off an island
/// (touching two outline sides, or closing a loop of touching holes) is
/// rejected by requiring it to touch at most one earlier hole and, when it
/// touches the outline, no earlier hole at all.
fn layout(rng: &mut Rng, size: i64, outline_touch: bool, hole_touch: bool) -> Vec<Cell> {
    let mut kept: Vec<Cell> = Vec::new();
    for _ in 0..40 {
        let x0 = rng.below(size as u64);
        let y0 = rng.below(size as u64);
        let r = (x0, y0, x0 + 1 + rng.below(4), y0 + 1 + rng.below(4));
        if r.2 > size || r.3 > size {
            continue;
        }
        let on_outline = on_edge(r, size);
        if on_outline && !outline_touch {
            continue;
        }
        if !on_outline && (r.0 == 0 || r.1 == 0) {
            continue;
        }
        let strict = |c: Cell| c.0 > 0 && c.1 > 0 && c.2 < size && c.3 < size;
        if !outline_touch && !strict(r) {
            continue;
        }
        let touching = kept.iter().filter(|&&k| touch(r, k)).count();
        let touches_outline_hole = kept.iter().any(|&k| on_edge(k, size));
        let ok = kept.iter().all(|&k| !overlap(r, k))
            && (hole_touch || touching == 0)
            && (touching <= 1)
            && !(on_outline && touching > 0 && touches_outline_hole)
            && !(on_outline && hole_touch && touching > 0);
        if ok {
            kept.push(r);
        }
    }
    kept
}

fn run_fuzz(seed: u64, outline_touch: bool, hole_touch: bool) -> (usize, usize, Vec<String>) {
    let mut rng = Rng(seed);
    let size = 14;
    let outer = rect(0.0, 0.0, size as f64, size as f64);
    let (mut cases, mut touching_cases) = (0, 0);
    let mut failures = Vec::new();
    for case in 0..400 {
        let cells = layout(&mut rng, size, outline_touch, hole_touch);
        let touches_outline = cells.iter().any(|&c| on_edge(c, size));
        let touches_hole = cells
            .iter()
            .enumerate()
            .any(|(i, &a)| cells[i + 1..].iter().any(|&b| touch(a, b)));
        let holes: Vec<_> = cells
            .iter()
            .map(|&(a, b, c, d)| rect(a as f64, b as f64, c as f64, d as f64))
            .collect();
        if holes.is_empty() {
            continue;
        }
        cases += 1;
        touching_cases += usize::from(touches_outline || touches_hole);
        if let Some(why) = partition_error(&outer, &holes) {
            failures.push(format!("case {case} {cells:?}: {why}"));
        }
    }
    (cases, touching_cases, failures)
}

#[test]
fn fuzz_holes_touching_the_outline() {
    let (cases, touching, failures) = run_fuzz(0x897, true, false);
    assert!(
        cases > 300 && touching > 100,
        "{cases} cases, {touching} touching"
    );
    assert!(
        failures.is_empty(),
        "{} failed, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
}

#[test]
fn fuzz_holes_touching_each_other() {
    let (cases, touching, failures) = run_fuzz(0x8971, false, true);
    assert!(
        cases > 300 && touching > 100,
        "{cases} cases, {touching} touching"
    );
    assert!(
        failures.is_empty(),
        "{} failed, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
}

#[test]
fn fuzz_holes_touching_both() {
    let (cases, touching, failures) = run_fuzz(0x8972, true, true);
    assert!(
        cases > 300 && touching > 100,
        "{cases} cases, {touching} touching"
    );
    assert!(
        failures.is_empty(),
        "{} failed, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
}

/// Every placement of a second rectangle (1x1 to 3x3) touching a fixed one
/// along part of a side or at a corner, inside the outline.
#[test]
fn a_hole_touching_another_in_every_placement() {
    let outer = rect(0.0, 0.0, 14.0, 14.0);
    let first = (5, 5, 7, 8);
    let mut placements = 0;
    for (w, h) in [(1, 1), (2, 1), (1, 3), (3, 2), (3, 3)] {
        for x in 1..=13 - w {
            for y in 1..=13 - h {
                let second = (x, y, x + w, y + h);
                if !touch(first, second) {
                    continue;
                }
                placements += 1;
                let holes: Vec<_> = [first, second]
                    .iter()
                    .map(|&(a, b, c, d)| rect(a as f64, b as f64, c as f64, d as f64))
                    .collect();
                assert_eq!(partition_error(&outer, &holes), None, "{second:?}");
            }
        }
    }
    assert!(placements > 40, "{placements} placements");
}

/// Timing harness for the ear clipper on grids of 16-gon holes:
/// `cargo test --release -p signex-sketch --lib -- --ignored --nocapture clipper_timing`.
#[test]
#[ignore = "timing harness"]
fn clipper_timing() {
    for side in [10_usize, 15, 20, 32] {
        let pitch = 10.0;
        let size = pitch * (side as f64 + 1.0);
        let holes: Vec<Vec<Point2>> = (0..side * side)
            .map(|k| {
                let (cx, cy) = (
                    pitch * ((k % side) as f64 + 1.0),
                    pitch * ((k / side) as f64 + 1.0),
                );
                (0..16)
                    .rev()
                    .map(|i| {
                        let a = std::f64::consts::TAU * i as f64 / 16.0;
                        p(cx + 3.0 * a.cos(), cy + 3.0 * a.sin())
                    })
                    .collect()
            })
            .collect();
        let outer = rect(0.0, 0.0, size, size);
        let t0 = std::time::Instant::now();
        let _ = super::holes::merge_holes(outer.clone(), holes.clone());
        println!("  merge only: {:?}", t0.elapsed());
        let start = std::time::Instant::now();
        let (merged, tris) = ear_clip_with_holes(&outer, &holes);
        let elapsed = start.elapsed();
        let total: f64 = tris
            .iter()
            .map(|&[a, b, c]| signed_area(&[merged[a], merged[b], merged[c]]).abs())
            .sum();
        let want =
            signed_area(&outer).abs() - holes.iter().map(|h| signed_area(h).abs()).sum::<f64>();
        println!(
            "{side}x{side}: {elapsed:?}, {} triangles, area err {:e}",
            tris.len(),
            (total - want).abs()
        );
    }
}
