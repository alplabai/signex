//! Endpoint-chaining core — join loose `Line`/`Arc` segments (schematic
//! symbol body strokes) end-to-end into a single closed polygon contour.
//!
//! This is the algorithmic core behind the future "join into polygon"
//! symbol-editor action: the caller supplies whatever `Line`/`Arc`
//! graphics the user has selected (in any order, any direction) and gets
//! back one ordered, CCW-wound vertex ring, or a precise diagnosis of why
//! the segments don't form one.
//!
//! ## Arc convention (load-bearing — read before touching this file)
//!
//! [`ChainSegment::Arc`] mirrors [`super::SymbolGraphicKind::Arc`]'s
//! `{ center, radius, start_deg, end_deg }` shape, so it must match that
//! type's real interpretation exactly. The authoritative reference is
//! `crates/signex-app/src/library/editor/symbol/state/hit_test.rs`
//! (the `graphic_contains_point` arm for `Arc`, plus the `ArcStart`/
//! `ArcEnd` handle-position arms just below it):
//!
//! - The point at angle `a` (degrees) on the circle is
//!   `center + radius * (cos(a), sin(a))` — standard math convention, no
//!   axis flips.
//! - The arc always sweeps **counter-clockwise** (increasing angle) from
//!   `start_deg` to `end_deg`, wrapping through 360° when
//!   `start_deg > end_deg` (see the `hit_test.rs` `if s <= e { .. } else
//!   { a >= s || a <= e }` split). This is corroborated by
//!   `signex-renderer`'s `arc_emitter_preserves_wraparound_and_tiny_radius_inputs`
//!   test, which exercises exactly a `start > end` wraparound arc.
//!
//! `start_deg == end_deg` is a degenerate zero-sweep case in the
//! `SymbolGraphicKind::Arc` hit-test world, but for a *chain input* we
//! treat it as a **full 360° circle**: the CCW-wraparound formula used
//! for the sweep magnitude (`(end - start).rem_euclid(360°)`, promoted to
//! 360° whenever it lands on exactly 0°) naturally falls out of the same
//! wraparound rule used for every other arc, needs no special case, and
//! gives a single self-closing segment a well-defined, useful ring
//! (see the `full_circle_arc_alone_*` tests below) instead of silently
//! collapsing to a single repeated point.
//!
//! `crates/signex-bake/src/profile.rs`'s `push_arc_interior_if_arc` uses
//! a *different* arc representation (`center`/`start`/`end` points plus
//! an explicit `sweep_ccw` flag) for PCB sketch profiles — it is not the
//! convention this module follows; `SymbolGraphicKind::Arc` carries no
//! such flag, so CCW-always (per `hit_test.rs`) is the only convention
//! consistent with the existing symbol data model.

use std::collections::HashMap;

/// Endpoints within this distance (mm) are treated as the same point for
/// chaining purposes.
pub const CHAIN_ENDPOINT_EPSILON_MM: f64 = 0.01;

/// Number of straight segments an [`ChainSegment::Arc`] is tessellated
/// into. Matches `signex-bake::profile::ARC_SAMPLES`.
pub const CHAIN_ARC_SAMPLES: usize = 16;

/// Squared degenerate-area threshold (mm²) below which a closed ring is
/// treated as having ~zero area (collinear / self-overlapping input)
/// rather than a real polygon. Small relative to any real symbol
/// geometry (mm-scale) but well above float noise.
const CHAIN_DEGENERATE_AREA_X2_EPS_MM2: f64 = 1e-9;

/// One input stroke to be chained. Mirrors the geometry-bearing fields
/// of [`super::SymbolGraphicKind::Line`] / [`super::SymbolGraphicKind::Arc`]
/// so callers can build these directly from selected symbol graphics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChainSegment {
    /// Straight stroke from `from` to `to`.
    Line { from: [f64; 2], to: [f64; 2] },
    /// Circular-arc stroke. See the module doc comment for the exact
    /// endpoint/sweep-direction convention.
    Arc {
        center: [f64; 2],
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
}

/// Why [`chain_into_closed_contour`] couldn't produce a single closed
/// ring from the given segments.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum ChainError {
    /// No segments were given.
    #[error("no segments given")]
    Empty,
    /// Every segment chains into one connected path, but it never closes
    /// back on itself. `gap_mm` is the distance between the two loose
    /// ends.
    #[error("chain doesn't close — {gap_mm} mm gap between the two loose ends")]
    OpenChain { gap_mm: f64 },
    /// More than two segment endpoints meet at the same point — the
    /// input isn't a simple chain (e.g. a T- or X-junction).
    #[error("{at:?} is shared by more than two segment endpoints")]
    Branching { at: [f64; 2] },
    /// The segments form more than one connected component (e.g. two
    /// separate closed shapes, or one closed shape plus leftover
    /// segments).
    #[error("segments form more than one connected component")]
    Disjoint,
    /// The chained ring has fewer than 3 distinct vertices, or its
    /// enclosed area is ~zero (collinear / self-cancelling input).
    #[error("chained ring is degenerate (too few vertices or ~zero area)")]
    DegenerateResult,
}

/// Chain `segments` end-to-end via shared endpoints (within
/// [`CHAIN_ENDPOINT_EPSILON_MM`]) into one closed contour.
///
/// Segments may be given in any order and any direction — a segment is
/// reversed as needed while walking the chain. [`ChainSegment::Arc`]
/// legs are tessellated into [`CHAIN_ARC_SAMPLES`] straight segments
/// (endpoint-exact: the first/last tessellated points are the arc's true
/// endpoints, so chaining tolerance behaves identically to a `Line`).
///
/// Returns the closed ring as an ordered vertex list **without**
/// repeating the first vertex at the end (implicit close); consecutive
/// duplicate points (within epsilon, including the wrap-around
/// last-to-first pair) are collapsed. The winding is normalised to
/// counter-clockwise (positive shoelace area).
///
/// Adjacency is resolved by direct pairwise (O(n²)) epsilon comparison
/// rather than hashing rounded coordinates or bucketing into a spatial
/// grid: symbol "join into polygon" selections are a handful of
/// segments, so the quadratic cost is irrelevant, and avoiding both
/// float-hashing and bucket-boundary edge cases keeps the endpoint match
/// exact and easy to audit.
pub fn chain_into_closed_contour(segments: &[ChainSegment]) -> Result<Vec<[f64; 2]>, ChainError> {
    if segments.is_empty() {
        return Err(ChainError::Empty);
    }

    let polylines: Vec<Vec<[f64; 2]>> = segments.iter().map(tessellate_segment).collect();
    let n = polylines.len();
    let eps_sq = CHAIN_ENDPOINT_EPSILON_MM * CHAIN_ENDPOINT_EPSILON_MM;

    // Cluster the 2n segment-endpoints (start + end of every segment)
    // into "nodes" — points within epsilon of each other, transitively,
    // via a small union-find. Ref `2*seg` is that segment's start
    // point, `2*seg + 1` is its end point.
    let endpoint_at = |seg: usize, is_start: bool| -> [f64; 2] {
        let poly = &polylines[seg];
        if is_start {
            poly[0]
        } else {
            *poly.last().expect("tessellated segment is never empty")
        }
    };
    let refs: Vec<[f64; 2]> = (0..n)
        .flat_map(|seg| [endpoint_at(seg, true), endpoint_at(seg, false)])
        .collect();
    let mut parent: Vec<usize> = (0..refs.len()).collect();
    for i in 0..refs.len() {
        for j in (i + 1)..refs.len() {
            if dist_sq(refs[i], refs[j]) <= eps_sq {
                uf_union(&mut parent, i, j);
            }
        }
    }

    // Group by cluster root into node tables: which (segment, is_start)
    // entries land on each node, and that node's representative point.
    let mut node_entries: HashMap<usize, Vec<(usize, bool)>> = HashMap::new();
    let mut node_points: HashMap<usize, Vec<[f64; 2]>> = HashMap::new();
    for seg in 0..n {
        for is_start in [true, false] {
            let ref_idx = ref_index(seg, is_start);
            let root = uf_find(&mut parent, ref_idx);
            node_entries.entry(root).or_default().push((seg, is_start));
            node_points.entry(root).or_default().push(refs[ref_idx]);
        }
    }

    // Branching: any node touched by more than two segment-ends.
    for (root, entries) in &node_entries {
        if entries.len() > 2 {
            let at = average_point(&node_points[root]);
            return Err(ChainError::Branching { at });
        }
    }

    // Connected components over segments: union two segments whenever a
    // shared (degree-2) node ties them together.
    let mut seg_parent: Vec<usize> = (0..n).collect();
    let mut dangling_nodes: Vec<usize> = Vec::new();
    for (root, entries) in &node_entries {
        match entries.as_slice() {
            [(a, _), (b, _)] => uf_union(&mut seg_parent, *a, *b),
            [_] => dangling_nodes.push(*root),
            _ => unreachable!("branching nodes already rejected above"),
        }
    }
    let components: std::collections::HashSet<usize> =
        (0..n).map(|s| uf_find(&mut seg_parent, s)).collect();
    if components.len() > 1 {
        return Err(ChainError::Disjoint);
    }

    match dangling_nodes.as_slice() {
        [] => {}
        [a, b] => {
            let gap_mm = dist_sq(
                average_point(&node_points[a]),
                average_point(&node_points[b]),
            )
            .sqrt();
            return Err(ChainError::OpenChain { gap_mm });
        }
        // A connected component whose nodes all have degree ≤ 2 is
        // either a simple cycle (0 dangling nodes) or a simple path
        // (exactly 2) — never anything else.
        _ => unreachable!("connected max-degree-2 component has 0 or 2 dangling nodes"),
    }

    // Every node has degree exactly 2 and every segment is reachable
    // from every other — this is a single simple cycle. Walk it,
    // reversing each segment's polyline as needed.
    let mut visited = vec![false; n];
    let mut ring_raw: Vec<[f64; 2]> = Vec::new();
    let mut current_seg = 0usize;
    let mut forward = true;
    loop {
        visited[current_seg] = true;
        let poly = &polylines[current_seg];
        if forward {
            ring_raw.extend(poly.iter().copied());
        } else {
            ring_raw.extend(poly.iter().rev().copied());
        }

        if visited.iter().all(|&v| v) {
            break;
        }

        let arrived_via_is_start = !forward;
        let arrived_node = uf_find(&mut parent, ref_index(current_seg, arrived_via_is_start));
        let (next_seg, next_is_start) = node_entries[&arrived_node]
            .iter()
            .copied()
            .find(|&(s, is_start)| !(s == current_seg && is_start == arrived_via_is_start))
            .expect("degree-2 node must have a distinct other entry");
        current_seg = next_seg;
        forward = next_is_start;
    }

    finalize_ring(ring_raw)
}

/// Collapse consecutive (and wrap-around) duplicate points, reject
/// degenerate results, and normalise winding to CCW.
fn finalize_ring(raw: Vec<[f64; 2]>) -> Result<Vec<[f64; 2]>, ChainError> {
    let eps_sq = CHAIN_ENDPOINT_EPSILON_MM * CHAIN_ENDPOINT_EPSILON_MM;
    let mut ring: Vec<[f64; 2]> = Vec::with_capacity(raw.len());
    for p in raw {
        match ring.last() {
            Some(&last) if dist_sq(last, p) <= eps_sq => {}
            _ => ring.push(p),
        }
    }
    while ring.len() > 1 && dist_sq(ring[0], *ring.last().unwrap()) <= eps_sq {
        ring.pop();
    }

    if ring.len() < 3 {
        return Err(ChainError::DegenerateResult);
    }

    let area_x2 = signed_area_x2(&ring);
    if area_x2.abs() < CHAIN_DEGENERATE_AREA_X2_EPS_MM2 {
        return Err(ChainError::DegenerateResult);
    }
    if area_x2 < 0.0 {
        ring.reverse();
    }
    Ok(ring)
}

/// Expand one input segment into its ordered point list, endpoints
/// included. `Line` is trivially its two endpoints; `Arc` is sampled
/// into `CHAIN_ARC_SAMPLES + 1` points per the module-level convention.
fn tessellate_segment(seg: &ChainSegment) -> Vec<[f64; 2]> {
    match *seg {
        ChainSegment::Line { from, to } => vec![from, to],
        ChainSegment::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let s = start_deg.rem_euclid(360.0);
            let e = end_deg.rem_euclid(360.0);
            // CCW sweep magnitude in (0, 360]; `s == e` (any full-turn
            // multiple of 360° apart) lands exactly on 360°, i.e. a
            // full circle — see the module doc comment.
            let mut delta = e - s;
            if delta <= 0.0 {
                delta += 360.0;
            }
            (0..=CHAIN_ARC_SAMPLES)
                .map(|i| {
                    let frac = i as f64 / CHAIN_ARC_SAMPLES as f64;
                    point_at_deg(center, radius, s + delta * frac)
                })
                .collect()
        }
    }
}

/// Point at angle `deg` (degrees) on the circle — matches
/// `SymbolGraphicKind::Arc`'s endpoint-handle math in `hit_test.rs`.
fn point_at_deg(center: [f64; 2], radius: f64, deg: f64) -> [f64; 2] {
    let rad = deg.to_radians();
    [
        center[0] + radius * rad.cos(),
        center[1] + radius * rad.sin(),
    ]
}

/// Index into the flat `refs` array (and the `parent` union-find) for a
/// segment's start (`is_start = true`) or end (`is_start = false`)
/// endpoint. Start is ref `2*seg`, end is ref `2*seg + 1`.
fn ref_index(seg: usize, is_start: bool) -> usize {
    2 * seg + usize::from(!is_start)
}

fn dist_sq(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

fn average_point(points: &[[f64; 2]]) -> [f64; 2] {
    let count = points.len() as f64;
    let sum_x: f64 = points.iter().map(|p| p[0]).sum();
    let sum_y: f64 = points.iter().map(|p| p[1]).sum();
    [sum_x / count, sum_y / count]
}

/// Twice the signed polygon area (shoelace, standard math orientation —
/// positive = counter-clockwise). Doubled to skip a division that every
/// call site only needs the sign/magnitude of.
fn signed_area_x2(ring: &[[f64; 2]]) -> f64 {
    let n = ring.len();
    let mut sum = 0.0;
    for i in 0..n {
        let [x0, y0] = ring[i];
        let [x1, y1] = ring[(i + 1) % n];
        sum += x0 * y1 - x1 * y0;
    }
    sum
}

fn uf_find(parent: &mut [usize], x: usize) -> usize {
    if parent[x] != x {
        parent[x] = uf_find(parent, parent[x]);
    }
    parent[x]
}

fn uf_union(parent: &mut [usize], a: usize, b: usize) {
    let ra = uf_find(parent, a);
    let rb = uf_find(parent, b);
    if ra != rb {
        parent[ra] = rb;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(from: [f64; 2], to: [f64; 2]) -> ChainSegment {
        ChainSegment::Line { from, to }
    }

    fn assert_approx_eq(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {a} ≈ {b}");
    }

    fn assert_point_approx_eq(a: [f64; 2], b: [f64; 2]) {
        assert!(dist_sq(a, b).sqrt() < 1e-9, "expected {a:?} ≈ {b:?}");
    }

    #[test]
    fn square_from_four_lines_shuffled_and_reversed_closes_ccw() {
        // Arrange: the 4 sides of a 4x4 square, shuffled and some
        // reversed — chaining must not depend on input order/direction.
        let segments = [
            line([4.0, 0.0], [0.0, 0.0]), // bottom, reversed
            line([4.0, 4.0], [4.0, 0.0]), // right, reversed
            line([0.0, 0.0], [0.0, 4.0]), // left, forward
            line([0.0, 4.0], [4.0, 4.0]), // top, forward
        ];

        // Act
        let ring = chain_into_closed_contour(&segments).expect("square should close");

        // Assert
        assert_eq!(ring.len(), 4);
        let mut sorted = ring.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(sorted, vec![[0.0, 0.0], [0.0, 4.0], [4.0, 0.0], [4.0, 4.0]]);
        assert!(signed_area_x2(&ring) > 0.0, "must be wound CCW");
    }

    #[test]
    fn cw_input_square_is_renormalised_to_ccw() {
        // Arrange: 4 sides walked in a CW order (up, right, down, left).
        let segments = [
            line([0.0, 0.0], [0.0, 4.0]),
            line([0.0, 4.0], [4.0, 4.0]),
            line([4.0, 4.0], [4.0, 0.0]),
            line([4.0, 0.0], [0.0, 0.0]),
        ];

        // Act
        let ring = chain_into_closed_contour(&segments).expect("square should close");

        // Assert: winding was flipped to CCW, area magnitude preserved.
        assert_eq!(ring.len(), 4);
        assert_approx_eq(signed_area_x2(&ring), 32.0);
    }

    #[test]
    fn triangle_with_one_arc_side_closes_with_tessellated_arc_points() {
        // Arrange: two straight sides plus one arc side (270° sweep)
        // closing the loop back to the origin.
        let p0 = [0.0, 0.0];
        let p1 = [4.0, 0.0];
        let p2 = [4.0, 4.0];
        // Arc centered at [0, 4], radius 4: p2 sits at 0°, p0 sits at
        // 270° from that center, so start_deg=0, end_deg=270 chains
        // p2 -> p0 exactly.
        let segments = [
            line(p0, p1),
            line(p1, p2),
            ChainSegment::Arc {
                center: [0.0, 4.0],
                radius: 4.0,
                start_deg: 0.0,
                end_deg: 270.0,
            },
        ];

        // Act
        let ring = chain_into_closed_contour(&segments).expect("D-shape should close");

        // Assert: 3 straight corners + (CHAIN_ARC_SAMPLES - 1) arc
        // interior points, no duplicate seam points, closed and CCW.
        assert_eq!(ring.len(), 3 + (CHAIN_ARC_SAMPLES - 1));
        for w in ring.windows(2) {
            assert!(dist_sq(w[0], w[1]) > 1e-12, "no duplicate seam points");
        }
        assert!(signed_area_x2(&ring) > 0.0);
        // The 90° arc sample (bulging away from the two straight sides)
        // must be present among the vertices.
        assert!(ring.iter().any(|p| p[1] > 4.0 + 1e-9));
    }

    #[test]
    fn open_chain_three_sides_of_square_reports_gap() {
        // Arrange: bottom, right, top sides present; left side missing.
        let segments = [
            line([0.0, 0.0], [4.0, 0.0]),
            line([4.0, 0.0], [4.0, 4.0]),
            line([4.0, 4.0], [0.0, 4.0]),
        ];

        // Act
        let err = chain_into_closed_contour(&segments).unwrap_err();

        // Assert
        match err {
            ChainError::OpenChain { gap_mm } => assert_approx_eq(gap_mm, 4.0),
            other => panic!("expected OpenChain, got {other:?}"),
        }
    }

    #[test]
    fn t_junction_of_three_lines_reports_branching() {
        // Arrange: three segments all touching the origin.
        let segments = [
            line([0.0, 0.0], [2.0, 0.0]),
            line([0.0, 0.0], [0.0, 2.0]),
            line([0.0, 0.0], [-2.0, 0.0]),
        ];

        // Act
        let err = chain_into_closed_contour(&segments).unwrap_err();

        // Assert
        match err {
            ChainError::Branching { at } => assert_point_approx_eq(at, [0.0, 0.0]),
            other => panic!("expected Branching, got {other:?}"),
        }
    }

    #[test]
    fn two_separate_triangles_report_disjoint() {
        // Arrange: two fully-closed, spatially separate triangles.
        let triangle_a = [
            line([0.0, 0.0], [2.0, 0.0]),
            line([2.0, 0.0], [1.0, 2.0]),
            line([1.0, 2.0], [0.0, 0.0]),
        ];
        let triangle_b = [
            line([10.0, 0.0], [12.0, 0.0]),
            line([12.0, 0.0], [11.0, 2.0]),
            line([11.0, 2.0], [10.0, 0.0]),
        ];
        let segments: Vec<ChainSegment> = triangle_a.into_iter().chain(triangle_b).collect();

        // Act
        let err = chain_into_closed_contour(&segments).unwrap_err();

        // Assert
        assert_eq!(err, ChainError::Disjoint);
    }

    #[test]
    fn line_plus_its_own_reverse_is_degenerate() {
        // Arrange: two segments tracing the exact same edge back and
        // forth — a closed but zero-area 2-vertex loop.
        let segments = [line([0.0, 0.0], [4.0, 0.0]), line([4.0, 0.0], [0.0, 0.0])];

        // Act
        let err = chain_into_closed_contour(&segments).unwrap_err();

        // Assert
        assert_eq!(err, ChainError::DegenerateResult);
    }

    #[test]
    fn endpoints_within_epsilon_still_chain() {
        // Arrange: a triangle where one joint is off by 0.005 mm — under
        // CHAIN_ENDPOINT_EPSILON_MM (0.01).
        let a = [0.0, 0.0];
        let b = [4.0, 0.0];
        let c = [2.0, 3.0];
        let a_near = [a[0] + 0.005, a[1]];
        let segments = [line(a, b), line(b, c), line(c, a_near)];

        // Act
        let ring = chain_into_closed_contour(&segments);

        // Assert
        assert!(ring.is_ok(), "sub-epsilon gap should still chain");
        assert_eq!(ring.unwrap().len(), 3);
    }

    #[test]
    fn endpoints_beyond_epsilon_report_open_chain() {
        // Arrange: same triangle, but the joint is off by 0.02 mm — over
        // CHAIN_ENDPOINT_EPSILON_MM (0.01).
        let a = [0.0, 0.0];
        let b = [4.0, 0.0];
        let c = [2.0, 3.0];
        let a_far = [a[0] + 0.02, a[1]];
        let segments = [line(a, b), line(b, c), line(c, a_far)];

        // Act
        let err = chain_into_closed_contour(&segments).unwrap_err();

        // Assert
        match err {
            ChainError::OpenChain { gap_mm } => assert_approx_eq(gap_mm, 0.02),
            other => panic!("expected OpenChain, got {other:?}"),
        }
    }

    #[test]
    fn full_circle_arc_alone_closes_into_a_ring() {
        // Arrange: a single Arc with start_deg == end_deg — per the
        // module doc comment, this chains as a full 360° circle rather
        // than a zero-length degenerate arc.
        let segments = [ChainSegment::Arc {
            center: [1.0, 1.0],
            radius: 2.0,
            start_deg: 40.0,
            end_deg: 40.0,
        }];

        // Act
        let ring = chain_into_closed_contour(&segments).expect("full circle should chain");

        // Assert: CHAIN_ARC_SAMPLES vertices, all on the circle, CCW.
        assert_eq!(ring.len(), CHAIN_ARC_SAMPLES);
        for p in &ring {
            assert_approx_eq(dist_sq(*p, [1.0, 1.0]).sqrt(), 2.0);
        }
        assert!(signed_area_x2(&ring) > 0.0);
    }

    #[test]
    fn empty_input_reports_empty() {
        assert_eq!(
            chain_into_closed_contour(&[]).unwrap_err(),
            ChainError::Empty
        );
    }
}
