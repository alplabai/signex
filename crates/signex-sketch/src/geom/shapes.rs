//! The whole shapes the sketch Create tools place in one go (mockup row 187): a slot
//! and a regular polygon. Pure builders return the entities and constraints
//! to add, so the tool adds them as one edit, and the outline helpers give
//! the canvas ghost the same geometry. Why the slot holds its tangency with
//! end diameters: docs/FOOTPRINT_SKETCH.md, "Create tools".

use std::f64::consts::TAU;

use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::entity::{Entity, EntityKind};
use crate::id::{ConstraintId, SketchEntityId};
use crate::plane::PlaneId;

/// Fewest sides a regular polygon takes.
pub const POLYGON_SIDES_MIN: u32 = 3;
/// Most sides a regular polygon takes.
pub const POLYGON_SIDES_MAX: u32 = 64;
/// Sides of a polygon when no count is typed.
pub const POLYGON_SIDES_DEFAULT: u32 = 6;
/// Shortest slot centre distance, slot half width or polygon radius, mm.
pub const MIN_SHAPE_MM: f64 = 1e-6;
/// Segments each slot end is drawn with in [`slot_outline`].
pub const SLOT_END_SEGMENTS: usize = 16;

/// A Point already in the sketch: its id and where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedPoint {
    pub id: SketchEntityId,
    pub at: (f64, f64),
}

/// The entities and constraints of one shape, in the order to add them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShapeParts {
    pub entities: Vec<Entity>,
    pub constraints: Vec<Constraint>,
    /// The Point at the click that closed the shape, when the shape has
    /// one (a polygon's first vertex); a slot's last click places none.
    pub clicked_point: Option<SketchEntityId>,
}

/// Why a shape cannot be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeRefusal {
    /// The two slot centres, or a polygon's centre and first vertex, are
    /// one point.
    Degenerate,
    /// The slot has no width.
    NoWidth,
    /// The side count is outside `POLYGON_SIDES_MIN..=POLYGON_SIDES_MAX`.
    SideCount,
}

/// The side count typed for a polygon: empty is the default, else a whole
/// number in `POLYGON_SIDES_MIN..=POLYGON_SIDES_MAX`; anything else is
/// `None`.
pub fn parse_side_count(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return Some(POLYGON_SIDES_DEFAULT);
    }
    text.parse::<u32>()
        .ok()
        .filter(|n| (POLYGON_SIDES_MIN..=POLYGON_SIDES_MAX).contains(n))
}

/// The slot half width the cursor at `p` sets: its distance from the line
/// through the centres `a` and `b`.
pub fn slot_half_width(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    if len < MIN_SHAPE_MM {
        return 0.0;
    }
    ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / len
}

/// The four join points of a slot: `[a_top, a_bot, b_top, b_bot]`, "top"
/// on the left of the direction from `a` to `b`.
fn slot_joins(a: (f64, f64), b: (f64, f64), half: f64) -> Option<[(f64, f64); 4]> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    let usable = len >= MIN_SHAPE_MM && half.is_finite() && half >= MIN_SHAPE_MM;
    if !usable {
        return None;
    }
    let n = (-dy / len * half, dx / len * half);
    Some([
        (a.0 + n.0, a.1 + n.1),
        (a.0 - n.0, a.1 - n.1),
        (b.0 + n.0, b.1 + n.1),
        (b.0 - n.0, b.1 - n.1),
    ])
}

/// The slot's outline as a closed polyline (first point not repeated),
/// each end drawn with [`SLOT_END_SEGMENTS`] segments. Empty when the slot
/// is degenerate.
pub fn slot_outline(a: (f64, f64), b: (f64, f64), half: f64) -> Vec<(f64, f64)> {
    let Some([a_top, _, _, b_bot]) = slot_joins(a, b, half) else {
        return Vec::new();
    };
    let half_turn = |centre: (f64, f64), from: (f64, f64)| {
        let start = (from.1 - centre.1).atan2(from.0 - centre.0);
        (0..=SLOT_END_SEGMENTS).map(move |i| {
            let t = start + TAU / 2.0 * i as f64 / SLOT_END_SEGMENTS as f64;
            (centre.0 + half * t.cos(), centre.1 + half * t.sin())
        })
    };
    half_turn(a, a_top).chain(half_turn(b, b_bot)).collect()
}

/// A slot: two semicircular Arcs on the centres `a` and `b`, two Lines
/// joining their ends, and a construction diameter across each end. The
/// centre distance and the width are dimensions; the slot is fully defined
/// but for where it sits and which way it points.
pub fn slot(
    plane: PlaneId,
    a: PlacedPoint,
    b: PlacedPoint,
    half_width: f64,
) -> Result<ShapeParts, ShapeRefusal> {
    if (b.at.0 - a.at.0).hypot(b.at.1 - a.at.1) < MIN_SHAPE_MM {
        return Err(ShapeRefusal::Degenerate);
    }
    let joins = slot_joins(a.at, b.at, half_width).ok_or(ShapeRefusal::NoWidth)?;
    let mut parts = ShapeParts::default();
    let [a_top, a_bot, b_top, b_bot] =
        joins.map(|(x, y)| parts.add(plane, EntityKind::Point { x, y }));
    let arc = |center, start, end| EntityKind::Arc {
        center,
        start,
        end,
        sweep_ccw: true,
    };
    let arc_a = parts.add(plane, arc(a.id, a_top, a_bot));
    let arc_b = parts.add(plane, arc(b.id, b_bot, b_top));
    parts.add(
        plane,
        EntityKind::Line {
            start: a_bot,
            end: b_bot,
        },
    );
    let top = parts.add(
        plane,
        EntityKind::Line {
            start: b_top,
            end: a_top,
        },
    );
    let chord_a = parts.add_construction(plane, a_top, a_bot);
    let chord_b = parts.add_construction(plane, b_bot, b_top);
    let centre_distance = (b.at.0 - a.at.0).hypot(b.at.1 - a.at.1);
    parts.constrain([
        ConstraintKind::Midpoint {
            point: a.id,
            line: chord_a,
        },
        ConstraintKind::Midpoint {
            point: b.id,
            line: chord_b,
        },
        ConstraintKind::Perpendicular {
            l1: chord_a,
            l2: top,
        },
        ConstraintKind::Perpendicular {
            l1: chord_b,
            l2: top,
        },
        ConstraintKind::EqualRadius {
            e1: arc_a,
            e2: arc_b,
        },
        ConstraintKind::DistancePtPt {
            p1: a.id,
            p2: b.id,
            target: DimTarget::Literal(centre_distance),
        },
        ConstraintKind::DistancePtPt {
            p1: a_top,
            p2: a_bot,
            target: DimTarget::Literal(2.0 * half_width),
        },
    ]);
    Ok(parts)
}

/// The vertices of a regular polygon on `centre` with its first vertex at
/// `first`, counter-clockwise. Empty when `centre` and `first` are one
/// point or `sides` is out of range.
pub fn polygon_vertices(centre: (f64, f64), first: (f64, f64), sides: u32) -> Vec<(f64, f64)> {
    let radius = (first.0 - centre.0).hypot(first.1 - centre.1);
    if radius < MIN_SHAPE_MM || !(POLYGON_SIDES_MIN..=POLYGON_SIDES_MAX).contains(&sides) {
        return Vec::new();
    }
    let start = (first.1 - centre.1).atan2(first.0 - centre.0);
    (0..sides)
        .map(|k| {
            let t = start + TAU * f64::from(k) / f64::from(sides);
            (centre.0 + radius * t.cos(), centre.1 + radius * t.sin())
        })
        .collect()
}

/// A regular polygon: `sides` Lines closing a loop, their vertices held on
/// a construction Circle on `centre`, each side as long as the next.
pub fn regular_polygon(
    plane: PlaneId,
    centre: PlacedPoint,
    first: (f64, f64),
    sides: u32,
) -> Result<ShapeParts, ShapeRefusal> {
    if !(POLYGON_SIDES_MIN..=POLYGON_SIDES_MAX).contains(&sides) {
        return Err(ShapeRefusal::SideCount);
    }
    let corners = polygon_vertices(centre.at, first, sides);
    if corners.is_empty() {
        return Err(ShapeRefusal::Degenerate);
    }
    let radius = (first.0 - centre.at.0).hypot(first.1 - centre.at.1);
    let mut parts = ShapeParts::default();
    let vertices: Vec<SketchEntityId> = corners
        .into_iter()
        .map(|(x, y)| parts.add(plane, EntityKind::Point { x, y }))
        .collect();
    parts.clicked_point = vertices.first().copied();
    let edges: Vec<SketchEntityId> = (0..vertices.len())
        .map(|k| {
            let end = vertices[(k + 1) % vertices.len()];
            parts.add(
                plane,
                EntityKind::Line {
                    start: vertices[k],
                    end,
                },
            )
        })
        .collect();
    let mut circle = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Circle {
            center: centre.id,
            radius,
        },
    );
    circle.construction = true;
    let circle_id = circle.id;
    parts.entities.push(circle);
    parts.constrain(
        vertices
            .iter()
            .map(|&point| ConstraintKind::DistancePtCircle {
                point,
                circle: circle_id,
                target: DimTarget::Literal(0.0),
            }),
    );
    parts.constrain(edges.windows(2).map(|pair| ConstraintKind::EqualLength {
        l1: pair[0],
        l2: pair[1],
    }));
    Ok(parts)
}

impl ShapeParts {
    fn add(&mut self, plane: PlaneId, kind: EntityKind) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.entities.push(Entity::new(id, plane, kind));
        id
    }

    fn add_construction(
        &mut self,
        plane: PlaneId,
        start: SketchEntityId,
        end: SketchEntityId,
    ) -> SketchEntityId {
        let id = self.add(plane, EntityKind::Line { start, end });
        if let Some(line) = self.entities.last_mut() {
            line.construction = true;
        }
        id
    }

    fn constrain(&mut self, kinds: impl IntoIterator<Item = ConstraintKind>) {
        self.constraints
            .extend(kinds.into_iter().map(|kind| Constraint {
                id: ConstraintId::new(),
                kind,
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SketchData;
    use crate::plane::{Plane, PlaneKind};
    use crate::solver::FullSolveOutput;
    use crate::solver::residual::ResolvedParams;

    /// A sketch with one plane and a Point at each of `at`.
    fn sketch_with(at: &[(f64, f64)]) -> (SketchData, PlaneId, Vec<PlacedPoint>) {
        let plane = PlaneId::new();
        let mut sketch = SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            ..SketchData::default()
        };
        let points = at
            .iter()
            .map(|&(x, y)| {
                let id = SketchEntityId::new();
                sketch
                    .entities
                    .push(Entity::new(id, plane, EntityKind::Point { x, y }));
                PlacedPoint { id, at: (x, y) }
            })
            .collect();
        (sketch, plane, points)
    }

    fn add(sketch: &mut SketchData, parts: ShapeParts) {
        sketch.entities.extend(parts.entities);
        sketch.constraints.extend(parts.constraints);
    }

    fn solve(sketch: &SketchData) -> FullSolveOutput {
        crate::Solver::default()
            .solve(sketch, &ResolvedParams::new())
            .expect("the shape solves")
    }

    /// Rank of `rows` by Gram-Schmidt, each row kept when what is left of
    /// it after projection is over 1e-8 of its length.
    fn rank(rows: &[Vec<f64>]) -> usize {
        let mut basis: Vec<Vec<f64>> = Vec::new();
        for row in rows {
            let length = row.iter().map(|v| v * v).sum::<f64>().sqrt();
            if length < 1e-12 {
                continue;
            }
            let mut rest = row.clone();
            for b in &basis {
                let dot: f64 = rest.iter().zip(b).map(|(r, q)| r * q).sum();
                rest.iter_mut().zip(b).for_each(|(r, q)| *r -= dot * q);
            }
            let left = rest.iter().map(|v| v * v).sum::<f64>().sqrt();
            if left > 1e-8 * length {
                basis.push(rest.iter().map(|v| v / left).collect());
            }
        }
        basis.len()
    }

    fn count(sketch: &SketchData, keep: impl Fn(&Entity) -> bool) -> usize {
        sketch.entities.iter().filter(|e| keep(e)).count()
    }

    fn line_ends(sketch: &SketchData, line: &Entity) -> ((f64, f64), (f64, f64)) {
        let at = |id| {
            sketch
                .entities
                .iter()
                .find_map(|e| match e.kind {
                    EntityKind::Point { x, y } if e.id == id => Some((x, y)),
                    _ => None,
                })
                .expect("a point")
        };
        match line.kind {
            EntityKind::Line { start, end } => (at(start), at(end)),
            _ => panic!("not a line"),
        }
    }

    #[test]
    fn a_slot_solves_with_no_conflict_and_only_its_placement_free() {
        let (mut sketch, plane, centres) = sketch_with(&[(1.0, 2.0), (7.0, 5.0)]);
        let parts = slot(plane, centres[0], centres[1], 1.5).expect("a slot");
        add(&mut sketch, parts);

        let arcs = count(&sketch, |e| matches!(e.kind, EntityKind::Arc { .. }));
        let outline = count(&sketch, |e| {
            matches!(e.kind, EntityKind::Line { .. }) && !e.construction
        });
        let diameters = count(&sketch, |e| {
            matches!(e.kind, EntityKind::Line { .. }) && e.construction
        });
        let dimensions = sketch
            .constraints
            .iter()
            .filter(|c| c.kind.dim_target().is_some())
            .count();
        assert_eq!((arcs, outline, diameters, dimensions), (2, 2, 2, 2));

        let out = solve(&sketch);
        assert!(out.over_constraints.is_empty(), "no conflict");
        assert!(out.redundant_constraints.is_empty(), "nothing restated");
        let free = out.result.state.len() - rank(&out.jacobian);
        assert_eq!(free, 3, "only where the slot sits and which way it points");
    }

    #[test]
    fn a_slot_joins_its_ends_tangentially_at_the_typed_width() {
        let (mut sketch, plane, centres) = sketch_with(&[(0.0, 0.0), (10.0, 0.0)]);
        add(
            &mut sketch,
            slot(plane, centres[0], centres[1], 2.0).expect("a slot"),
        );
        for line in sketch
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Line { .. }) && !e.construction)
        {
            let (s, e) = line_ends(&sketch, line);
            assert!((s.1 - e.1).abs() < 1e-12, "a side runs along the centres");
            assert!(
                (s.1.abs() - 2.0).abs() < 1e-12,
                "a side is the half width out"
            );
            assert!(
                (s.0 - e.0).abs() > 9.999,
                "a side spans the centre distance"
            );
        }
    }

    #[test]
    fn a_slot_needs_two_centres_and_a_width() {
        let (_, plane, centres) = sketch_with(&[(0.0, 0.0), (0.0, 0.0), (4.0, 0.0)]);
        assert_eq!(
            slot(plane, centres[0], centres[1], 1.0),
            Err(ShapeRefusal::Degenerate)
        );
        assert_eq!(
            slot(plane, centres[0], centres[2], 0.0),
            Err(ShapeRefusal::NoWidth)
        );
        assert_eq!(slot_half_width((0.0, 0.0), (4.0, 0.0), (1.0, -1.5)), 1.5);
        assert_eq!(slot_outline((0.0, 0.0), (4.0, 0.0), 0.0), Vec::new());
        assert_eq!(
            slot_outline((0.0, 0.0), (4.0, 0.0), 1.0).len(),
            2 * (SLOT_END_SEGMENTS + 1)
        );
    }

    #[test]
    fn a_polygon_has_equal_closed_sides_on_its_circle() {
        let (mut sketch, plane, centre) = sketch_with(&[(2.0, -1.0)]);
        let parts = regular_polygon(plane, centre[0], (6.0, -1.0), 5).expect("a pentagon");
        let first = parts.clicked_point.expect("the first vertex");
        add(&mut sketch, parts);

        let sides: Vec<&Entity> = sketch
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
            .collect();
        assert_eq!(sides.len(), 5);
        let lengths: Vec<f64> = sides
            .iter()
            .map(|line| {
                let (s, e) = line_ends(&sketch, line);
                (e.0 - s.0).hypot(e.1 - s.1)
            })
            .collect();
        assert!(lengths.iter().all(|l| (l - lengths[0]).abs() < 1e-9));
        let closes = matches!(
            sides.last().map(|line| &line.kind),
            Some(EntityKind::Line { end, .. }) if *end == first
        );
        assert!(closes, "the last side ends on the first vertex");
        assert_eq!(
            count(&sketch, |e| matches!(e.kind, EntityKind::Circle { .. })
                && e.construction),
            1
        );

        let out = solve(&sketch);
        assert!(out.over_constraints.is_empty(), "no conflict");
        assert!(out.redundant_constraints.is_empty(), "nothing restated");
        let free = out.result.state.len() - rank(&out.jacobian);
        assert_eq!(free, 4, "placement, rotation and size");
    }

    #[test]
    fn the_side_count_is_three_to_sixty_four_and_empty_is_six() {
        assert_eq!(parse_side_count(""), Some(6));
        assert_eq!(parse_side_count(" 5 "), Some(5));
        assert_eq!(parse_side_count("3"), Some(3));
        assert_eq!(parse_side_count("64"), Some(64));
        for refused in ["2", "65", "0", "-4", "5.5", "five"] {
            assert_eq!(parse_side_count(refused), None, "{refused}");
        }
        let (_, plane, centre) = sketch_with(&[(0.0, 0.0)]);
        assert_eq!(
            regular_polygon(plane, centre[0], (1.0, 0.0), 2),
            Err(ShapeRefusal::SideCount)
        );
        assert_eq!(
            regular_polygon(plane, centre[0], (0.0, 0.0), 6),
            Err(ShapeRefusal::Degenerate)
        );
        assert_eq!(polygon_vertices((0.0, 0.0), (1.0, 0.0), 64).len(), 64);
    }
}
