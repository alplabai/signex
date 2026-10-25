//! Holes that touch the outline or each other (#897): the bridged ring must
//! still partition `outline - holes` exactly, so the GPU fill never has to
//! fall back to the solid outline for them. Also covers bridging through
//! near-degenerate rays on decimal-coordinate grids of polygons.

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

/// Sum of the triangle areas of `ear_clip_with_holes` minus the expected area,
/// `None` when they agree.
fn area_error(outer: &[Point2], holes: &[Vec<Point2>]) -> Option<f64> {
    let (merged, tris) = ear_clip_with_holes(outer, holes);
    let expected =
        signed_area(outer).abs() - holes.iter().map(|h| signed_area(h).abs()).sum::<f64>();
    let total: f64 = tris
        .iter()
        .map(|&[a, b, c]| signed_area(&[merged[a], merged[b], merged[c]]).abs())
        .sum();
    let off = total - expected;
    (tris.is_empty() || off.abs() > expected.abs() * 1e-9 + 1e-9).then_some(off)
}

/// Where coverage is sampled: one generic point in each cell of a 14 x 14
/// grid over the outline's box, plus seeded random points, so a gap hiding
/// anywhere is as likely to be seen as an overlap.
fn sample_points(outer: &[Point2]) -> Vec<Point2> {
    let (lo, hi) = outer.iter().fold(
        (
            p(f64::INFINITY, f64::INFINITY),
            p(f64::NEG_INFINITY, f64::NEG_INFINITY),
        ),
        |(lo, hi), q| {
            (
                p(lo.x.min(q.x), lo.y.min(q.y)),
                p(hi.x.max(q.x), hi.y.max(q.y)),
            )
        },
    );
    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
    let mut samples: Vec<Point2> = (0..14)
        .flat_map(|gx| (0..14).map(move |gy| (gx, gy)))
        .map(|(gx, gy)| {
            p(
                lo.x + (gx as f64 + 0.37) / 14.0 * w,
                lo.y + (gy as f64 + 0.61) / 14.0 * h,
            )
        })
        .collect();
    let mut rng = Rng(0x5EED);
    for _ in 0..300 {
        let unit = |r: &mut Rng| (r.next() >> 11) as f64 / (1_u64 << 53) as f64;
        samples.push(p(lo.x + unit(&mut rng) * w, lo.y + unit(&mut rng) * h));
    }
    samples
}

/// Why a triangulation is not a partition of `outer - holes`, if it is not.
/// Beyond the total area, every sample point must be covered exactly once
/// outside the holes and not at all inside them, which catches overlaps hidden
/// by compensating gaps.
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
    for s in sample_points(outer) {
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
    None
}

/// How the integer layout is mapped to the coordinates the triangulator sees:
/// board-like decimal coordinates, optionally rounded through `f32` as the
/// GPU path does.
#[derive(Clone, Copy)]
struct Frame {
    scale: f64,
    offset: f64,
    through_f32: bool,
}

const INTEGER: Frame = Frame {
    scale: 1.0,
    offset: 0.0,
    through_f32: false,
};

impl Frame {
    fn at(self, v: i64) -> f64 {
        let x = v as f64 * self.scale + self.offset;
        if self.through_f32 { x as f32 as f64 } else { x }
    }

    fn rect(self, (x0, y0, x1, y1): Cell) -> Vec<Point2> {
        rect(self.at(x0), self.at(y0), self.at(x1), self.at(y1))
    }
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

fn run_fuzz(
    seed: u64,
    outline_touch: bool,
    hole_touch: bool,
    frame: Frame,
) -> (usize, usize, Vec<String>) {
    let mut rng = Rng(seed);
    let size = 14;
    let outer = frame.rect((0, 0, size, size));
    let (mut cases, mut touching_cases) = (0, 0);
    let mut failures = Vec::new();
    for case in 0..400 {
        let cells = layout(&mut rng, size, outline_touch, hole_touch);
        let touches_outline = cells.iter().any(|&c| on_edge(c, size));
        let touches_hole = cells
            .iter()
            .enumerate()
            .any(|(i, &a)| cells[i + 1..].iter().any(|&b| touch(a, b)));
        let holes: Vec<_> = cells.iter().map(|&c| frame.rect(c)).collect();
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
    let (cases, touching, failures) = run_fuzz(0x897, true, false, INTEGER);
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
    let (cases, touching, failures) = run_fuzz(0x8971, false, true, INTEGER);
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
    let (cases, touching, failures) = run_fuzz(0x8972, true, true, INTEGER);
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
fn fuzz_touching_holes_on_a_decimal_grid() {
    let frame = Frame {
        scale: 0.127,
        offset: 100.05,
        through_f32: false,
    };
    let (cases, touching, failures) = run_fuzz(0x8973, true, true, frame);
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
fn fuzz_touching_holes_rounded_through_f32() {
    let frame = Frame {
        scale: 0.127,
        offset: 100.05,
        through_f32: true,
    };
    let (cases, touching, failures) = run_fuzz(0x8974, true, true, frame);
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

fn regular_polygon(cx: f64, cy: f64, r: f64, sides: usize, rotation: f64) -> Vec<Point2> {
    (0..sides)
        .map(|i| {
            let a = rotation + std::f64::consts::TAU * i as f64 / sides as f64;
            p(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Octagonal clearances on a 1.27 mm grid, rotated pi/8 so their vertices
/// lie on rows the ray from a neighbour passes a few ulp off: the common via
/// clearance shape on decimal coordinates. No cut-out may be bridged across.
#[test]
fn octagon_grids_on_decimal_coordinates() {
    let pitch = 1.27;
    let mut failures = Vec::new();
    for side in 2..=7_usize {
        let size = pitch * (side as f64 + 1.0);
        let outer = rect(0.0, 0.0, size, size);
        for step in 0..=100 {
            let r = 0.4 + 0.2 * step as f64 / 100.0;
            let holes: Vec<_> = (0..side * side)
                .map(|k| {
                    let (cx, cy) = (
                        pitch * ((k % side) as f64 + 1.0),
                        pitch * ((k / side) as f64 + 1.0),
                    );
                    regular_polygon(cx, cy, r, 8, std::f64::consts::PI / 8.0)
                })
                .collect();
            if let Some(off) = area_error(&outer, &holes) {
                failures.push(format!("side {side} r {r}: area off by {off}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 606 failed, first: {:?}",
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

/// A square outline of `vertices` points with a small inward wiggle, like an
/// arc-tessellated board edge.
fn wavy_outline(size: f64, vertices: usize) -> Vec<Point2> {
    if vertices == 4 {
        return rect(0.0, 0.0, size, size);
    }
    let per = vertices / 4;
    let mut ring = Vec::with_capacity(vertices);
    for side in 0..4 {
        for i in 0..per {
            let t = i as f64 / per as f64 * size;
            let w = if i % 2 == 1 { 0.5 } else { 0.0 };
            ring.push(match side {
                0 => p(t, w),
                1 => p(size - w, t),
                2 => p(size - t, size - w),
                _ => p(w, size - t),
            });
        }
    }
    ring
}

/// A 32 x 32 grid of 16-gon cut-outs, the size the timing harness reports
/// on, triangulates to the right area and stays far inside a generous time
/// bound, so a slide back to the old cubic clipper (about 1.6 s in release,
/// far longer in a debug test build) fails here instead of only in the
/// ignored harness.
#[test]
fn a_32_by_32_grid_of_cut_outs_triangulates_exactly_and_quickly() {
    let (side, pitch) = (32_usize, 10.0);
    let size = pitch * (side as f64 + 1.0);
    let holes: Vec<Vec<Point2>> = (0..side * side)
        .map(|k| {
            let (cx, cy) = (
                pitch * ((k % side) as f64 + 1.0),
                pitch * ((k / side) as f64 + 1.0),
            );
            regular_polygon(cx, cy, 3.0, 16, 0.0)
        })
        .collect();
    let outer = rect(0.0, 0.0, size, size);
    let t0 = std::time::Instant::now();
    assert_eq!(area_error(&outer, &holes), None);
    let took = t0.elapsed();
    assert!(
        took < std::time::Duration::from_secs(5),
        "32x32 cut-outs took {took:?}"
    );
}

/// `count` regular `sides`-gon cut-outs in one row along a strip about
/// 1000 times longer than it is tall (`count` = 1000), wound clockwise as
/// `merge_holes` takes them.
fn strip_of_cut_outs(count: usize, sides: usize) -> (Vec<Point2>, Vec<Vec<Point2>>) {
    let pitch = 10.0;
    let holes = (0..count)
        .map(|k| {
            let mut ring = regular_polygon(pitch * (k as f64 + 1.0), pitch / 2.0, 3.0, sides, 0.0);
            ring.reverse();
            ring
        })
        .collect();
    (rect(0.0, 0.0, pitch * (count as f64 + 1.0), pitch), holes)
}

/// `side` x `side` 16-gon cut-outs on a 10 mm pitch, wound clockwise.
fn grid_of_cut_outs(side: usize) -> (Vec<Point2>, Vec<Vec<Point2>>) {
    let pitch = 10.0;
    let holes = (0..side * side)
        .map(|k| {
            let (cx, cy) = (
                pitch * ((k % side) as f64 + 1.0),
                pitch * ((k / side) as f64 + 1.0),
            );
            let mut ring = regular_polygon(cx, cy, 3.0, 16, 0.0);
            ring.reverse();
            ring
        })
        .collect();
    let size = pitch * (side as f64 + 1.0);
    (rect(0.0, 0.0, size, size), holes)
}

/// Bridge `holes` into `outer`, require the merged ring to hold every vertex
/// plus the two repeats per bridge, and return how long it took.
fn timed_merge(outer: Vec<Point2>, holes: Vec<Vec<Point2>>) -> std::time::Duration {
    let expected = outer.len() + holes.iter().map(|h| h.len() + 2).sum::<usize>();
    let t0 = std::time::Instant::now();
    let merged = super::holes::merge_holes(outer, holes);
    let took = t0.elapsed();
    assert_eq!(merged.len(), expected);
    took
}

/// Bridging 4096 cut-outs into one ring (about 74k vertices) takes ~0.1 s in a
/// debug build. Folding the holes in one at a time over a flat vector, with a
/// full pass over the growing ring per hole, took ~5 s, so the bound separates
/// the two without being sensitive to a slow runner.
#[test]
fn bridging_a_64_by_64_grid_of_cut_outs_stays_near_linear() {
    let (outer, holes) = grid_of_cut_outs(64);
    let took = timed_merge(outer, holes);
    assert!(
        took < std::time::Duration::from_millis(2500),
        "bridging 64x64 cut-outs took {took:?}"
    );
}

/// The same bound for 8000 cut-outs in a strip (about 144k vertices): ~0.3 s
/// in a debug build, ~20 s with a pass over the ring per hole.
#[test]
fn bridging_an_8000_hole_strip_stays_near_linear() {
    let (outer, holes) = strip_of_cut_outs(8000, 16);
    let took = timed_merge(outer, holes);
    assert!(
        took < std::time::Duration::from_secs(3),
        "bridging an 8000-hole strip took {took:?}"
    );
}

/// A 1000 : 1 strip of cut-outs triangulates to the right area. The time
/// bound here only catches a blow-up in the whole pipeline; the clipper still
/// takes more than linear time on a strip (long thin ears query a box across
/// it), so the tight bounds are on the bridging above.
#[test]
fn a_1000_to_1_strip_of_cut_outs_triangulates_exactly() {
    let (outer, holes) = strip_of_cut_outs(1000, 16);
    let t0 = std::time::Instant::now();
    assert_eq!(area_error(&outer, &holes), None);
    let took = t0.elapsed();
    assert!(
        took < std::time::Duration::from_secs(30),
        "1000:1 strip took {took:?}"
    );
}

/// The clipper looks at a bounded number of reflex vertices per ear on a
/// strip of cut-outs. Its ears are long and thin and most are blocked by a
/// vertex far along the strip, so a box query or a scan up to the blocker
/// looks at millions of vertices here (8.2 million for 18004 vertices, and
/// quadratic in the strip length). Counting vertices rather than time keeps
/// the bound exact on any machine.
#[test]
fn clipping_a_strip_of_cut_outs_tests_few_vertices_per_ear() {
    let (outer, holes) = strip_of_cut_outs(1000, 16);
    let merged = super::holes::merge_holes(outer, holes);
    super::tolerant_clip::CANDIDATES_TESTED.with(|n| n.set(0));
    let tris = super::tolerant_clip::clip_ears_tolerant(&merged);
    let tested = super::tolerant_clip::CANDIDATES_TESTED.with(|n| n.get());
    assert_eq!(tris.len(), merged.len() - 2);
    assert!(
        tested < 120 * merged.len(),
        "{tested} vertices tested for a ring of {}",
        merged.len()
    );
}

/// Timing harness for the bridging and ear clipping of grids of 16-gon holes:
/// `cargo test --release -p signex-sketch --lib -- --ignored --nocapture clipper_timing`.
#[test]
#[ignore = "timing harness"]
fn clipper_timing() {
    for (side, outline) in [
        (10_usize, 4_usize),
        (15, 4),
        (20, 4),
        (32, 4),
        (20, 4000),
        (32, 4000),
        (32, 20000),
        (64, 4),
        (100, 4),
    ] {
        let pitch = 10.0;
        let size = pitch * (side as f64 + 1.0);
        let holes: Vec<Vec<Point2>> = (0..side * side)
            .map(|k| {
                let (cx, cy) = (
                    pitch * ((k % side) as f64 + 1.0),
                    pitch * ((k / side) as f64 + 1.0),
                );
                let mut ring = regular_polygon(cx, cy, 3.0, 16, 0.0);
                ring.reverse();
                ring
            })
            .collect();
        let outer = wavy_outline(size, outline);
        let t0 = std::time::Instant::now();
        let merged = super::holes::merge_holes(outer.clone(), holes.clone());
        let merge = t0.elapsed();
        let t1 = std::time::Instant::now();
        let tris = super::tolerant_clip::clip_ears_tolerant(&merged);
        println!(
            "{side}x{side} holes, {outline} outline vertices: merge {merge:?}, clip {:?}, {} triangles",
            t1.elapsed(),
            tris.len()
        );
    }
    for (count, sides) in [
        (500_usize, 16_usize),
        (1000, 16),
        (2000, 16),
        (4000, 16),
        (8000, 16),
    ] {
        let (outer, holes) = strip_of_cut_outs(count, sides);
        let t0 = std::time::Instant::now();
        let merged = super::holes::merge_holes(outer, holes);
        let merge = t0.elapsed();
        let t1 = std::time::Instant::now();
        let tris = super::tolerant_clip::clip_ears_tolerant(&merged);
        println!(
            "strip of {count} {sides}-gons: merge {merge:?}, clip {:?}, {} triangles",
            t1.elapsed(),
            tris.len()
        );
    }
}
