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
//! type's real interpretation exactly.
//!
//! - The point at angle `a` (degrees) on the circle is
//!   `center + radius * (cos(a), sin(a))` — standard math convention, no
//!   axis flips. Authoritative reference:
//!   `crates/signex-app/src/library/editor/symbol/state/hit_test.rs`'s
//!   `graphic_contains_point` `Arc` arm and its `ArcStart`/`ArcEnd`
//!   handle-position arms just below it.
//! - The arc always sweeps **counter-clockwise** (increasing angle) from
//!   `start_deg` to `end_deg`, wrapping through 360° when
//!   `start_deg > end_deg`. Corroborated by two independent runtime
//!   consumers computing the identical wraparound: `hit_test.rs:131-135`
//!   (`if s <= e { a >= s && a <= e } else { a >= s || a <= e }`) and
//!   `crates/signex-gfx/src/shader/arc.wgsl:52-54`'s `normalize_angle`
//!   (`sweep = normalize_angle(end_angle - start_angle)`, `in_sweep = a
//!   <= sweep`).
//!
//! `start_deg == end_deg` (mod 360°) is a **degenerate zero-sweep**
//! input, not an implicit full circle — every runtime consumer treats it
//! as a zero-length point: `hit_test.rs`'s `s <= e` range collapses to
//! the single value `[s, s]`, and `arc.wgsl`'s `sweep` is `0.0` there too
//! (`in_sweep` only true exactly on the start angle). This module rejects
//! such a segment up front — along with any other segment whose own
//! tessellated *length* is shorter than [`CHAIN_ENDPOINT_EPSILON_MM`] —
//! as [`ChainError::DegenerateSegment`], rather than silently
//! materialising a circle the user never actually drew.
//!
//! A near-360° (but not exactly zero-sweep) arc is real, hit-test-
//! selectable geometry, not degenerate — it's gated on total
//! tessellated length, not the chord between its two (possibly very
//! close together) endpoints. See [`reject_sub_epsilon_segments`]'s doc
//! comment for why the distinction matters.
//!
//! Known divergence, not this module's concern: the CPU canvas fallback
//! (`renderer_scene_canvas.rs`'s `draw_arc_bucket`) feeds
//! `start_angle`/`end_angle` straight into `iced`'s `canvas::path::Arc`
//! builder as a signed sweep, without the `normalize_angle` wraparound
//! the GPU shader and `hit_test.rs` both apply — being fixed separately.
//! This module follows the `hit_test.rs`/GPU-shader convention
//! (CCW-always, wraparound-normalised), which is the one every other
//! reader of `SymbolGraphicKind::Arc` should also treat as authoritative.
//!
//! `crates/signex-bake/src/profile.rs`'s `push_arc_interior_if_arc` uses
//! a *different* arc representation (`center`/`start`/`end` points plus
//! an explicit `sweep_ccw` flag) for PCB sketch profiles — it is not the
//! convention this module follows; `SymbolGraphicKind::Arc` carries no
//! such flag, so CCW-always (per `hit_test.rs`) is the only convention
//! consistent with the existing symbol data model.

use std::collections::{HashMap, HashSet};

/// Endpoints within this distance (mm) are treated as the same point for
/// chaining purposes.
pub const CHAIN_ENDPOINT_EPSILON_MM: f64 = 0.01;

/// Number of straight segments an [`ChainSegment::Arc`] is tessellated
/// into. Matches `signex-bake::profile::ARC_SAMPLES`.
pub const CHAIN_ARC_SAMPLES: usize = 16;

/// Doubled (×2) shoelace-area threshold (mm²) below which a closed ring
/// is treated as having ~zero area (collinear / self-overlapping input)
/// rather than a real polygon — compared directly against
/// [`signed_area_x2`]'s undivided output. Small relative to any real
/// symbol geometry (mm-scale) but well above float noise.
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
    /// A segment carries a non-finite (`NaN`/`inf`) coordinate or
    /// radius. Checked up front so bad input never masquerades as a
    /// real topology error (e.g. a silent `OpenChain { gap_mm: NaN }`).
    #[error("segment {segment_index} has a non-finite coordinate or radius")]
    InvalidInput { segment_index: usize },
    /// One *input* segment is itself degenerate — its own tessellated
    /// length (see [`reject_sub_epsilon_segments`]) is shorter than
    /// [`CHAIN_ENDPOINT_EPSILON_MM`]: a near-zero-length `Line` stub, or
    /// a zero-sweep `Arc` (`start_deg == end_deg` mod 360°; see the
    /// module doc comment). Carries the offending segment's index,
    /// unlike [`ChainError::DegenerateResult`], whose degeneracy belongs
    /// to the assembled ring as a whole rather than any single segment.
    #[error("segment {segment_index} is degenerate (near-zero length)")]
    DegenerateSegment { segment_index: usize },
    /// Every segment chains into one connected path, but it never closes
    /// back on itself. `gap_mm` is the distance between the two loose
    /// ends; `ends` are those two loose (odd-degree) endpoints
    /// themselves, so a caller can auto-close by synthesizing the
    /// missing edge between them.
    #[error("chain doesn't close — {gap_mm} mm gap between the two loose ends")]
    OpenChain { gap_mm: f64, ends: [[f64; 2]; 2] },
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
    /// enclosed area is ~zero (collinear / self-cancelling input). Not
    /// attributable to any single segment — see
    /// [`ChainError::DegenerateSegment`] for that case.
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
/// exact and easy to audit. Clustering is **transitive**: if endpoint
/// `A` is within epsilon of `B`, and `B` is within epsilon of `C`, then
/// `A` and `C` join the same node even when `A` and `C` alone are
/// farther apart than epsilon — a chain of several sub-epsilon joints
/// can span a cumulative distance larger than one epsilon.
pub fn chain_into_closed_contour(segments: &[ChainSegment]) -> Result<Vec<[f64; 2]>, ChainError> {
    if segments.is_empty() {
        return Err(ChainError::Empty);
    }
    validate_finite(segments)?;

    let polylines: Vec<Vec<[f64; 2]>> = segments.iter().map(tessellate_segment).collect();
    reject_sub_epsilon_segments(&polylines)?;

    let clusters = build_endpoint_clusters(&polylines);
    validate_topology(polylines.len(), &clusters)?;

    finalize_ring(walk_cycle(&polylines, &clusters))
}

/// Reject any segment carrying a `NaN`/`inf` coordinate or radius up
/// front. Left unchecked, non-finite input propagates silently through
/// every downstream distance/angle computation and can surface as a
/// misleading [`ChainError::OpenChain`] (`gap_mm: NaN`) or
/// [`ChainError::Disjoint`] instead of the actual problem: bad input.
fn validate_finite(segments: &[ChainSegment]) -> Result<(), ChainError> {
    for (segment_index, seg) in segments.iter().enumerate() {
        let finite = match *seg {
            ChainSegment::Line { from, to } => from.iter().chain(to.iter()).all(|v| v.is_finite()),
            ChainSegment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                center.iter().all(|v| v.is_finite())
                    && radius.is_finite()
                    && start_deg.is_finite()
                    && end_deg.is_finite()
            }
        };
        if !finite {
            return Err(ChainError::InvalidInput { segment_index });
        }
    }
    Ok(())
}

/// Reject any segment whose total tessellated *length* is shorter than
/// [`CHAIN_ENDPOINT_EPSILON_MM`], as [`ChainError::DegenerateSegment`].
///
/// Length here is [`polyline_length`] — the sum of consecutive
/// tessellated-point distances — **not** the chord (straight-line
/// distance) between the segment's two endpoints. For a `Line` (a
/// 2-point polyline) the two are identical, but for an `Arc` they
/// diverge badly near a 360° sweep: `Arc { radius: 2.0, start_deg: 0.0,
/// end_deg: 359.9 }`'s two endpoints sit only `2r·sin(0.05°) ≈ 0.0035
/// mm` apart (its chord), while its true length is `≈ 12.56 mm` — almost
/// a full circumference. Gating on the chord would reject that arc
/// outright even though it's real, hit-test-selectable geometry; gating
/// on length lets it through to normal endpoint clustering instead,
/// where its own tiny chord gap then self-closes it into a ring (see
/// the `near_full_sweep_arc_*` test in `chain_tests.rs`).
///
/// This still catches both a near-zero-length `Line` stub and a
/// zero-sweep `Arc` — its tessellated points are all the same point, so
/// its length is exactly `0.0` (see the module doc comment). Left
/// unchecked, such a stub's two coincident ends land on whatever node
/// its real endpoint touches, inflating that node's degree and faking a
/// [`ChainError::Branching`] at an otherwise clean corner.
fn reject_sub_epsilon_segments(polylines: &[Vec<[f64; 2]>]) -> Result<(), ChainError> {
    for (segment_index, poly) in polylines.iter().enumerate() {
        if polyline_length(poly) < CHAIN_ENDPOINT_EPSILON_MM {
            return Err(ChainError::DegenerateSegment { segment_index });
        }
    }
    Ok(())
}

/// Total length of a tessellated polyline — the sum of consecutive
/// point-to-point distances. For a `Line` (a 2-point polyline) this is
/// exactly its chord length; for a tessellated `Arc` it's the polygonal
/// approximation of the true arc length. See
/// [`reject_sub_epsilon_segments`] for why this, not the chord between
/// the segment's own two endpoints, is the right degeneracy gate.
fn polyline_length(poly: &[[f64; 2]]) -> f64 {
    poly.windows(2).map(|w| dist_sq(w[0], w[1]).sqrt()).sum()
}

/// Endpoint-adjacency result of clustering every segment's start/end
/// point. `node_entries`/`node_points` are keyed by cluster id (an
/// arbitrary but stable `usize`); `endpoint_node` maps a segment's own
/// `(segment_index, is_start)` endpoint directly to its cluster id.
struct EndpointClusters {
    node_entries: HashMap<usize, Vec<(usize, bool)>>,
    node_points: HashMap<usize, Vec<[f64; 2]>>,
    endpoint_node: HashMap<(usize, bool), usize>,
}

/// Cluster the `2n` segment endpoints (start + end of every segment)
/// into nodes — points within [`CHAIN_ENDPOINT_EPSILON_MM`] of each
/// other, transitively (see the [`chain_into_closed_contour`] doc
/// comment) — via a small union-find.
fn build_endpoint_clusters(polylines: &[Vec<[f64; 2]>]) -> EndpointClusters {
    let n = polylines.len();
    let eps_sq = CHAIN_ENDPOINT_EPSILON_MM * CHAIN_ENDPOINT_EPSILON_MM;

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

    let mut node_entries: HashMap<usize, Vec<(usize, bool)>> = HashMap::new();
    let mut node_points: HashMap<usize, Vec<[f64; 2]>> = HashMap::new();
    let mut endpoint_node: HashMap<(usize, bool), usize> = HashMap::new();
    for seg in 0..n {
        for is_start in [true, false] {
            let ref_idx = ref_index(seg, is_start);
            let root = uf_find(&mut parent, ref_idx);
            node_entries.entry(root).or_default().push((seg, is_start));
            node_points.entry(root).or_default().push(refs[ref_idx]);
            endpoint_node.insert((seg, is_start), root);
        }
    }

    EndpointClusters {
        node_entries,
        node_points,
        endpoint_node,
    }
}

/// Confirm `clusters` describes exactly one simple cycle spanning all
/// `n` segments: no node touched by more than two segment-ends
/// ([`ChainError::Branching`]), exactly one connected component
/// ([`ChainError::Disjoint`] otherwise), and no loose ends
/// ([`ChainError::OpenChain`] otherwise). `Ok(())` means
/// [`walk_cycle`] can run.
fn validate_topology(n: usize, clusters: &EndpointClusters) -> Result<(), ChainError> {
    for (root, entries) in &clusters.node_entries {
        if entries.len() > 2 {
            let at = average_point(&clusters.node_points[root]);
            return Err(ChainError::Branching { at });
        }
    }

    // Connected components over segments: union two segments whenever a
    // shared (degree-2) node ties them together.
    let mut seg_parent: Vec<usize> = (0..n).collect();
    let mut dangling_nodes: Vec<usize> = Vec::new();
    for (root, entries) in &clusters.node_entries {
        match entries.as_slice() {
            [(a, _), (b, _)] => uf_union(&mut seg_parent, *a, *b),
            [_] => dangling_nodes.push(*root),
            _ => unreachable!("branching nodes already rejected above"),
        }
    }
    let components: HashSet<usize> = (0..n).map(|s| uf_find(&mut seg_parent, s)).collect();
    if components.len() > 1 {
        return Err(ChainError::Disjoint);
    }

    match dangling_nodes.as_slice() {
        [] => Ok(()),
        [a, b] => {
            let end_a = average_point(&clusters.node_points[a]);
            let end_b = average_point(&clusters.node_points[b]);
            let gap_mm = dist_sq(end_a, end_b).sqrt();
            Err(ChainError::OpenChain {
                gap_mm,
                ends: [end_a, end_b],
            })
        }
        // A connected component whose nodes all have degree ≤ 2 is
        // either a simple cycle (0 dangling nodes) or a simple path
        // (exactly 2) — never anything else.
        _ => unreachable!("connected max-degree-2 component has 0 or 2 dangling nodes"),
    }
}

/// Walk the single simple cycle [`validate_topology`] already confirmed,
/// reversing each segment's tessellated polyline as needed, and return
/// the raw (not yet deduplicated / wound) point sequence.
fn walk_cycle(polylines: &[Vec<[f64; 2]>], clusters: &EndpointClusters) -> Vec<[f64; 2]> {
    let n = polylines.len();
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
        let arrived_node = clusters.endpoint_node[&(current_seg, arrived_via_is_start)];
        let (next_seg, next_is_start) = clusters.node_entries[&arrived_node]
            .iter()
            .copied()
            .find(|&(s, is_start)| !(s == current_seg && is_start == arrived_via_is_start))
            .expect("degree-2 node must have a distinct other entry");
        current_seg = next_seg;
        forward = next_is_start;
    }
    ring_raw
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
            // CCW sweep magnitude in [0, 360°) — no promotion when it
            // lands on exactly 0°. `start_deg == end_deg` (mod 360°) is
            // a real zero-sweep segment, rejected up front by
            // `reject_sub_epsilon_segments`; see the module doc comment.
            let delta = (e - s).rem_euclid(360.0);
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
