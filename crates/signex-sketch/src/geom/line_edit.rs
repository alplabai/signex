//! Geometry of the sketch Chamfer and Extend line edits: where a chamfer
//! cuts two Lines meeting at a corner, and what a Line reaches when one
//! end is extended. Pure, so the edit and its canvas ghost agree.

use super::Point2;
use super::segment::{
    Arc2, Circle2, Segment2, SegmentIntersection, segment_arc_intersections,
    segment_circle_intersections, segment_segment_intersection,
};
use crate::SketchData;
use crate::entity::EntityKind;
use crate::id::SketchEntityId;

/// Why a chamfer cannot be cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChamferRefusal {
    /// The two Lines run in one direction: there is no corner.
    Colinear,
    /// The distance reaches past the far end of a Line.
    TooLong,
}

/// The two cut points of a chamfer at `corner`, `distance` along the Line
/// towards `a` and the Line towards `b`.
pub fn chamfer_cut(
    corner: Point2,
    a: Point2,
    b: Point2,
    distance: f64,
) -> Result<(Point2, Point2), ChamferRefusal> {
    let a_len = corner.distance(a);
    let b_len = corner.distance(b);
    if a_len <= 1e-9 || b_len <= 1e-9 {
        return Err(ChamferRefusal::TooLong);
    }
    let ua = ((a.x - corner.x) / a_len, (a.y - corner.y) / a_len);
    let ub = ((b.x - corner.x) / b_len, (b.y - corner.y) / b_len);
    let cross = ua.0 * ub.1 - ua.1 * ub.0;
    if cross.abs() < 1e-6 {
        return Err(ChamferRefusal::Colinear);
    }
    // A cut at the far end would leave a zero-length Line behind.
    if distance >= a_len * 0.999 || distance >= b_len * 0.999 {
        return Err(ChamferRefusal::TooLong);
    }
    Ok((
        Point2::new(corner.x + ua.0 * distance, corner.y + ua.1 * distance),
        Point2::new(corner.x + ub.0 * distance, corner.y + ub.1 * distance),
    ))
}

/// Where an extended Line stops: the crossing point and the entity crossed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtendHit {
    pub at: Point2,
    pub target: SketchEntityId,
}

/// The nearest crossing ahead of `moving` on the ray from `fixed` through
/// `moving`, with any Line, Circle or Arc of `sketch` but `line` itself.
/// `pos` reads a Point's position (solved or authored). `None` when the
/// ray reaches nothing.
pub fn extension_hit(
    sketch: &SketchData,
    line: SketchEntityId,
    fixed: Point2,
    moving: Point2,
    pos: impl Fn(SketchEntityId) -> Option<Point2>,
) -> Option<ExtendHit> {
    let len = fixed.distance(moving);
    if len <= 1e-9 {
        return None;
    }
    let dir = ((moving.x - fixed.x) / len, (moving.y - fixed.y) / len);
    // Long enough to cross the whole sketch from `moving`.
    let reach = sketch
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Point { x, y } => Some(moving.distance(Point2::new(x, y))),
            EntityKind::Circle { radius, .. } => Some(radius),
            _ => None,
        })
        .fold(0.0_f64, |acc, d| acc + d)
        * 2.0
        + 1.0;
    let ray = Segment2::new(
        moving,
        Point2::new(moving.x + dir.0 * reach, moving.y + dir.1 * reach),
    );
    // Hits closer than this to `moving` are the end itself touching a
    // neighbour (a shared corner), not something ahead of it.
    let min_t = 1e-6 / reach;
    let mut best: Option<(f64, ExtendHit)> = None;
    let mut consider = |t: f64, at: Point2, target: SketchEntityId| {
        if t > min_t && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, ExtendHit { at, target }));
        }
    };
    for entity in &sketch.entities {
        if entity.id == line {
            continue;
        }
        match entity.kind {
            EntityKind::Line { start, end } => {
                let (Some(a), Some(b)) = (pos(start), pos(end)) else {
                    continue;
                };
                if let SegmentIntersection::Point { pt, t0, .. } =
                    segment_segment_intersection(ray, Segment2::new(a, b))
                {
                    consider(t0, pt, entity.id);
                }
            }
            EntityKind::Circle { center, radius } => {
                let Some(c) = pos(center) else { continue };
                for (pt, t) in segment_circle_intersections(ray, Circle2::new(c, radius)) {
                    consider(t, pt, entity.id);
                }
            }
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => {
                let (Some(c), Some(s), Some(e)) = (pos(center), pos(start), pos(end)) else {
                    continue;
                };
                let arc = Arc2::new(
                    c,
                    c.distance(s),
                    (s.y - c.y).atan2(s.x - c.x),
                    (e.y - c.y).atan2(e.x - c.x),
                    sweep_ccw,
                );
                for (pt, t) in segment_arc_intersections(ray, arc) {
                    consider(t, pt, entity.id);
                }
            }
            EntityKind::Point { .. } => {}
        }
    }
    best.map(|(_, hit)| hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entity;
    use crate::plane::PlaneId;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn a_chamfer_cuts_both_lines_at_the_distance() {
        let (ta, tb) = chamfer_cut(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0), 2.0).unwrap();
        assert!(ta.distance(p(2.0, 0.0)) < 1e-9);
        assert!(tb.distance(p(0.0, 2.0)) < 1e-9);
    }

    #[test]
    fn a_chamfer_longer_than_a_line_or_on_colinear_lines_is_refused() {
        assert_eq!(
            chamfer_cut(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 10.0), 2.0),
            Err(ChamferRefusal::TooLong)
        );
        assert_eq!(
            chamfer_cut(p(0.0, 0.0), p(10.0, 0.0), p(-10.0, 0.0), 2.0),
            Err(ChamferRefusal::Colinear)
        );
    }

    /// A sketch of authored points and lines; returns it with the ids.
    fn sketch_with(
        points: &[(f64, f64)],
        lines: &[(usize, usize)],
    ) -> (SketchData, Vec<SketchEntityId>, Vec<SketchEntityId>) {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        let pids: Vec<SketchEntityId> = points
            .iter()
            .map(|&(x, y)| {
                let id = SketchEntityId::new();
                sketch
                    .entities
                    .push(Entity::new(id, plane, EntityKind::Point { x, y }));
                id
            })
            .collect();
        let lids = lines
            .iter()
            .map(|&(a, b)| {
                let id = SketchEntityId::new();
                sketch.entities.push(Entity::new(
                    id,
                    plane,
                    EntityKind::Line {
                        start: pids[a],
                        end: pids[b],
                    },
                ));
                id
            })
            .collect();
        (sketch, pids, lids)
    }

    fn authored(sketch: &SketchData) -> impl Fn(SketchEntityId) -> Option<Point2> + '_ {
        move |id| {
            sketch
                .entities
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| match e.kind {
                    EntityKind::Point { x, y } => Some(p(x, y)),
                    _ => None,
                })
        }
    }

    #[test]
    fn an_extended_line_stops_at_the_nearest_line_ahead() {
        // The line (0,0)-(2,0) points at walls x = 5 and x = 8.
        let (sketch, _, lids) = sketch_with(
            &[
                (0.0, 0.0),
                (2.0, 0.0),
                (5.0, -1.0),
                (5.0, 1.0),
                (8.0, -1.0),
                (8.0, 1.0),
            ],
            &[(0, 1), (2, 3), (4, 5)],
        );
        let hit = extension_hit(
            &sketch,
            lids[0],
            p(0.0, 0.0),
            p(2.0, 0.0),
            authored(&sketch),
        )
        .expect("the wall at x = 5");
        assert_eq!(hit.target, lids[1]);
        assert!(hit.at.distance(p(5.0, 0.0)) < 1e-9);
    }

    #[test]
    fn an_extended_line_reaches_nothing_behind_it_or_beside_it() {
        // The wall is behind the moving end and a second one is off the ray.
        let (sketch, _, lids) = sketch_with(
            &[
                (0.0, 0.0),
                (2.0, 0.0),
                (-3.0, -1.0),
                (-3.0, 1.0),
                (5.0, 2.0),
                (5.0, 4.0),
            ],
            &[(0, 1), (2, 3), (4, 5)],
        );
        assert_eq!(
            extension_hit(
                &sketch,
                lids[0],
                p(0.0, 0.0),
                p(2.0, 0.0),
                authored(&sketch)
            ),
            None
        );
    }
}
