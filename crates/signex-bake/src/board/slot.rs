//! Whether a Slot profile is a slot (decision 2, 2026-11-20): two parallel
//! lines of equal length joined by two arcs of one radius, each tangent to
//! both lines and bulging outward. The bake refuses any other profile by
//! name, so a routed slot is never a guess. See `docs/PCB_SKETCH.md`,
//! "Targets".

use std::collections::HashMap;

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::pcb::Point;

use super::geometry::point;
use super::{BoardBakeError, Context};
use crate::profile::trace_closed_profile_entities;

/// How far, in mm, a solved position may be from the exact slot.
const TOLERANCE_MM: f64 = 1e-4;

/// The size of a slot: `width` across (twice the arc radius) and `length`
/// end to end, in mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub width: f64,
    pub length: f64,
}

/// The slot the closed profile through `seed` draws at `solve`, for the
/// Properties row; `None` when it is not one.
pub fn slot_size(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    seed: SketchEntityId,
) -> Option<Slot> {
    let params = HashMap::new();
    let ctx = Context {
        sketch,
        solve,
        params: &params,
        instances: Default::default(),
    };
    check(&ctx, seed).ok()
}

/// One edge of the profile at its solved place.
#[derive(Clone, Copy, Debug)]
enum Edge {
    Line(Point, Point),
    Arc {
        center: Point,
        start: Point,
        end: Point,
        ccw: bool,
    },
}

/// The slot the closed profile through `seed` draws, or
/// [`BoardBakeError::NotASlot`] naming `seed`.
pub(super) fn check(ctx: &Context<'_>, seed: SketchEntityId) -> Result<Slot, BoardBakeError> {
    let not_a_slot = || BoardBakeError::NotASlot { entity: seed };
    let profile = trace_closed_profile_entities(ctx.sketch, seed).map_err(|_| not_a_slot())?;
    let mut edges = Vec::with_capacity(profile.edges.len());
    for edge in &profile.edges {
        let entity = ctx
            .sketch
            .entities
            .iter()
            .find(|e| e.id == edge.id)
            .ok_or_else(not_a_slot)?;
        let at = |id| point(ctx, id, BakedKind::Cutouts);
        edges.push(match entity.kind {
            EntityKind::Line { start, end } => Edge::Line(at(start)?, at(end)?),
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => Edge::Arc {
                center: at(center)?,
                start: at(start)?,
                end: at(end)?,
                ccw: sweep_ccw,
            },
            _ => return Err(not_a_slot()),
        });
    }
    stadium(&edges).ok_or_else(not_a_slot)
}

/// The slot of four edges in walk order, or `None`.
fn stadium(edges: &[Edge]) -> Option<Slot> {
    let [a, b, c, d] = edges else {
        return None;
    };
    // Lines and arcs alternate round the loop.
    let (lines, arcs) = match (a, b, c, d) {
        (Edge::Line(..), Edge::Arc { .. }, Edge::Line(..), Edge::Arc { .. }) => ([a, c], [b, d]),
        (Edge::Arc { .. }, Edge::Line(..), Edge::Arc { .. }, Edge::Line(..)) => ([b, d], [a, c]),
        _ => return None,
    };
    let (Edge::Line(a0, a1), Edge::Line(b0, b1)) = (lines[0], lines[1]) else {
        return None;
    };
    let (first, second) = (arc_parts(arcs[0])?, arc_parts(arcs[1])?);
    let radius = first.radius;
    if radius <= TOLERANCE_MM || (radius - second.radius).abs() > TOLERANCE_MM {
        return None;
    }
    let along = unit(sub(*a1, *a0))?;
    let other = unit(sub(*b1, *b0))?;
    let lengths_match = (dist(*a0, *a1) - dist(*b0, *b1)).abs() <= TOLERANCE_MM;
    let parallel = cross(along, other).abs() <= TOLERANCE_MM;
    let apart = (cross(along, sub(*b0, *a0)).abs() - 2.0 * radius).abs() <= TOLERANCE_MM;
    if !(lengths_match && parallel && apart) {
        return None;
    }
    for (arc, away) in [(&first, &second), (&second, &first)] {
        let tangent = [arc.start, arc.end]
            .iter()
            .all(|&p| dot(sub(p, arc.center), along).abs() <= TOLERANCE_MM);
        // The arc's middle faces away from the other arc: the slot's end,
        // not a bite out of it.
        let outward = dot(sub(arc.middle, arc.center), sub(arc.center, away.center)) > 0.0;
        if !(tangent && outward) {
            return None;
        }
    }
    Some(Slot {
        width: 2.0 * radius,
        length: dist(first.center, second.center) + 2.0 * radius,
    })
}

struct ArcParts {
    center: Point,
    start: Point,
    end: Point,
    middle: Point,
    radius: f64,
}

/// An arc's solved parts, `None` when its ends are not on one circle.
fn arc_parts(edge: &Edge) -> Option<ArcParts> {
    let Edge::Arc {
        center,
        start,
        end,
        ccw,
    } = *edge
    else {
        return None;
    };
    let radius = dist(center, start);
    if (dist(center, end) - radius).abs() > TOLERANCE_MM {
        return None;
    }
    let tau = std::f64::consts::TAU;
    let from = (start.y - center.y).atan2(start.x - center.x);
    let to = (end.y - center.y).atan2(end.x - center.x);
    let sweep = if ccw {
        (to - from).rem_euclid(tau)
    } else {
        -(from - to).rem_euclid(tau)
    };
    let mid = from + sweep / 2.0;
    Some(ArcParts {
        center,
        start,
        end,
        middle: Point {
            x: center.x + radius * mid.cos(),
            y: center.y + radius * mid.sin(),
        },
        radius,
    })
}

fn sub(a: Point, b: Point) -> Point {
    Point {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn unit(v: Point) -> Option<Point> {
    let length = v.x.hypot(v.y);
    (length > TOLERANCE_MM).then(|| Point {
        x: v.x / length,
        y: v.y / length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// A 12 x 3 slot along X: bottom line, right end arc, top line, left
    /// end arc, walked counter-clockwise.
    fn slot_edges() -> Vec<Edge> {
        vec![
            Edge::Line(p(0.0, 0.0), p(9.0, 0.0)),
            Edge::Arc {
                center: p(9.0, 1.5),
                start: p(9.0, 0.0),
                end: p(9.0, 3.0),
                ccw: true,
            },
            Edge::Line(p(9.0, 3.0), p(0.0, 3.0)),
            Edge::Arc {
                center: p(0.0, 1.5),
                start: p(0.0, 3.0),
                end: p(0.0, 0.0),
                ccw: true,
            },
        ]
    }

    #[test]
    fn a_stadium_is_a_slot_with_its_width_and_length() {
        assert_eq!(
            stadium(&slot_edges()),
            Some(Slot {
                width: 3.0,
                length: 12.0
            })
        );
        let mut from_an_arc = slot_edges();
        from_an_arc.rotate_left(1);
        assert!(stadium(&from_an_arc).is_some(), "any edge may seed it");
    }

    #[test]
    fn a_rectangle_is_not_a_slot() {
        let rect = [
            Edge::Line(p(0.0, 0.0), p(9.0, 0.0)),
            Edge::Line(p(9.0, 0.0), p(9.0, 3.0)),
            Edge::Line(p(9.0, 3.0), p(0.0, 3.0)),
            Edge::Line(p(0.0, 3.0), p(0.0, 0.0)),
        ];
        assert_eq!(stadium(&rect), None);
        assert_eq!(stadium(&slot_edges()[..3]), None, "three edges");
    }

    #[test]
    fn arcs_of_two_radii_are_not_a_slot() {
        let mut edges = slot_edges();
        edges[1] = Edge::Arc {
            center: p(9.0, 1.0),
            start: p(9.0, 0.0),
            end: p(9.0, 2.0),
            ccw: true,
        };
        assert_eq!(stadium(&edges), None);
    }

    #[test]
    fn an_arc_bulging_inward_is_not_a_slot() {
        let mut edges = slot_edges();
        edges[1] = Edge::Arc {
            center: p(9.0, 1.5),
            start: p(9.0, 0.0),
            end: p(9.0, 3.0),
            ccw: false,
        };
        assert_eq!(stadium(&edges), None);
    }

    #[test]
    fn lines_that_are_not_parallel_are_not_a_slot() {
        let mut edges = slot_edges();
        edges[2] = Edge::Line(p(9.0, 3.0), p(0.0, 3.5));
        assert_eq!(stadium(&edges), None);
    }
}
