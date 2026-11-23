//! The inside-corner check of a board sketch (mockup row 179d): the router bit that
//! cuts the outline and its cutouts cannot cut a corner tighter than its own
//! radius. A corner matters where the router runs on its convex side:
//! a concave corner of the outline (the router runs outside the board) and
//! a convex corner of a cutout (the router runs inside the opening). A
//! sharp corner has radius 0 (a cutout's own edge radius when it names
//! one), a fillet arc its radius; edges meeting tangentially make no
//! corner. Drilled mounting holes are not routed and are skipped. Rules:
//! `docs/PCB_SKETCH.md`, "Outline checks".

use std::f64::consts::TAU;

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::geom::{Point2, signed_area};
use signex_sketch::id::SketchEntityId;
use signex_types::pcb::Point;

use super::Context;
use super::checks::{ClosedOutline, Finding};
use super::geometry::{boundary, circle, point};
use crate::eval_util::{build_ctx, opt_eval_mm};
use crate::profile::{ProfileEdge, trace_closed_profile_entities};
use crate::traced_loops::TracedLoops;

/// A turn smaller than this, in radians, is a tangent join, not a corner.
const TANGENT_RAD: f64 = 1e-4;

/// The side of a loop the router runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Router {
    /// Around the board outline.
    Outside,
    /// Inside an opening.
    Inside,
}

/// Every corner of the outline and of each cutout tighter than a router
/// tool of `diameter` mm cuts.
pub(super) fn tight_corners(
    ctx: &Context<'_>,
    outline: &ClosedOutline,
    diameter: f64,
) -> Vec<Finding> {
    let tool = diameter / 2.0;
    let mut out = Vec::new();
    loop_corners(ctx, outline.seed, Router::Outside, 0.0, tool, &mut out);
    let eval = build_ctx(ctx.sketch, ctx.params);
    let mut loops = TracedLoops::default();
    for entity in ctx.tagged(|e| e.board_cutout.is_some()) {
        let Some(attr) = entity.board_cutout.as_ref() else {
            continue;
        };
        let kind = BakedKind::Cutouts;
        // Records the loop, so its other edges are not seeds again.
        if !matches!(
            boundary(ctx, &mut loops, &format!("{attr:?}"), entity, kind),
            Ok(Some(_))
        ) {
            continue;
        }
        if matches!(entity.kind, EntityKind::Circle { .. }) {
            if let Ok((centre, radius)) = circle(ctx, entity, kind)
                && radius < tool
            {
                out.push(finding(entity.id, centre, radius, tool));
            }
            continue;
        }
        let edge_radius = opt_eval_mm(&attr.edge_radius_expr, &eval)
            .ok()
            .flatten()
            .filter(|r| r.is_finite() && *r > 0.0)
            .unwrap_or(0.0);
        loop_corners(ctx, entity.id, Router::Inside, edge_radius, tool, &mut out);
    }
    out
}

fn finding(entity: SketchEntityId, at: Point, radius_mm: f64, tool_radius_mm: f64) -> Finding {
    Finding::TightCorner {
        entity,
        at,
        radius_mm,
        tool_radius_mm,
    }
}

/// The corners of the loop through `seed` the router cannot cut: each
/// vertex where two edges meet at an angle (radius `corner_radius`) and
/// each arc, on the router's convex side.
fn loop_corners(
    ctx: &Context<'_>,
    seed: SketchEntityId,
    router: Router,
    corner_radius: f64,
    tool: f64,
    out: &mut Vec<Finding>,
) {
    let Ok(profile) = trace_closed_profile_entities(ctx.sketch, seed) else {
        return;
    };
    let edges = &profile.edges;
    let Some(ccw) = walk_is_ccw(ctx, edges) else {
        return;
    };
    for (i, edge) in edges.iter().enumerate() {
        let Some(entity) = entity(ctx, edge.id) else {
            continue;
        };
        if let Some((walk_ccw, radius, at)) = arc_walk(ctx, entity, edge)
            && router_side(walk_ccw, ccw, router)
            && radius < tool
        {
            out.push(finding(edge.id, at, radius, tool));
        }
        let next = &edges[(i + 1) % edges.len()];
        let turn = direction(ctx, edge, edge.to)
            .zip(direction(ctx, next, next.from))
            .map(|(a, b)| (a.0 * b.1 - a.1 * b.0).atan2(a.0 * b.0 + a.1 * b.1));
        let Some(turn) = turn else {
            continue;
        };
        // A tangent join is no corner; a full fold back has no inside.
        if turn.abs() < TANGENT_RAD || turn.abs() > std::f64::consts::PI - TANGENT_RAD {
            continue;
        }
        if router_side(turn > 0.0, ccw, router)
            && corner_radius < tool
            && let Some(at) = position(ctx, edge.to)
        {
            out.push(finding(edge.to, at, corner_radius, tool));
        }
    }
}

/// Whether a bend to the left (`left`) on a loop walked counter-clockwise
/// (`ccw`) is a corner on the router's side: convex corners face an
/// opening's router, concave ones the outline's.
fn router_side(left: bool, ccw: bool, router: Router) -> bool {
    let convex = left == ccw;
    match router {
        Router::Outside => !convex,
        Router::Inside => convex,
    }
}

/// Whether the walk goes round its loop counter-clockwise, from the area
/// of its vertices and arc middles; `None` for a loop of no area.
fn walk_is_ccw(ctx: &Context<'_>, edges: &[ProfileEdge]) -> Option<bool> {
    let mut polygon = Vec::with_capacity(edges.len() * 2);
    for edge in edges {
        let from = position(ctx, edge.from)?;
        polygon.push(Point2::new(from.x, from.y));
        if let Some((_, _, mid)) = entity(ctx, edge.id).and_then(|e| arc_walk(ctx, e, edge)) {
            polygon.push(Point2::new(mid.x, mid.y));
        }
    }
    let area = signed_area(&polygon);
    (area.abs() > f64::EPSILON).then_some(area > 0.0)
}

/// For an arc edge: whether the walk goes round its centre
/// counter-clockwise, its radius, and its middle.
fn arc_walk(ctx: &Context<'_>, entity: &Entity, edge: &ProfileEdge) -> Option<(bool, f64, Point)> {
    let EntityKind::Arc {
        center,
        start,
        end,
        sweep_ccw,
    } = entity.kind
    else {
        return None;
    };
    let c = position(ctx, center)?;
    let s = position(ctx, start)?;
    let e = position(ctx, end)?;
    let radius = (s.x - c.x).hypot(s.y - c.y);
    let a0 = (s.y - c.y).atan2(s.x - c.x);
    let a1 = (e.y - c.y).atan2(e.x - c.x);
    let sweep = if sweep_ccw {
        (a1 - a0).rem_euclid(TAU)
    } else {
        -(a0 - a1).rem_euclid(TAU)
    };
    let mid = a0 + sweep / 2.0;
    let at = Point {
        x: c.x + radius * mid.cos(),
        y: c.y + radius * mid.sin(),
    };
    // The walk runs the arc's own sweep when it enters at the start.
    Some((sweep_ccw == (edge.from == start), radius, at))
}

/// The direction the walk moves along `edge` at its endpoint `at`.
fn direction(ctx: &Context<'_>, edge: &ProfileEdge, at: SketchEntityId) -> Option<(f64, f64)> {
    let entity = entity(ctx, edge.id)?;
    let (x, y) = match entity.kind {
        EntityKind::Line { .. } => {
            let a = position(ctx, edge.from)?;
            let b = position(ctx, edge.to)?;
            (b.x - a.x, b.y - a.y)
        }
        EntityKind::Arc { center, .. } => {
            let (walk_ccw, _, _) = arc_walk(ctx, entity, edge)?;
            let c = position(ctx, center)?;
            let p = position(ctx, at)?;
            let (rx, ry) = (p.x - c.x, p.y - c.y);
            if walk_ccw { (-ry, rx) } else { (ry, -rx) }
        }
        EntityKind::Point { .. } | EntityKind::Circle { .. } => return None,
    };
    let length = x.hypot(y);
    (length > f64::EPSILON).then(|| (x / length, y / length))
}

fn entity<'a>(ctx: &Context<'a>, id: SketchEntityId) -> Option<&'a Entity> {
    ctx.sketch.entities.iter().find(|e| e.id == id)
}

fn position(ctx: &Context<'_>, id: SketchEntityId) -> Option<Point> {
    point(ctx, id, BakedKind::BoardOutline).ok()
}
