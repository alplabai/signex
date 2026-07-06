//! General polygon boolean operations — union, intersection,
//! difference, xor — via the Greiner-Hormann clipping pattern.
//!
//! Handles concave subject polygons against concave clip polygons
//! and returns a list of result rings (booleans can produce
//! multiple disjoint output polygons even from connected inputs).
//!
//! # Limitations
//!
//! This is the non-degenerate variant: it assumes intersections
//! between edges occur strictly in their interiors. Vertex-on-edge,
//! colinear edges, and shared boundaries are NOT handled — those
//! produce undefined output. Callers needing those cases should
//! pre-perturb their inputs (offset by a tiny non-uniform jitter)
//! or fall back to [`super::boolean::intersect_convex_clip`] when
//! the clip is convex.
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
//! 4. For each unvisited entry intersection, walk the result ring:
//!    advance through the current polygon, switch polygons at the
//!    next intersection, and continue until returning to the start.
//!    The walk direction depends on the operation.

use super::Point2;

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
fn insert_after_in_alpha_order(verts: &mut Vec<Vertex>, start_idx: usize, new_vertex: Vertex) -> usize {
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
/// `inside` before the flip, so an intersection is an "entry"
/// when the walker is OUTSIDE the other polygon and is about to
/// step inside.
fn classify_entries(verts: &mut Vec<Vertex>, start: usize, mut inside: bool) {
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

/// True when intersection vertex `idx` is unvisited and matches
/// the operation's "start here" rule.
fn is_walk_start(verts: &[Vertex], idx: usize, op: BoolOp) -> bool {
    if !verts[idx].intersect || verts[idx].visited {
        return false;
    }
    match op {
        // Walk subject forward when the intersection is an entry;
        // the result is the intersection (∩) — we cross from outside
        // the clip to inside, follow subject inside, switch to clip
        // when we exit, follow clip until we re-enter subject, etc.
        BoolOp::Intersection => verts[idx].entry,
        // Union: walk subject forward at exits — we leave the clip
        // and stay along subject's outside-of-clip portion, then
        // switch to clip at the next intersection.
        BoolOp::Union => !verts[idx].entry,
        // Difference (subject - clip): same as intersection on
        // subject side but swap walking direction on clip — handled
        // in the walker.
        BoolOp::Difference => verts[idx].entry,
    }
}

/// Walk one output ring starting at `start` (an intersection on
/// the subject ring). Returns the ring's vertex positions in
/// output order. Marks every intersection visited along the way
/// (in BOTH rings).
fn walk_one_ring(
    subject: &mut Vec<Vertex>,
    clip: &mut Vec<Vertex>,
    start: usize,
    op: BoolOp,
) -> Vec<Point2> {
    let mut out: Vec<Point2> = Vec::new();
    let mut on_subject = true;
    let mut cur = start;
    let mut step_budget = subject.len() * clip.len() + 4;
    loop {
        if step_budget == 0 {
            break;
        }
        step_budget -= 1;

        let v = if on_subject { &mut subject[cur] } else { &mut clip[cur] };
        if v.intersect {
            v.visited = true;
        }
        out.push(v.pos);

        // Direction of travel along the current ring. Greiner-
        // Hormann walks forward when "inside the result" and
        // backward when "outside" — we encode that via the entry
        // flag: at an entry we walk forward; at an exit we walk
        // backward in the OTHER ring after switching.
        let forward = if v.intersect {
            match op {
                BoolOp::Intersection => v.entry,
                BoolOp::Union => !v.entry,
                BoolOp::Difference => {
                    // Subject side: walk forward at entries,
                    // backward at exits.
                    // Clip side: walk backward at entries, forward
                    // at exits (the clip is logically reversed).
                    if on_subject {
                        v.entry
                    } else {
                        !v.entry
                    }
                }
            }
        } else {
            // Non-intersection vertex — keep the direction we set
            // on the most recent intersection. We stash that
            // implicitly by always using `next` for forward and
            // `prev` for backward, and we last decided direction
            // when leaving the previous intersection.
            // For a fresh non-intersection segment (e.g. just
            // started) default to forward.
            true
        };

        // Step along the current ring.
        let next_idx = if forward {
            if on_subject { subject[cur].next } else { clip[cur].next }
        } else if on_subject {
            subject[cur].prev
        } else {
            clip[cur].prev
        };

        // If we just stepped onto an intersection in the current
        // ring, jump to its neighbor in the other ring. Snapshot
        // the bits we need from the next vertex before mutating
        // either ring so the borrow checker stays happy.
        let (next_intersect, next_neighbor, next_pos) = {
            let v = if on_subject {
                &subject[next_idx]
            } else {
                &clip[next_idx]
            };
            (v.intersect, v.neighbor, v.pos)
        };
        if next_intersect {
            if let Some(nb) = next_neighbor {
                if on_subject {
                    subject[next_idx].visited = true;
                    clip[nb].visited = true;
                } else {
                    clip[next_idx].visited = true;
                    subject[nb].visited = true;
                }
                out.push(next_pos);
                on_subject = !on_subject;
                cur = nb;
                // Termination check — we've looped back to the
                // start vertex's position via the neighbor link.
                let start_pos = subject_start_pos_or_default(subject, start);
                let last = out.last().copied().unwrap_or(Point2::new(f64::INFINITY, f64::INFINITY));
                if (last.x - start_pos.x).abs() < 1e-12
                    && (last.y - start_pos.y).abs() < 1e-12
                {
                    out.pop();
                    break;
                }
                continue;
            }
        }
        cur = next_idx;
        // Termination: returned to start (subject side).
        if on_subject && cur == start {
            break;
        }
    }
    out
}

fn subject_start_pos_or_default(subject: &[Vertex], idx: usize) -> Point2 {
    subject.get(idx).map(|v| v.pos).unwrap_or(Point2::new(f64::NAN, f64::NAN))
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
/// - `Difference` → subject as outer + clip reversed as a hole
///   (callers needing hole support should request the result
///   rings separately — for now this path returns the subject
///   ring only and is documented as a limitation).
pub fn polygon_op(subject: &[Point2], clip: &[Point2], op: BoolOp) -> Vec<Vec<Point2>> {
    let mut subj = match build_ring(subject) {
        Some(v) => v,
        None => return Vec::new(),
    };
    let mut cl = match build_ring(clip) {
        Some(v) => v,
        None => return Vec::new(),
    };

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
                let s_inserted = insert_after_in_alpha_order(
                    &mut subj,
                    si,
                    Vertex::intersection(pt, t_s),
                );
                let c_inserted = insert_after_in_alpha_order(
                    &mut cl,
                    ci,
                    Vertex::intersection(pt, t_c),
                );
                subj[s_inserted].neighbor = Some(c_inserted);
                cl[c_inserted].neighbor = Some(s_inserted);
                had_intersection = true;
            }
        }
    }

    if !had_intersection {
        // No edges cross. Check containment to pick the trivial
        // case for each operation.
        let subj_inside = !subj.is_empty()
            && point_in_ring(&cl, 0, subj[0].pos);
        let clip_inside = !cl.is_empty() && point_in_ring(&subj, 0, cl[0].pos);
        return match (op, subj_inside, clip_inside) {
            (BoolOp::Intersection, true, _) => vec![subject.to_vec()],
            (BoolOp::Intersection, _, true) => vec![clip.to_vec()],
            (BoolOp::Intersection, _, _) => Vec::new(),
            (BoolOp::Union, true, _) => vec![clip.to_vec()],
            (BoolOp::Union, _, true) => vec![subject.to_vec()],
            (BoolOp::Union, _, _) => vec![subject.to_vec(), clip.to_vec()],
            (BoolOp::Difference, true, _) => Vec::new(),
            (BoolOp::Difference, _, true) => vec![subject.to_vec()],
            (BoolOp::Difference, _, _) => vec![subject.to_vec()],
        };
    }

    // Phase 2 — classify entries and exits.
    let subj_seed = first_corner(&subj, 0)
        .expect("non-degenerate polygon must have a non-intersection corner");
    let clip_seed = first_corner(&cl, 0)
        .expect("non-degenerate polygon must have a non-intersection corner");
    let subj_starts_inside_clip = point_in_ring(&cl, 0, subj[subj_seed].pos);
    let clip_starts_inside_subj = point_in_ring(&subj, 0, cl[clip_seed].pos);
    classify_entries(&mut subj, subj_seed, subj_starts_inside_clip);
    classify_entries(&mut cl, clip_seed, clip_starts_inside_subj);

    // Phase 3 — walk result rings starting at every unvisited
    // intersection that matches the operation's start rule.
    let mut rings: Vec<Vec<Point2>> = Vec::new();
    for i in 0..subj.len() {
        if is_walk_start(&subj, i, op) {
            let ring = walk_one_ring(&mut subj, &mut cl, i, op);
            if ring.len() >= 3 {
                rings.push(ring);
            }
        }
    }
    rings
}

/// Strict-interior segment×segment intersection. Returns the hit
/// point and the parametric `(t_subject, t_clip)` only when both
/// parameters lie strictly inside `(0, 1)` — endpoints don't
/// count as proper intersections in Greiner-Hormann.
fn proper_segment_intersection(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
) -> Option<(Point2, f64, f64)> {
    let r = (b.x - a.x, b.y - a.y);
    let s = (d.x - c.x, d.y - c.y);
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom.abs() < 1e-12 {
        return None;
    }
    let qmp = (c.x - a.x, c.y - a.y);
    let t = (qmp.0 * s.1 - qmp.1 * s.0) / denom;
    let u = (qmp.0 * r.1 - qmp.1 * r.0) / denom;
    let eps = 1e-9;
    if t > eps && t < 1.0 - eps && u > eps && u < 1.0 - eps {
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

    fn area(poly: &[Point2]) -> f64 {
        signed_area(poly).abs()
    }

    #[test]
    fn disjoint_intersection_is_empty() {
        let a = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let b = vec![p(2.0, 2.0), p(3.0, 2.0), p(3.0, 3.0), p(2.0, 3.0)];
        let out = polygon_op(&a, &b, BoolOp::Intersection);
        assert!(out.is_empty() || out.iter().all(|r| area(r) < 1e-12));
    }

    #[test]
    fn disjoint_union_is_two_polygons() {
        let a = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let b = vec![p(2.0, 2.0), p(3.0, 2.0), p(3.0, 3.0), p(2.0, 3.0)];
        let out = polygon_op(&a, &b, BoolOp::Union);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn disjoint_difference_is_subject() {
        let a = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let b = vec![p(2.0, 2.0), p(3.0, 2.0), p(3.0, 3.0), p(2.0, 3.0)];
        let out = polygon_op(&a, &b, BoolOp::Difference);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn quarter_overlap_intersection_unit_area() {
        let a = vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)];
        let b = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = polygon_op(&a, &b, BoolOp::Intersection);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn fully_contained_intersection_returns_inner() {
        let outer = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let inner = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = polygon_op(&inner, &outer, BoolOp::Intersection);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn fully_contained_union_returns_outer() {
        let outer = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let inner = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = polygon_op(&inner, &outer, BoolOp::Union);
        assert_eq!(out.len(), 1);
        assert!((area(&out[0]) - 16.0).abs() < 1e-9);
    }
}
