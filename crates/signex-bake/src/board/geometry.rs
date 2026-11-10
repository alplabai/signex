//! Solved positions and closed profiles as board points.

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::solver::state::{circle_radius, point_xy};
use signex_types::pcb::Point;

use super::{BoardBakeError, Context};
use crate::profile::{ARC_SAMPLES, TraceError};
use crate::traced_loops::TracedLoops;

/// Vertices of a tessellated circle: four arcs' worth of samples, so a
/// round board edge is as fine as an arc of the same profile.
pub(crate) const CIRCLE_VERTICES: usize = 4 * ARC_SAMPLES;

/// The solved position of point `id`.
pub(crate) fn point(
    ctx: &Context<'_>,
    id: SketchEntityId,
    kind: BakedKind,
) -> Result<Point, BoardBakeError> {
    let result = &ctx.solve.result;
    point_xy(id, &result.state, &result.index, ctx.sketch)
        .map(|(x, y)| Point { x, y })
        .ok_or(BoardBakeError::MissingPoint { kind, point: id })
}

/// The solved centre and radius of a circle entity.
pub(crate) fn circle(
    ctx: &Context<'_>,
    entity: &Entity,
    kind: BakedKind,
) -> Result<(Point, f64), BoardBakeError> {
    let EntityKind::Circle { center, radius } = entity.kind else {
        return Err(BoardBakeError::NotAnEdge {
            kind,
            entity: entity.id,
        });
    };
    let result = &ctx.solve.result;
    let radius = circle_radius(entity.id, &result.state, &result.index).unwrap_or(radius);
    Ok((point(ctx, center, kind)?, radius))
}

/// A circle entity as a closed polygon of [`CIRCLE_VERTICES`] corners,
/// counter-clockwise from angle 0.
pub(crate) fn circle_polygon(
    ctx: &Context<'_>,
    entity: &Entity,
    kind: BakedKind,
) -> Result<Vec<Point>, BoardBakeError> {
    let (c, r) = circle(ctx, entity, kind)?;
    Ok((0..CIRCLE_VERTICES)
        .map(|i| {
            let theta = std::f64::consts::TAU * i as f64 / CIRCLE_VERTICES as f64;
            Point {
                x: c.x + r * theta.cos(),
                y: c.y + r * theta.sin(),
            }
        })
        .collect())
}

/// The closed profile through edge `seed`, arcs tessellated, recorded in
/// `loops` under `attribute` so its other edges are not traced again.
pub(crate) fn trace(
    ctx: &Context<'_>,
    loops: &mut TracedLoops,
    attribute: &str,
    seed: SketchEntityId,
    kind: BakedKind,
) -> Result<Vec<Point>, BoardBakeError> {
    match loops.trace(attribute, ctx.sketch, ctx.solve, seed) {
        Ok(vertices) => Ok(vertices.into_iter().map(|[x, y]| Point { x, y }).collect()),
        Err(TraceError::OpenChain) => Err(BoardBakeError::OpenProfile { kind, edge: seed }),
        Err(TraceError::Branching) => Err(BoardBakeError::BranchingProfile { kind, edge: seed }),
        Err(TraceError::MissingEndpoint(point)) => {
            Err(BoardBakeError::MissingPoint { kind, point })
        }
        Err(TraceError::NotAnEdge | TraceError::CircleInProfile | TraceError::Runaway) => {
            Err(BoardBakeError::UntraceableProfile { kind, edge: seed })
        }
    }
}

/// A closed boundary from a seed entity: a circle tessellated, a line or
/// arc traced through its loop. `None` when the seed's loop was already
/// baked for this attribute.
pub(crate) fn boundary(
    ctx: &Context<'_>,
    loops: &mut TracedLoops,
    attribute: &str,
    seed: &Entity,
    kind: BakedKind,
) -> Result<Option<Vec<Point>>, BoardBakeError> {
    match seed.kind {
        EntityKind::Circle { .. } => circle_polygon(ctx, seed, kind).map(Some),
        EntityKind::Line { .. } | EntityKind::Arc { .. } => {
            if loops.contains(attribute, seed.id) {
                return Ok(None);
            }
            trace(ctx, loops, attribute, seed.id, kind).map(Some)
        }
        EntityKind::Point { .. } => Err(BoardBakeError::NotAnEdge {
            kind,
            entity: seed.id,
        }),
    }
}
