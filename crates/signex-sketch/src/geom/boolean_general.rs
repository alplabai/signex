//! General polygon boolean operations — union, intersection,
//! and difference — via the Greiner-Hormann clipping pattern.
//!
//! Handles concave subject polygons against concave clip polygons
//! and returns a list of result rings (booleans can produce
//! multiple disjoint output polygons even from connected inputs).
//!
//! # Result contract
//!
//! Each ring is an independent closed polygon. A ring with a
//! negative signed area is a hole of the enclosing positive-area
//! (outer) ring in the same result; consumers that treat every ring
//! as an island (for example a ring offset) must account for that.
//! Holes come from a Difference whose clip lies strictly inside the
//! subject, and from a walked Union that encloses a gap.
//!
//! Output winding depends on the path. Walked results are not
//! normalised: outer rings come out CCW and a walked hole comes out
//! CW. The no-intersection shortcuts (disjoint and containment)
//! return the caller's input rings as given, except the
//! Difference hole branch, which returns the subject normalised to
//! CCW plus the clip reversed to CW. Callers that need a fixed
//! winding must normalise themselves.
//!
//! # Limitations
//!
//! This is the non-degenerate variant: it assumes intersections
//! between edges occur strictly in their interiors. Shared edges,
//! shared vertices and a vertex of one ring lying on the other ring's
//! boundary are NOT computed; [`polygon_op`] refuses them with
//! [`PolygonOpError::BoundaryContact`] instead of guessing. Callers
//! that need those cases must handle the error (keep the inputs
//! separate, or pre-perturb them) or fall back to
//! [`super::boolean::intersect_convex_clip`] when the clip is convex.
//! Every other failure (an odd crossing count, a walk that runs out of
//! steps or hits an intersection with no partner) is also an error,
//! never a partial ring.
//!
//! # Algorithm overview
//!
//! 1. Build doubly-linked vertex rings for both polygons. Each
//!    vertex carries an `intersect` flag, an `entry` flag, a
//!    `neighbor` link to the matching vertex in the other ring,
//!    and an `alpha` value for sorted insertion.
//! 2. Walk every pair of (subject edge, clip edge), compute
//!    intersections, and insert a paired vertex into both rings
//!    at the right alpha-sorted edge positions.
//! 3. Classify every intersection as "entry" or "exit" relative to
//!    the OTHER polygon's interior. Done by tracking running
//!    inside-state during a single ring walk per polygon, seeded
//!    from a point-in-polygon test on the first non-intersection
//!    vertex.
//! 4. For each unvisited start intersection, walk the result ring:
//!    advance through the current polygon, switch polygons at the
//!    next intersection, and continue until returning to the start.
//!    The start rule depends on the operation (see `flips`): the
//!    subject intersections whose `entry` flag differs from the
//!    subject's inversion. The walk direction is chosen at each
//!    intersection (entry flag, inverted per ring for the operation)
//!    and persists until the next intersection.

use super::Point2;
use super::predicates::signed_area;

/// Why [`polygon_op`] refused to produce a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PolygonOpError {
    /// An input ring has fewer than three vertices.
    #[error("polygon boolean input ring has fewer than 3 vertices")]
    TooFewVertices,
    /// A vertex of one ring lies on the other ring's boundary (shared
    /// edge, shared vertex or T-contact), which the walk cannot classify.
    #[error("polygon boolean inputs touch along a shared edge or vertex")]
    BoundaryContact,
    /// Defensive guard: the rings crossed an odd number of times, so the
    /// entry/exit classification is not trustworthy. Closed curves cross
    /// an even number of times, so this only fires on a broken tolerance.
    #[error("polygon boolean inputs cross an odd number of times")]
    OddCrossingCount,
    /// A result walk did not return to its start within its step budget.
    #[error("polygon boolean walk did not close within its step budget")]
    StepBudgetExhausted,
    /// Defensive guard: a result walk reached an intersection that has no
    /// partner vertex. Every inserted crossing is paired, so this is
    /// unreachable unless the ring construction is broken.
    #[error("polygon boolean walk reached an intersection with no partner")]
    DanglingIntersection,
    /// Defensive guard: a result walk closed with fewer than three points.
    /// A closed walk between proper crossings always has at least four.
    #[error("polygon boolean walk closed with fewer than 3 points")]
    DegenerateRing,
}

/// Boolean operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// Subject ∪ Clip — combined area.
    Union,
    /// Subject ∩ Clip — overlapping area.
    Intersection,
    /// Subject − Clip — subject minus the clip's coverage.
    Difference,
}

#[derive(Clone, Copy, Debug)]
struct Vertex {
    pos: Point2,
    next: usize,
    prev: usize,
    intersect: bool,
    entry: bool,
    neighbor: Option<usize>,
    /// Parametric position along the originating edge — used only
    /// during construction to insert intersections in sorted order
    /// along each edge.
    alpha: f64,
    visited: bool,
}

impl Vertex {
    fn corner(pos: Point2, next: usize, prev: usize) -> Self {
        Self {
            pos,
            next,
            prev,
            intersect: false,
            entry: false,
            neighbor: None,
            alpha: 0.0,
            visited: false,
        }
    }

    fn intersection(pos: Point2, alpha: f64) -> Self {
        Self {
            pos,
            next: usize::MAX,
            prev: usize::MAX,
            intersect: true,
            entry: false,
            neighbor: None,
            alpha,
            visited: false,
        }
    }
}

/// Build a closed ring from a slice of corner positions. The
/// returned Vec's `next` / `prev` form a circular doubly-linked
/// list. Returns `None` for a degenerate input (< 3 vertices).
fn build_ring(positions: &[Point2]) -> Option<Vec<Vertex>> {
    let n = positions.len();
    if n < 3 {
        return None;
    }
    let mut verts: Vec<Vertex> = Vec::with_capacity(n);
    for (i, &pos) in positions.iter().enumerate() {
        let next = (i + 1) % n;
        let prev = (i + n - 1) % n;
        verts.push(Vertex::corner(pos, next, prev));
    }
    Some(verts)
}

/// Insert intersection vertex at the right edge-sorted position
/// between `start_idx` (the corner that begins the edge) and
/// whatever vertex currently follows it. Multiple intersections
/// on one edge sort by ascending `alpha`.
fn insert_after_in_alpha_order(
    verts: &mut Vec<Vertex>,
    start_idx: usize,
    new_vertex: Vertex,
) -> usize {
    let new_idx = verts.len();
    verts.push(new_vertex);
    // Walk forward from start_idx until we find the insertion
    // point: either the next corner (intersect = false on the
    // outbound side) or an intersection with greater alpha.
    let mut cur = start_idx;
    loop {
        let next = verts[cur].next;
        // The next vertex is the edge endpoint corner if it's
        // not an intersection — stop here.
        if !verts[next].intersect {
            break;
        }
        // It IS an intersection on the same edge — compare alpha.
        if verts[new_idx].alpha < verts[next].alpha {
            break;
        }
        cur = next;
    }
    let next_idx = verts[cur].next;
    verts[cur].next = new_idx;
    verts[new_idx].prev = cur;
    verts[new_idx].next = next_idx;
    verts[next_idx].prev = new_idx;
    new_idx
}

/// Even-odd point-in-polygon test against the linked ring rooted
/// at `head`. Walks the ring's `next` chain to gather edges.
fn point_in_ring(verts: &[Vertex], head: usize, p: Point2) -> bool {
    let mut inside = false;
    let mut cur = head;
    let start = head;
    loop {
        let nxt = verts[cur].next;
        let a = verts[cur].pos;
        let b = verts[nxt].pos;
        let denom = b.y - a.y;
        if denom.abs() > 1e-10 {
            let intersect =
                ((a.y > p.y) != (b.y > p.y)) && (p.x < (b.x - a.x) * (p.y - a.y) / denom + a.x);
            if intersect {
                inside = !inside;
            }
        }
        cur = nxt;
        if cur == start {
            break;
        }
    }
    inside
}

/// Absolute distance (in input units) below which two features count
/// as touching. Shared by the boundary pre-check and the crossing
/// finder so a crossing the finder drops is always caught as contact.
const CONTACT_TOL: f64 = 1e-9;

/// Closed curves cross an even number of times; an odd count means the
/// crossing tolerance misclassified a contact.
fn require_even_crossings(count: usize) -> Result<(), PolygonOpError> {
    if count % 2 == 0 {
        Ok(())
    } else {
        Err(PolygonOpError::OddCrossingCount)
    }
}

/// True when `p` lies within [`CONTACT_TOL`] of the boundary of `ring`.
fn on_boundary(ring: &[Point2], p: Point2) -> bool {
    let n = ring.len();
    (0..n).any(|i| {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (qx, qy) = (a.x + t * dx, a.y + t * dy);
        (p.x - qx).hypot(p.y - qy) <= CONTACT_TOL
    })
}

/// Find the first non-intersection vertex starting from `start`.
/// Required for the entry/exit classifier seed: we need a vertex
/// from the original polygon (not an inserted intersection) so
/// we can ask "is this point inside the other polygon?" without
/// the intersection point sitting on its boundary.
fn first_corner(verts: &[Vertex], start: usize) -> Option<usize> {
    let mut cur = start;
    let mut steps = 0;
    while verts[cur].intersect {
        cur = verts[cur].next;
        steps += 1;
        if steps > verts.len() {
            return None;
        }
    }
    Some(cur)
}

/// Walk the whole ring once and assign `entry` to each
/// intersection: alternates from the seed inside-state, true →
/// false → true → … through the intersections in ring order.
///
/// `inside` is the running state — `true` when the current
/// position is inside the OTHER polygon. We flip it at each
/// intersection. The intersection's `entry` flag is set to
/// `!inside` before the flip, so an intersection is an "entry"
/// when the walker is OUTSIDE the other polygon and is about to
/// step inside.
fn classify_entries(verts: &mut [Vertex], start: usize, mut inside: bool) {
    let mut cur = start;
    let total = verts.len();
    for _ in 0..total {
        if verts[cur].intersect {
            verts[cur].entry = !inside;
            inside = !inside;
        }
        cur = verts[cur].next;
    }
}

/// Which rings have their `entry` flags inverted for `op`. A
/// flipped ring is walked forward at exits instead of entries.
/// Intersection: neither. Union: both. Difference (subject − clip):
/// subject only, so the walk follows the subject outside the clip
/// and then the clip backwards through the subject.
fn flips(op: BoolOp) -> (bool, bool) {
    match op {
        BoolOp::Intersection => (false, false),
        BoolOp::Union => (true, true),
        BoolOp::Difference => (true, false),
    }
}

/// Walk one output ring starting at the subject intersection
/// `start`. The direction of travel is decided at every
/// intersection the walk lands on (`entry` xor the ring's flip) and
/// persists along the corners until the next intersection.
/// Marks every intersection visited in BOTH rings. Returns an error
/// rather than a partial ring when the walk cannot close.
fn walk_one_ring(
    subject: &mut [Vertex],
    clip: &mut [Vertex],
    start: usize,
    flip: (bool, bool),
    step_budget: usize,
) -> Result<Vec<Point2>, PolygonOpError> {
    let mut out: Vec<Point2> = vec![subject[start].pos];
    subject[start].visited = true;
    if let Some(nb) = subject[start].neighbor {
        clip[nb].visited = true;
    }
    let mut on_subject = true;
    let mut cur = start;
    let mut forward = subject[start].entry != flip.0;
    for _ in 0..step_budget {
        let ring: &[Vertex] = if on_subject { subject } else { clip };
        cur = if forward {
            ring[cur].next
        } else {
            ring[cur].prev
        };
        let v = ring[cur];
        if !v.intersect {
            out.push(v.pos);
            continue;
        }
        let Some(nb) = v.neighbor else {
            return Err(PolygonOpError::DanglingIntersection);
        };
        let (s_idx, c_idx) = if on_subject { (cur, nb) } else { (nb, cur) };
        subject[s_idx].visited = true;
        clip[c_idx].visited = true;
        if s_idx == start {
            return if out.len() >= 3 {
                Ok(out)
            } else {
                Err(PolygonOpError::DegenerateRing)
            };
        }
        out.push(v.pos);
        on_subject = !on_subject;
        cur = nb;
        forward = if on_subject {
            subject[cur].entry != flip.0
        } else {
            clip[cur].entry != flip.1
        };
    }
    Err(PolygonOpError::StepBudgetExhausted)
}

/// Copy of `poly` wound counter-clockwise.
fn normalise_ccw(poly: &[Point2]) -> Vec<Point2> {
    let mut v = poly.to_vec();
    if signed_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// Compute `subject ∩ clip`, `subject ∪ clip`, or `subject − clip`
/// for general (concave) polygons. Returns a list of result rings;
/// boolean operations on connected inputs can yield multiple
/// disjoint output polygons.
///
/// Disjoint inputs short-circuit:
/// - Disjoint + `Intersection` → empty.
/// - Disjoint + `Union` → both polygons.
/// - Disjoint + `Difference` → subject only.
///
/// Subject fully inside clip:
/// - `Intersection` → subject.
/// - `Union` → clip.
/// - `Difference` → empty.
///
/// Clip fully inside subject:
/// - `Intersection` → clip.
/// - `Union` → subject.
/// - `Difference` → two rings: the subject as the outer CCW ring
///   and the clip as a CW hole (negative signed area).
///
/// Inputs that touch (shared edge, shared vertex, a vertex on the
/// other ring's boundary) are outside the non-degenerate contract and
/// return [`PolygonOpError::BoundaryContact`].
///
/// Inputs may wind either way; both are normalised to CCW before the
/// walk. A negative signed area ring is a hole of the enclosing outer
/// ring (see the module doc). Output winding depends on the path:
/// walked outer rings are CCW and walked holes CW, the
/// disjoint/containment shortcuts return the inputs as given, and the
/// Difference hole branch returns CCW outer plus CW hole.
///
/// # Errors
///
/// Returns a [`PolygonOpError`] for degenerate input (see the module
/// doc) instead of a plausible-looking wrong answer.
pub fn polygon_op(
    subject: &[Point2],
    clip: &[Point2],
    op: BoolOp,
) -> Result<Vec<Vec<Point2>>, PolygonOpError> {
    polygon_op_budgeted(subject, clip, op, None)
}

/// [`polygon_op`] with an optional override of the per-walk step
/// budget; the override exists so tests can exhaust it.
fn polygon_op_budgeted(
    subject: &[Point2],
    clip: &[Point2],
    op: BoolOp,
    budget_override: Option<usize>,
) -> Result<Vec<Vec<Point2>>, PolygonOpError> {
    // The walk assumes both rings wind the same way; normalise to CCW.
    let subject_ccw = normalise_ccw(subject);
    let clip_ccw = normalise_ccw(clip);
    let mut subj = build_ring(&subject_ccw).ok_or(PolygonOpError::TooFewVertices)?;
    let mut cl = build_ring(&clip_ccw).ok_or(PolygonOpError::TooFewVertices)?;

    // Shared edges, shared vertices and T-contacts put a vertex on the
    // other ring's boundary. Refuse them: the containment shortcuts and
    // the entry/exit seeds below test single vertices and are only
    // sound when no vertex is on a boundary.
    let touches = subject_ccw.iter().any(|&v| on_boundary(&clip_ccw, v))
        || clip_ccw.iter().any(|&v| on_boundary(&subject_ccw, v));
    if touches {
        return Err(PolygonOpError::BoundaryContact);
    }

    // Phase 1 — find every intersection between subject edge and
    // clip edge, insert into both rings.
    let n_subj_corners = subj.len();
    let n_clip_corners = cl.len();
    let mut had_intersection = false;
    for si in 0..n_subj_corners {
        for ci in 0..n_clip_corners {
            let s_a = subj[si].pos;
            let s_b = subj[(si + 1) % n_subj_corners].pos;
            let c_a = cl[ci].pos;
            let c_b = cl[(ci + 1) % n_clip_corners].pos;
            if let Some((pt, t_s, t_c)) = proper_segment_intersection(s_a, s_b, c_a, c_b) {
                let s_inserted =
                    insert_after_in_alpha_order(&mut subj, si, Vertex::intersection(pt, t_s));
                let c_inserted =
                    insert_after_in_alpha_order(&mut cl, ci, Vertex::intersection(pt, t_c));
                subj[s_inserted].neighbor = Some(c_inserted);
                cl[c_inserted].neighbor = Some(s_inserted);
                had_intersection = true;
            }
        }
    }

    if !had_intersection {
        // No crossings and no boundary contact: the rings are disjoint
        // or nested, so one vertex decides.
        let subj_inside = point_in_ring(&cl, 0, subj[0].pos);
        let clip_inside = point_in_ring(&subj, 0, cl[0].pos);
        return Ok(match (op, subj_inside, clip_inside) {
            (BoolOp::Intersection, true, _) => vec![subject.to_vec()],
            (BoolOp::Intersection, _, true) => vec![clip.to_vec()],
            (BoolOp::Intersection, _, _) => Vec::new(),
            (BoolOp::Union, true, _) => vec![clip.to_vec()],
            (BoolOp::Union, _, true) => vec![subject.to_vec()],
            (BoolOp::Union, _, _) => vec![subject.to_vec(), clip.to_vec()],
            (BoolOp::Difference, true, _) => Vec::new(),
            (BoolOp::Difference, _, true) => {
                // Subject as the outer CCW ring, clip as a CW hole.
                let mut hole = clip_ccw.clone();
                hole.reverse();
                vec![subject_ccw.clone(), hole]
            }
            (BoolOp::Difference, _, _) => vec![subject.to_vec()],
        });
    }

    require_even_crossings(subj.iter().filter(|v| v.intersect).count())?;

    // Phase 2 — classify entries and exits.
    let subj_seed = first_corner(&subj, 0).ok_or(PolygonOpError::TooFewVertices)?;
    let clip_seed = first_corner(&cl, 0).ok_or(PolygonOpError::TooFewVertices)?;
    let subj_starts_inside_clip = point_in_ring(&cl, 0, subj[subj_seed].pos);
    let clip_starts_inside_subj = point_in_ring(&subj, 0, cl[clip_seed].pos);
    classify_entries(&mut subj, subj_seed, subj_starts_inside_clip);
    classify_entries(&mut cl, clip_seed, clip_starts_inside_subj);

    // Phase 3 — walk result rings starting at every unvisited
    // intersection that matches the operation's start rule.
    let flip = flips(op);
    let budget = budget_override.unwrap_or(2 * (subj.len() + cl.len()) + 4);
    let mut rings: Vec<Vec<Point2>> = Vec::new();
    for i in 0..subj.len() {
        // Start at unvisited subject intersections the walk leaves
        // forward along the subject.
        if subj[i].intersect && !subj[i].visited && subj[i].entry != flip.0 {
            rings.push(walk_one_ring(&mut subj, &mut cl, i, flip, budget)?);
        }
    }
    Ok(rings)
}

/// Strict-interior segment×segment intersection. Returns the hit
/// point and the parametric `(t_subject, t_clip)` only when the hit
/// is more than [`CONTACT_TOL`] (absolute distance) from every
/// endpoint. A hit closer than that is a vertex contact, which
/// `on_boundary` reports before any walk starts.
fn proper_segment_intersection(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
) -> Option<(Point2, f64, f64)> {
    let r = (b.x - a.x, b.y - a.y);
    let s = (d.x - c.x, d.y - c.y);
    let denom = r.0 * s.1 - r.1 * s.0;
    let (r_len, s_len) = (r.0.hypot(r.1), s.0.hypot(s.1));
    // Relative parallel test: |sin(angle)| below 1e-12.
    if denom.abs() <= 1e-12 * r_len * s_len {
        return None;
    }
    let qmp = (c.x - a.x, c.y - a.y);
    let t = (qmp.0 * s.1 - qmp.1 * s.0) / denom;
    let u = (qmp.0 * r.1 - qmp.1 * r.0) / denom;
    let clear =
        |param: f64, len: f64| param * len > CONTACT_TOL && (1.0 - param) * len > CONTACT_TOL;
    if clear(t, r_len) && clear(u, s_len) {
        Some((Point2::new(a.x + t * r.0, a.y + t * r.1), t, u))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::predicates::signed_area;
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    /// Run `polygon_op` on inputs the test expects to be computable.
    fn op(a: &[Point2], b: &[Point2], o: BoolOp) -> Vec<Vec<Point2>> {
        polygon_op(a, b, o).expect("computable inputs")
    }

    fn area(poly: &[Point2]) -> f64 {
        signed_area(poly).abs()
    }

    #[test]
    fn disjoint_intersection_is_empty() {
        let a = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let b = vec![p(2.0, 2.0), p(3.0, 2.0), p(3.0, 3.0), p(2.0, 3.0)];
        let out = op(&a, &b, BoolOp::Intersection);
        assert!(out.is_empty() || out.iter().all(|r| area(r) < 1e-12));
    }

    #[test]
    fn disjoint_union_is_two_polygons() {
        let a = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let b = vec![p(2.0, 2.0), p(3.0, 2.0), p(3.0, 3.0), p(2.0, 3.0)];
        let out = op(&a, &b, BoolOp::Union);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn disjoint_difference_is_subject() {
        let a = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let b = vec![p(2.0, 2.0), p(3.0, 2.0), p(3.0, 3.0), p(2.0, 3.0)];
        let out = op(&a, &b, BoolOp::Difference);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn union_emits_each_intersection_once() {
        // #1413: the old walker pushed each ring-switch intersection twice.
        let a = vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)];
        let b = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = op(&a, &b, BoolOp::Union);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 8, "two crossings, no repeats: {:?}", out[0]);
        for (i, v) in out[0].iter().enumerate() {
            let w = out[0][(i + 1) % out[0].len()];
            assert!(
                (v.x - w.x).abs() > 1e-9 || (v.y - w.y).abs() > 1e-9,
                "zero-length edge at {i}: {:?}",
                out[0]
            );
        }
    }

    #[test]
    fn quarter_overlap_intersection_unit_area() {
        let a = vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)];
        let b = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = op(&a, &b, BoolOp::Intersection);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn fully_contained_intersection_returns_inner() {
        let outer = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let inner = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = op(&inner, &outer, BoolOp::Intersection);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn fully_contained_union_returns_outer() {
        let outer = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let inner = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = op(&inner, &outer, BoolOp::Union);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 16.0).abs() < 1e-9);
    }

    fn sq(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point2> {
        vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
    }

    fn reversed(mut v: Vec<Point2>) -> Vec<Point2> {
        v.reverse();
        v
    }

    fn has_vertex(ring: &[Point2], x: f64, y: f64) -> bool {
        ring.iter()
            .any(|q| (q.x - x).abs() < 1e-9 && (q.y - y).abs() < 1e-9)
    }

    fn assert_l_shape(out: &[Vec<Point2>]) {
        assert_eq!(out.len(), 1, "{out:?}");
        let r = &out[0];
        assert!((area(r) - 3.0).abs() < 1e-9, "{r:?}");
        assert_eq!(r.len(), 6, "{r:?}");
        for (x, y) in [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ] {
            assert!(has_vertex(r, x, y), "missing ({x},{y}) in {r:?}");
        }
    }

    #[test]
    fn overlapping_difference_is_l_shape() {
        let out = op(
            &sq(0.0, 0.0, 2.0, 2.0),
            &sq(1.0, 1.0, 3.0, 3.0),
            BoolOp::Difference,
        );
        assert_l_shape(&out);
    }

    #[test]
    fn difference_mixed_winding_is_l_shape() {
        let a = sq(0.0, 0.0, 2.0, 2.0);
        let b = sq(1.0, 1.0, 3.0, 3.0);
        assert_l_shape(&op(&reversed(a.clone()), &b, BoolOp::Difference));
        assert_l_shape(&op(&a, &reversed(b.clone()), BoolOp::Difference));
        assert_l_shape(&op(&reversed(a), &reversed(b), BoolOp::Difference));
    }

    #[test]
    fn difference_clip_inside_returns_outer_and_hole() {
        let outer = sq(0.0, 0.0, 4.0, 4.0);
        let inner = sq(1.0, 1.0, 3.0, 3.0);
        let out = op(&outer, &inner, BoolOp::Difference);
        assert_eq!(out.len(), 2);
        assert!((signed_area(&out[0]) - 16.0).abs() < 1e-9);
        assert!((signed_area(&out[1]) + 4.0).abs() < 1e-9);
        for (x, y) in [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)] {
            assert!(has_vertex(&out[1], x, y));
        }
    }

    #[test]
    fn difference_subject_inside_clip_is_empty() {
        let out = op(
            &sq(1.0, 1.0, 2.0, 2.0),
            &sq(0.0, 0.0, 4.0, 4.0),
            BoolOp::Difference,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn difference_splitting_subject_gives_two_rings() {
        let out = op(
            &sq(0.0, 0.0, 3.0, 1.0),
            &sq(1.0, -1.0, 2.0, 2.0),
            BoolOp::Difference,
        );
        assert_eq!(out.len(), 2, "{out:?}");
        for r in &out {
            assert!((signed_area(r) - 1.0).abs() < 1e-9, "{r:?}");
        }
        assert!(out.iter().any(|r| has_vertex(r, 0.0, 0.0)));
        assert!(out.iter().any(|r| has_vertex(r, 3.0, 1.0)));
    }

    #[test]
    fn union_enclosing_a_notch_emits_a_hole() {
        let u = vec![
            p(0.0, 0.0),
            p(3.0, 0.0),
            p(3.0, 3.0),
            p(2.0, 3.0),
            p(2.0, 1.0),
            p(1.0, 1.0),
            p(1.0, 3.0),
            p(0.0, 3.0),
        ];
        let out = op(&u, &sq(-1.0, 2.0, 4.0, 4.0), BoolOp::Union);
        assert_eq!(out.len(), 2, "{out:?}");
        let hole = out.iter().find(|r| signed_area(r) < 0.0).expect("a hole");
        assert!((signed_area(hole) + 1.0).abs() < 1e-9, "{hole:?}");
        for (x, y) in [(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)] {
            assert!(has_vertex(hole, x, y), "missing ({x},{y}) in {hole:?}");
        }
        let outer = out.iter().find(|r| signed_area(r) > 0.0).expect("an outer");
        // Net area = outer minus the 1x1 notch hole.
        let net: f64 = out.iter().map(|r| signed_area(r)).sum();
        assert!((net - (signed_area(outer) - 1.0)).abs() < 1e-9);
        assert!((net - 15.0).abs() < 1e-9, "net {net}");
    }

    #[test]
    fn issue_1448_shared_edge_is_refused_not_wrong() {
        let subject = vec![p(0.0, 3.0), p(2.0, 3.0), p(2.0, 1.0), p(0.0, 1.0)];
        let clip = vec![p(1.0, 5.0), p(2.0, 5.0), p(2.0, 2.0), p(1.0, 2.0)];
        // True areas would be 6, 1 and 3; shared edges are refused
        // outright rather than computed, so no wrong area can escape.
        for o in [BoolOp::Union, BoolOp::Intersection, BoolOp::Difference] {
            assert_eq!(
                polygon_op(&subject, &clip, o),
                Err(PolygonOpError::BoundaryContact)
            );
        }
    }

    #[test]
    fn shared_edge_difference_is_refused() {
        let r = polygon_op(
            &sq(0.0, 0.0, 2.0, 2.0),
            &sq(1.0, 0.0, 3.0, 2.0),
            BoolOp::Difference,
        );
        assert_eq!(r, Err(PolygonOpError::BoundaryContact));
    }

    #[test]
    fn adjacent_squares_sharing_a_whole_edge_are_refused() {
        for o in [BoolOp::Union, BoolOp::Intersection, BoolOp::Difference] {
            let r = polygon_op(&sq(0.0, 0.0, 1.0, 1.0), &sq(1.0, 0.0, 2.0, 1.0), o);
            assert_eq!(r, Err(PolygonOpError::BoundaryContact), "{o:?}");
        }
    }

    #[test]
    fn shared_vertex_is_refused() {
        for o in [BoolOp::Union, BoolOp::Intersection, BoolOp::Difference] {
            let r = polygon_op(&sq(0.0, 0.0, 1.0, 1.0), &sq(1.0, 1.0, 2.0, 2.0), o);
            assert_eq!(r, Err(PolygonOpError::BoundaryContact), "{o:?}");
        }
    }

    #[test]
    fn vertex_touching_an_edge_from_inside_is_refused() {
        // Inner square touches the outer's right edge: the single-vertex
        // containment test used to pick a shortcut here.
        let outer = sq(0.0, 0.0, 4.0, 4.0);
        let inner = sq(1.0, 1.0, 4.0, 3.0);
        for o in [BoolOp::Union, BoolOp::Intersection, BoolOp::Difference] {
            assert_eq!(
                polygon_op(&outer, &inner, o),
                Err(PolygonOpError::BoundaryContact),
                "{o:?}"
            );
        }
    }

    #[test]
    fn near_vertex_crossing_never_yields_a_wrong_area() {
        // A clip vertex sits `d` inside a long subject edge. The result must be the
        // true area or an error, never the old wrong 100 / 10 / 90.
        let subject = sq(0.0, 0.0, 10.0, 10.0);
        for d in [5e-9, 1e-8, 1e-6] {
            let clip = vec![p(10.0 - d, 5.0), p(20.0, 4.0), p(20.0, 6.0)];
            // Clip triangle area outside the subject is the whole triangle
            // minus a sliver of width d; the overlap is ~ d * 0.5 * (2/10).
            let overlap = 0.5 * d * (2.0 * d / 10.0);
            let clip_area = 0.5 * 10.0 * 2.0;
            for (o, want) in [
                (BoolOp::Union, 100.0 + clip_area - overlap),
                (BoolOp::Intersection, overlap),
                (BoolOp::Difference, 100.0 - overlap),
            ] {
                if let Ok(out) = polygon_op(&subject, &clip, o) {
                    let net: f64 = out.iter().map(|r| signed_area(r)).sum();
                    assert!((net - want).abs() < 1e-3, "d={d} {o:?}: {net} vs {want}");
                }
            }
        }
    }

    #[test]
    fn odd_crossing_count_is_an_error() {
        assert_eq!(
            require_even_crossings(3),
            Err(PolygonOpError::OddCrossingCount)
        );
        assert_eq!(require_even_crossings(4), Ok(()));
    }

    #[test]
    fn too_few_vertices_is_an_error() {
        let line = vec![p(0.0, 0.0), p(1.0, 0.0)];
        assert_eq!(
            polygon_op(&line, &sq(0.0, 0.0, 1.0, 1.0), BoolOp::Union),
            Err(PolygonOpError::TooFewVertices)
        );
    }

    #[test]
    fn exhausted_step_budget_is_an_error_not_a_partial_ring() {
        let a = sq(0.0, 0.0, 2.0, 2.0);
        let b = sq(1.0, 1.0, 3.0, 3.0);
        for o in [BoolOp::Union, BoolOp::Intersection, BoolOp::Difference] {
            assert_eq!(
                polygon_op_budgeted(&a, &b, o, Some(1)),
                Err(PolygonOpError::StepBudgetExhausted),
                "{o:?}"
            );
            // The default budget still succeeds on the same input.
            assert!(polygon_op(&a, &b, o).is_ok());
        }
    }

    #[test]
    fn mixed_winding_union_and_intersection() {
        let a = sq(0.0, 0.0, 2.0, 2.0);
        let b = reversed(sq(1.0, 1.0, 3.0, 3.0));
        let u = op(&a, &b, BoolOp::Union);
        assert_eq!(u.len(), 1, "{u:?}");
        assert!((area(&u[0]) - 7.0).abs() < 1e-9, "{u:?}");
        assert_eq!(u[0].len(), 8);
        for (x, y) in [(0.0, 0.0), (3.0, 3.0), (2.0, 1.0), (1.0, 2.0)] {
            assert!(has_vertex(&u[0], x, y), "missing ({x},{y}) in {:?}", u[0]);
        }
        let i = op(&reversed(a), &b, BoolOp::Intersection);
        assert_eq!(i.len(), 1, "{i:?}");
        assert!((area(&i[0]) - 1.0).abs() < 1e-9);
        for (x, y) in [(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)] {
            assert!(has_vertex(&i[0], x, y), "missing ({x},{y}) in {:?}", i[0]);
        }
    }
}
