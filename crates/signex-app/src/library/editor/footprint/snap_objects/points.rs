//! The hotspots the footprint object snap chooses from, each kind behind
//! its Objects for snapping switch (#2726): fixed points (origin, centres,
//! vertices, texts, the 3D body's corners and centre) and points that
//! depend on the pointer (the nearest point of a silk line, a pad edge, a
//! region edge or a 3D body axis).

use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

use super::super::state::{EditorMode, FootprintEditorState};
use super::{ObjectTargets, pad_ring};

/// Which points [`candidates`] gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Points {
    /// Fixed points and the nearest points of lines and edges.
    All,
    /// Fixed points only: axis snapping.
    Fixed,
}

/// Every enabled hotspot for a pointer at `at`. Pads mode offers every
/// object; Sketch mode the pads and silk the sketch does not draw, and no
/// origin or body; any other mode nothing.
pub(super) fn candidates(
    at: (f64, f64),
    state: &FootprintEditorState,
    targets: ObjectTargets<'_>,
    points: Points,
) -> Vec<(f64, f64)> {
    let in_sketch = match state.mode {
        EditorMode::Normal => false,
        EditorMode::Sketch => true,
        EditorMode::View3d => return Vec::new(),
    };
    let opts = state.snap_options;
    let nearest = points == Points::All;
    let mut out = Vec::new();
    if opts.snap_footprint_origins && !in_sketch {
        out.push((0.0, 0.0));
    }
    let carried = if targets.moving_selection {
        state.selected_pad_indices()
    } else {
        Vec::new()
    };
    for (idx, pad) in state.pads.iter().enumerate() {
        let moving = targets.moving_pad == Some(idx) || carried.contains(&idx);
        if moving || (in_sketch && pad.sketch_entity_id.is_some()) {
            continue;
        }
        if opts.snap_pad_centers {
            out.push(pad.position_mm);
        }
        if opts.snap_pad_vertices {
            out.extend(pad_ring::vertices(pad));
        }
        if opts.snap_pad_edges && nearest {
            out.extend(nearest_on_ring(&pad_ring::ring(pad), at));
        }
    }
    let silk = targets
        .silk_f
        .iter()
        .chain(targets.silk_b)
        .filter(|g| !(in_sketch && g.sketch_entity.is_some()));
    for graphic in silk {
        graphic_points(graphic, state, at, nearest, &mut out);
    }
    if let Some(body) = targets.body.filter(|b| b.is_usable() && !in_sketch)
        && opts.snap_3d_body_points
    {
        out.extend(body.corners());
        out.push(body.centre());
        if nearest {
            out.extend(
                body.axes()
                    .into_iter()
                    .map(|(a, b)| nearest_on_segment(a, b, at)),
            );
        }
    }
    out.retain(|p| p.0.is_finite() && p.1.is_finite());
    out
}

/// The enabled hotspots of one silk graphic.
fn graphic_points(
    graphic: &FpGraphic,
    state: &FootprintEditorState,
    at: (f64, f64),
    nearest: bool,
    out: &mut Vec<(f64, f64)>,
) {
    let opts = state.snap_options;
    let pt = |v: &[f64; 2]| (v[0], v[1]);
    match &graphic.kind {
        FpGraphicKind::Line { from, to } => {
            if opts.snap_track_vertices {
                out.extend([pt(from), pt(to)]);
            }
            if opts.snap_track_lines && nearest {
                out.push(nearest_on_segment(pt(from), pt(to), at));
            }
        }
        FpGraphicKind::Rectangle { from, to } => {
            let ring = vec![pt(from), (to[0], from[1]), pt(to), (from[0], to[1])];
            polygon_points(&ring, graphic.filled, state, at, nearest, out);
        }
        FpGraphicKind::Polygon { vertices } => {
            let ring: Vec<(f64, f64)> = vertices.iter().map(pt).collect();
            polygon_points(&ring, graphic.filled, state, at, nearest, out);
        }
        FpGraphicKind::Circle { center, radius } => {
            if opts.snap_arc_centers {
                out.push(pt(center));
            }
            if opts.snap_track_lines && nearest {
                out.extend(nearest_on_circle(pt(center), *radius, at));
            }
        }
        FpGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            if opts.snap_arc_centers {
                out.push(pt(center));
            }
            if opts.snap_track_lines && nearest {
                out.extend(nearest_on_arc(
                    pt(center),
                    *radius,
                    (*start_deg, *end_deg),
                    at,
                ));
            }
        }
        FpGraphicKind::Text { position, .. } => {
            if opts.snap_texts {
                out.push(pt(position));
            }
        }
    }
}

/// A rectangle's or polygon's hotspots: its vertices and sides as track
/// vertices and lines, and a filled one's vertices and edges as a region.
fn polygon_points(
    ring: &[(f64, f64)],
    filled: bool,
    state: &FootprintEditorState,
    at: (f64, f64),
    nearest: bool,
    out: &mut Vec<(f64, f64)>,
) {
    let opts = state.snap_options;
    let region = filled && opts.snap_regions;
    if opts.snap_track_vertices || region {
        out.extend_from_slice(ring);
    }
    if nearest && (opts.snap_track_lines || region) {
        out.extend(nearest_on_ring(ring, at));
    }
}

/// The point of the closed polygon `ring` nearest `p`.
pub(super) fn nearest_on_ring(ring: &[(f64, f64)], p: (f64, f64)) -> Option<(f64, f64)> {
    let n = ring.len();
    (0..n)
        .map(|i| nearest_on_segment(ring[i], ring[(i + 1) % n], p))
        .min_by(|a, b| dist_sq(*a, p).total_cmp(&dist_sq(*b, p)))
}

/// The point of segment `a`-`b` nearest `p`.
pub(super) fn nearest_on_segment(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-18 {
        return a;
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0);
    (a.0 + t * dx, a.1 + t * dy)
}

/// The point of the circle nearest `p`; `None` at the centre, where every
/// point is as near.
fn nearest_on_circle(c: (f64, f64), r: f64, p: (f64, f64)) -> Option<(f64, f64)> {
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    let len = dx.hypot(dy);
    (len > 1e-12 && r > 0.0).then(|| (c.0 + dx / len * r, c.1 + dy / len * r))
}

/// The point of the arc nearest `p`: on the curve when `p`'s angle is in
/// the sweep, else the nearer end. The sweep runs from start to end through
/// increasing angle (`FpGraphicKind::arc_sweep_deg`).
fn nearest_on_arc(
    c: (f64, f64),
    r: f64,
    (start_deg, end_deg): (f64, f64),
    p: (f64, f64),
) -> Option<(f64, f64)> {
    let sweep = FpGraphicKind::arc_sweep_deg(start_deg, end_deg);
    let at = |deg: f64| {
        let t = deg.to_radians();
        (c.0 + r * t.cos(), c.1 + r * t.sin())
    };
    let angle = (p.1 - c.1).atan2(p.0 - c.0).to_degrees();
    if sweep >= 360.0 || (angle - start_deg).rem_euclid(360.0) <= sweep {
        return nearest_on_circle(c, r, p);
    }
    let (a, b) = (at(start_deg), at(start_deg + sweep));
    Some(if dist_sq(a, p) <= dist_sq(b, p) { a } else { b })
}

fn dist_sq(a: (f64, f64), p: (f64, f64)) -> f64 {
    (a.0 - p.0).powi(2) + (a.1 - p.1).powi(2)
}
