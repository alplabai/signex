//! Add fillets on failing inside corners (mockup row 179d): a sharp corner
//! between two lines takes a tangent arc of the radius the router needs. A
//! corner that is an arc, whose lines are too short, or that something else
//! depends on is left as it was and counted. The arc is held tangent by a
//! construction radius to each tangent point, as the Slot holds its ends.
//! Rules: `docs/PCB_SKETCH.md`, "Outline checks".

use std::collections::HashMap;
use std::f64::consts::PI;

use signex_sketch::attr::{CustomPadShape, PadAttr, PadShape, PasteAperturePattern};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::PlaneId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use super::checks::{Finding, OutlineChecks};

/// Over the required radius, in mm: two file units (1 nm each), so a fillet
/// read back from disk still passes the check it fixes.
const RADIUS_MARGIN_MM: f64 = 2e-6;
/// Shortest line a fillet may leave, in mm.
const MIN_LINE_LEFT_MM: f64 = 1e-6;
/// An angle this close to flat or folded, in radians, has no corner to round.
const MIN_ANGLE_RAD: f64 = 1e-3;

type Xy = (f64, f64);

/// What [`add_fillets`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilletReport {
    pub applied: usize,
    pub refused: usize,
}

/// The sharp corners of `checks` a fillet can fix, each with the radius it
/// needs.
pub fn fillet_targets(sketch: &SketchData, checks: &OutlineChecks) -> Vec<(SketchEntityId, f64)> {
    let mut out: Vec<(SketchEntityId, f64)> = Vec::new();
    for (_, finding) in checks.failures() {
        if let Finding::TightCorner {
            entity,
            tool_radius_mm,
            ..
        } = finding
            && corner_arms(sketch, *entity).is_some()
            && !out.iter().any(|(id, _)| id == entity)
        {
            out.push((*entity, *tool_radius_mm));
        }
    }
    out
}

/// Round each of `targets` at the positions of `solve`. Corners sharing a
/// line are cut in turn, each from what the last one left.
pub fn add_fillets(
    sketch: &mut SketchData,
    solve: &FullSolveOutput,
    targets: &[(SketchEntityId, f64)],
) -> FilletReport {
    let mut at = solved_points(sketch, solve);
    let mut report = FilletReport::default();
    for &(corner, radius) in targets {
        let free_arms = corner_arms(sketch, corner).filter(|arms| !is_held(sketch, corner, arms));
        let fillet = free_arms.and_then(|arms| {
            let geometry = fillet_geometry(
                *at.get(&corner)?,
                *at.get(&arms[0].far)?,
                *at.get(&arms[1].far)?,
                radius + RADIUS_MARGIN_MM,
            )?;
            Some((arms, geometry))
        });
        match fillet {
            Some((arms, geometry)) => {
                splice(sketch, &mut at, corner, arms, geometry);
                report.applied += 1;
            }
            None => report.refused += 1,
        }
    }
    report
}

/// One line at a corner and its other end.
#[derive(Clone, Copy)]
struct Arm {
    line: SketchEntityId,
    far: SketchEntityId,
}

/// The two lines meeting at `corner`; `None` unless it is exactly two lines.
fn corner_arms(sketch: &SketchData, corner: SketchEntityId) -> Option<[Arm; 2]> {
    let mut arms = Vec::new();
    for entity in sketch.entities.iter().filter(|e| !e.bake_skipped()) {
        match entity.kind {
            EntityKind::Line { start, end } if start == corner => arms.push(Arm {
                line: entity.id,
                far: end,
            }),
            EntityKind::Line { start, end } if end == corner => arms.push(Arm {
                line: entity.id,
                far: start,
            }),
            EntityKind::Arc { start, end, .. } if start == corner || end == corner => return None,
            _ => {}
        }
    }
    <[Arm; 2]>::try_from(arms).ok()
}

/// Whether rounding `corner` would break something that depends on it. The
/// corner leaves the profile, so whatever names it (a pattern above all: the
/// bake finds a pattern through the profile's points) would stop following
/// the outline. A trim shortens both lines, so a constraint on their length
/// or middle would move the outline; one on their direction still holds.
fn is_held(sketch: &SketchData, corner: SketchEntityId, arms: &[Arm; 2]) -> bool {
    let is_arm = |id: SketchEntityId| arms.iter().any(|arm| arm.line == id);
    let on_other_entity = sketch.entities.iter().any(|e| {
        (e.id == corner && e.pad.is_some())
            || (e.id != corner && !is_arm(e.id) && e.point_refs().contains(&corner))
    });
    let in_a_pattern = sketch
        .arrays
        .iter()
        .any(|a| a.kind.entity_refs().contains(&corner));
    let constrained = sketch.constraints.iter().any(|c| {
        c.kind.touches(corner)
            || match c.kind {
                ConstraintKind::EqualLength { l1, l2 } => is_arm(l1) || is_arm(l2),
                ConstraintKind::Midpoint { line, .. } => is_arm(line),
                _ => false,
            }
    });
    on_other_entity || in_a_pattern || constrained
}

fn solved_points(sketch: &SketchData, solve: &FullSolveOutput) -> HashMap<SketchEntityId, Xy> {
    sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .filter_map(|e| {
            let xy = point_xy(e.id, &solve.result.state, &solve.result.index, sketch)?;
            Some((e.id, xy))
        })
        .collect()
}

/// A fillet's tangent points on the two lines, its centre and its sweep.
#[derive(Debug, Clone, Copy)]
struct Geometry {
    on_first: Xy,
    on_second: Xy,
    centre: Xy,
    radius: f64,
    sweep_ccw: bool,
}

/// The arc of `radius` tangent to the lines from `corner` to `a` and `b`;
/// `None` when they are in line or too short to hold it.
fn fillet_geometry(corner: Xy, a: Xy, b: Xy, radius: f64) -> Option<Geometry> {
    let (ax, ay) = (a.0 - corner.0, a.1 - corner.1);
    let (bx, by) = (b.0 - corner.0, b.1 - corner.1);
    let (a_len, b_len) = (ax.hypot(ay), bx.hypot(by));
    if a_len <= MIN_LINE_LEFT_MM || b_len <= MIN_LINE_LEFT_MM {
        return None;
    }
    let (ua, ub) = ((ax / a_len, ay / a_len), (bx / b_len, by / b_len));
    let theta = (ua.0 * ub.0 + ua.1 * ub.1).clamp(-1.0, 1.0).acos();
    if theta < MIN_ANGLE_RAD || PI - theta < MIN_ANGLE_RAD {
        return None;
    }
    let half = theta / 2.0;
    let trim = radius / half.tan();
    if trim >= a_len - MIN_LINE_LEFT_MM || trim >= b_len - MIN_LINE_LEFT_MM {
        return None;
    }
    let on_first = (corner.0 + ua.0 * trim, corner.1 + ua.1 * trim);
    let on_second = (corner.0 + ub.0 * trim, corner.1 + ub.1 * trim);
    let (mx, my) = (ua.0 + ub.0, ua.1 + ub.1);
    let m_len = mx.hypot(my);
    let reach = radius / half.sin();
    let centre = (corner.0 + mx / m_len * reach, corner.1 + my / m_len * reach);
    // The short way round from the first tangent point to the second.
    let (sx, sy) = (on_first.0 - centre.0, on_first.1 - centre.1);
    let (ex, ey) = (on_second.0 - centre.0, on_second.1 - centre.1);
    Some(Geometry {
        on_first,
        on_second,
        centre,
        radius,
        sweep_ccw: sx * ey - sy * ex > 0.0,
    })
}

/// Add the arc and its points, end the two lines on the arc and hold it
/// tangent at its radius. The arc takes the outline or cutout tag of the
/// line it continues.
fn splice(
    sketch: &mut SketchData,
    at: &mut HashMap<SketchEntityId, Xy>,
    corner: SketchEntityId,
    arms: [Arm; 2],
    geometry: Geometry,
) {
    let Some(line) = sketch.entities.iter().find(|e| e.id == arms[0].line) else {
        return;
    };
    let (plane, outline, cutout) = (line.plane, line.board_outline, line.board_cutout.clone());
    let mut point = |xy: Xy| {
        let id = SketchEntityId::new();
        at.insert(id, xy);
        sketch.entities.push(Entity::new(
            id,
            plane,
            EntityKind::Point { x: xy.0, y: xy.1 },
        ));
        id
    };
    let start = point(geometry.on_first);
    let end = point(geometry.on_second);
    let center = point(geometry.centre);
    let mut arc = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw: geometry.sweep_ccw,
        },
    );
    arc.board_outline = outline;
    arc.board_cutout = cutout;
    sketch.entities.push(arc);
    move_line_end(sketch, arms[0].line, corner, start);
    move_line_end(sketch, arms[1].line, corner, end);
    // Left in, no residual reads it: a frozen Point, always under-constrained.
    if is_orphan(sketch, corner) {
        sketch.entities.retain(|e| e.id != corner);
        at.remove(&corner);
    }
    let to_first = construction_radius(sketch, plane, center, start);
    let to_second = construction_radius(sketch, plane, center, end);
    // Three rows on the arc's five free coordinates leave the two the
    // corner Point had.
    let held = [
        ConstraintKind::Perpendicular {
            l1: to_first,
            l2: arms[0].line,
        },
        ConstraintKind::Perpendicular {
            l1: to_second,
            l2: arms[1].line,
        },
        ConstraintKind::DistancePtPt {
            p1: center,
            p2: start,
            target: DimTarget::Literal(geometry.radius),
        },
    ];
    sketch
        .constraints
        .extend(held.into_iter().map(|kind| Constraint {
            id: ConstraintId::new(),
            kind,
        }));
}

/// Whether `corner` is a bare Point nothing in the sketch names any more.
fn is_orphan(sketch: &SketchData, corner: SketchEntityId) -> bool {
    let is_bare = sketch
        .entities
        .iter()
        .find(|e| e.id == corner)
        .is_some_and(|e| *e == Entity::new(e.id, e.plane, e.kind.clone()));
    let on_entity = sketch.entities.iter().any(|e| {
        (e.id != corner && e.point_refs().contains(&corner))
            || e.pad.as_ref().is_some_and(|pad| pad_names(pad, corner))
    });
    let constrained = sketch.constraints.iter().any(|c| c.kind.touches(corner));
    let in_a_pattern = sketch
        .arrays
        .iter()
        .any(|a| a.kind.entity_refs().contains(&corner))
        || sketch.pattern_mirrors.iter().any(|m| m.line == corner);
    is_bare && !on_entity && !constrained && !in_a_pattern
}

/// Whether a pad's outline ledger, custom shape or paste apertures name `id`.
fn pad_names(pad: &PadAttr, id: SketchEntityId) -> bool {
    let in_shape = matches!(
        &pad.shape,
        PadShape::Custom(CustomPadShape::SketchProfile { source }) if source.contains(&id)
    );
    let in_paste = matches!(
        &pad.paste_apertures,
        PasteAperturePattern::Custom { source } if source.contains(&id)
    );
    pad.owned.contains(&id) || in_shape || in_paste
}

/// A construction line from the arc's centre to one tangent point.
fn construction_radius(
    sketch: &mut SketchData,
    plane: PlaneId,
    center: SketchEntityId,
    tangent_point: SketchEntityId,
) -> SketchEntityId {
    let id = SketchEntityId::new();
    let mut radius = Entity::new(
        id,
        plane,
        EntityKind::Line {
            start: center,
            end: tangent_point,
        },
    );
    radius.construction = true;
    sketch.entities.push(radius);
    id
}

fn move_line_end(
    sketch: &mut SketchData,
    line: SketchEntityId,
    from: SketchEntityId,
    to: SketchEntityId,
) {
    let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == line) else {
        return;
    };
    if let EntityKind::Line { start, end } = &mut entity.kind {
        if *start == from {
            *start = to;
        } else if *end == from {
            *end = to;
        }
    }
}

#[cfg(test)]
mod tests {
    use signex_sketch::Solver;
    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
    use signex_sketch::attr::BoardCutoutAttr;
    use signex_sketch::constraint::{Constraint, DimTarget};
    use signex_sketch::id::ConstraintId;
    use signex_sketch::parameter::resolve;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::solver::dof::DofColor;
    use signex_types::sketch::board_attr::ComponentAnchorAttr;

    use super::super::checks::{CheckState, outline_checks_with};
    use super::*;

    /// An L-shaped outline: one concave corner, at (5, 5).
    fn l_outline(notch: f64) -> SketchData {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let corners = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, notch),
            (notch, notch),
            (notch, 10.0),
            (0.0, 10.0),
        ];
        let points: Vec<SketchEntityId> = corners
            .iter()
            .map(|&(x, y)| {
                let id = SketchEntityId::new();
                sketch
                    .entities
                    .push(Entity::new(id, plane, EntityKind::Point { x, y }));
                id
            })
            .collect();
        for i in 0..points.len() {
            let kind = EntityKind::Line {
                start: points[i],
                end: points[(i + 1) % points.len()],
            };
            let mut line = Entity::new(SketchEntityId::new(), plane, kind);
            line.board_outline = true;
            sketch.entities.push(line);
        }
        sketch
    }

    fn solved(sketch: &SketchData) -> FullSolveOutput {
        let params = resolve(&sketch.parameters).unwrap();
        Solver::default().solve(sketch, &params).unwrap()
    }

    fn checks(sketch: &SketchData, solve: &FullSolveOutput) -> OutlineChecks {
        outline_checks_with(sketch, solve, &solve.params, Some(2.0))
    }

    #[test]
    fn a_sharp_inside_corner_takes_the_required_radius_and_passes() {
        let mut sketch = l_outline(5.0);
        let solve = solved(&sketch);
        let targets = fillet_targets(&sketch, &checks(&sketch, &solve));
        assert_eq!(targets.len(), 1, "{targets:?}");
        assert_eq!(targets[0].1, 1.0);

        let report = add_fillets(&mut sketch, &solve, &targets);

        assert_eq!(
            report,
            FilletReport {
                applied: 1,
                refused: 0
            }
        );
        let solve = solved(&sketch);
        let after = checks(&sketch, &solve);
        assert_eq!(after.closed, CheckState::Passed, "{after:?}");
        assert_eq!(after.inside_corners, CheckState::Passed, "{after:?}");
        let arc = sketch
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Arc { .. }))
            .expect("the fillet arc");
        assert!(arc.board_outline, "the arc is part of the outline");
    }

    #[test]
    fn a_corner_whose_lines_are_too_short_is_refused_and_left_alone() {
        // The notch leaves 0.5 mm lines: no room for a 1 mm radius.
        let mut sketch = l_outline(9.5);
        let solve = solved(&sketch);
        let before = sketch.clone();
        let targets = fillet_targets(&sketch, &checks(&sketch, &solve));
        assert_eq!(targets.len(), 1);

        let report = add_fillets(&mut sketch, &solve, &targets);

        assert_eq!(
            report,
            FilletReport {
                applied: 0,
                refused: 1
            }
        );
        assert_eq!(sketch, before);
    }

    #[test]
    fn the_fillet_arc_takes_the_short_way_round() {
        let g = fillet_geometry((0.0, 0.0), (5.0, 0.0), (0.0, 5.0), 1.0).unwrap();
        assert!(!g.sweep_ccw, "{g:?}");
        assert!((g.centre.0 - 1.0).abs() < 1e-9 && (g.centre.1 - 1.0).abs() < 1e-9);
    }

    /// The ids of an L outline: its six corner points, then its six lines.
    fn l_ids(sketch: &SketchData) -> (Vec<SketchEntityId>, Vec<SketchEntityId>) {
        let ids: Vec<SketchEntityId> = sketch.entities.iter().map(|e| e.id).collect();
        (ids[..6].to_vec(), ids[6..12].to_vec())
    }

    fn constrain(sketch: &mut SketchData, kind: ConstraintKind) {
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind,
        });
    }

    /// Fillet the L's one failing corner, at (5, 5).
    fn fillet_the_l(sketch: &mut SketchData) -> FilletReport {
        let solve = solved(sketch);
        let targets = fillet_targets(sketch, &checks(sketch, &solve));
        assert_eq!(targets.len(), 1, "{targets:?}");
        add_fillets(sketch, &solve, &targets)
    }

    const REFUSED: FilletReport = FilletReport {
        applied: 0,
        refused: 1,
    };

    #[test]
    fn a_corner_a_dimension_names_is_refused_and_left_alone() {
        let mut sketch = l_outline(5.0);
        let (points, _) = l_ids(&sketch);
        constrain(
            &mut sketch,
            ConstraintKind::DistancePtPt {
                p1: points[3],
                p2: points[2],
                target: DimTarget::Literal(5.0),
            },
        );
        let before = sketch.clone();

        assert_eq!(fillet_the_l(&mut sketch), REFUSED);

        assert_eq!(sketch, before);
    }

    #[test]
    fn a_line_whose_length_is_constrained_keeps_its_corner() {
        let mut sketch = l_outline(5.0);
        let (_, lines) = l_ids(&sketch);
        // Line 2 ends at the corner; line 1 is the 5 mm edge it must equal.
        constrain(
            &mut sketch,
            ConstraintKind::EqualLength {
                l1: lines[2],
                l2: lines[1],
            },
        );
        let before = sketch.clone();

        assert_eq!(fillet_the_l(&mut sketch), REFUSED);

        assert_eq!(sketch, before);
    }

    #[test]
    fn a_midpoint_on_a_trimmed_line_keeps_its_corner() {
        let mut sketch = l_outline(5.0);
        let (_, lines) = l_ids(&sketch);
        let plane = sketch.entities[0].plane;
        let middle = SketchEntityId::new();
        sketch.entities.push(Entity::new(
            middle,
            plane,
            EntityKind::Point { x: 7.5, y: 5.0 },
        ));
        constrain(
            &mut sketch,
            ConstraintKind::Midpoint {
                point: middle,
                line: lines[2],
            },
        );
        let before = sketch.clone();

        assert_eq!(fillet_the_l(&mut sketch), REFUSED);

        assert_eq!(sketch, before);
    }

    #[test]
    fn a_direction_constraint_on_a_line_does_not_stop_the_fillet() {
        let mut sketch = l_outline(5.0);
        let (_, lines) = l_ids(&sketch);
        constrain(&mut sketch, ConstraintKind::Horizontal { line: lines[2] });

        let report = fillet_the_l(&mut sketch);

        assert_eq!(
            report,
            FilletReport {
                applied: 1,
                refused: 0
            }
        );
        assert_eq!(
            sketch.constraints[0].kind,
            ConstraintKind::Horizontal { line: lines[2] },
            "the constraint stays"
        );
        assert_eq!(
            sketch.constraints.len(),
            4,
            "and the fillet adds its two perpendiculars and its radius"
        );
        let solve = solved(&sketch);
        let after = checks(&sketch, &solve);
        assert_eq!(after.inside_corners, CheckState::Passed, "{after:?}");
    }

    /// Rank of `rows` by Gram-Schmidt.
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

    fn free_coordinates(solve: &FullSolveOutput) -> usize {
        solve.result.state.len() - rank(&solve.jacobian)
    }

    /// The L with both arms of its corner held and the edge beside them
    /// dimensioned at 5 mm, so that dimension moves the corner.
    fn dimensioned_l() -> (SketchData, Vec<SketchEntityId>) {
        let mut sketch = l_outline(5.0);
        let (points, lines) = l_ids(&sketch);
        for kind in [
            ConstraintKind::Fixed { point: points[1] },
            ConstraintKind::Fixed { point: points[4] },
            ConstraintKind::Vertical { line: lines[1] },
            ConstraintKind::Horizontal { line: lines[2] },
            ConstraintKind::Vertical { line: lines[3] },
            ConstraintKind::DistancePtPt {
                p1: points[1],
                p2: points[2],
                target: DimTarget::Literal(5.0),
            },
        ] {
            constrain(&mut sketch, kind);
        }
        (sketch, points)
    }

    /// DECISIONS 2026-11-22, mockup row 179d: the arc stays tangent and
    /// keeps its radius through a later dimension edit.
    #[test]
    fn a_later_dimension_edit_keeps_the_fillet_tangent_at_its_radius() {
        let (mut sketch, points) = dimensioned_l();
        let free_before = free_coordinates(&solved(&sketch));
        assert_eq!(fillet_the_l(&mut sketch).applied, 1);

        let filleted = solved(&sketch);
        assert!(filleted.over_constraints.is_empty(), "no conflict");
        assert!(
            filleted.redundant_constraints.is_empty(),
            "nothing restated"
        );
        assert_eq!(free_coordinates(&filleted), free_before, "no freedom added");

        for constraint in &mut sketch.constraints {
            if let ConstraintKind::DistancePtPt { p2, target, .. } = &mut constraint.kind
                && *p2 == points[2]
            {
                *target = DimTarget::Literal(7.0);
            }
        }
        let edited = solved(&sketch);

        assert!(edited.over_constraints.is_empty(), "no conflict");
        let at = |id| {
            point_xy(id, &edited.result.state, &edited.result.index, &sketch).expect("a point")
        };
        let (center, start, end) = sketch
            .entities
            .iter()
            .find_map(|e| match e.kind {
                EntityKind::Arc {
                    center, start, end, ..
                } => Some((at(center), at(start), at(end))),
                _ => None,
            })
            .expect("the fillet arc");
        let (near, far) = (at(points[2]), at(points[4]));
        assert!((near.1 - 7.0).abs() < 1e-6, "the edge moved: {near:?}");
        assert!((start.1 - 7.0).abs() < 1e-6, "the arc followed: {start:?}");
        let radius = 1.0 + RADIUS_MARGIN_MM;
        for point in [start, end] {
            let reach = (point.0 - center.0).hypot(point.1 - center.1);
            assert!((reach - radius).abs() < 1e-6, "radius kept: {reach}");
        }
        let radius_dot_line =
            |p: Xy, q: Xy| (p.0 - center.0) * (p.0 - q.0) + (p.1 - center.1) * (p.1 - q.1);
        assert!(
            radius_dot_line(start, near).abs() < 1e-6,
            "tangent to the first line"
        );
        assert!(
            radius_dot_line(end, far).abs() < 1e-6,
            "tangent to the second line"
        );
        let after = checks(&sketch, &edited);
        assert_eq!(after.inside_corners, CheckState::Passed, "{after:?}");
    }

    /// The L with every point pinned: five Fixed, and the corner by the
    /// directions of its two lines, which a fillet may keep.
    fn fully_constrained_l() -> (SketchData, Vec<SketchEntityId>) {
        let mut sketch = l_outline(5.0);
        let (points, lines) = l_ids(&sketch);
        for index in [0, 1, 2, 4, 5] {
            constrain(
                &mut sketch,
                ConstraintKind::Fixed {
                    point: points[index],
                },
            );
        }
        constrain(&mut sketch, ConstraintKind::Horizontal { line: lines[2] });
        constrain(&mut sketch, ConstraintKind::Vertical { line: lines[3] });
        (sketch, points)
    }

    fn all_full(solve: &FullSolveOutput) -> bool {
        solve.colours.values().all(|c| *c == DofColor::Full)
    }

    /// The old corner stayed as a frozen Point, always Under, so the DOF
    /// chip read under-constrained.
    #[test]
    fn a_fully_constrained_sketch_stays_fully_constrained_after_a_fillet() {
        let (mut sketch, points) = fully_constrained_l();
        assert!(all_full(&solved(&sketch)), "the fixture is fully pinned");

        assert_eq!(fillet_the_l(&mut sketch).applied, 1);

        assert!(
            !sketch.entities.iter().any(|e| e.id == points[3]),
            "the old corner is gone"
        );
        assert!(sketch.dangling_refs().is_empty());
        let after = solved(&sketch);
        assert!(after.over_constraints.is_empty(), "no conflict");
        assert!(all_full(&after), "{:?}", after.colours);
    }

    #[test]
    fn a_corner_carrying_a_component_anchor_stays_after_the_fillet() {
        let mut sketch = l_outline(5.0);
        let (points, _) = l_ids(&sketch);
        sketch.entities[3].component_anchor = Some(ComponentAnchorAttr::default());

        assert_eq!(fillet_the_l(&mut sketch).applied, 1);

        assert!(sketch.entities.iter().any(|e| e.id == points[3]));
    }

    #[test]
    fn a_point_a_pad_outline_owns_is_not_an_orphan() {
        let plane = PlaneId::new();
        let corner = SketchEntityId::new();
        let mut pad = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Point { x: 1.0, y: 1.0 },
        );
        pad.pad = Some(PadAttr {
            owned: vec![corner],
            ..PadAttr::default()
        });
        let mut sketch = SketchData::default();
        sketch.entities.push(Entity::new(
            corner,
            plane,
            EntityKind::Point { x: 0.0, y: 0.0 },
        ));
        sketch.entities.push(pad);

        assert!(!is_orphan(&sketch, corner));

        sketch.entities[1].pad = None;
        assert!(is_orphan(&sketch, corner));
    }

    /// The review's case: a pattern of a corner Point repeats the cutout, so
    /// the fillet must leave that Point in the profile. The cutout is 6 x 4
    /// so that every corner has room for R 1.
    #[test]
    fn a_corner_a_pattern_repeats_is_kept_and_every_instance_still_bakes() {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let corners: Vec<SketchEntityId> = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)]
            .iter()
            .map(|&(x, y)| {
                let id = SketchEntityId::new();
                sketch
                    .entities
                    .push(Entity::new(id, plane, EntityKind::Point { x, y }));
                id
            })
            .collect();
        for i in 0..4 {
            let kind = EntityKind::Line {
                start: corners[i],
                end: corners[(i + 1) % 4],
            };
            let mut line = Entity::new(SketchEntityId::new(), plane, kind);
            line.board_cutout = Some(BoardCutoutAttr {
                edge_radius_expr: Some("0.5mm".into()),
                through: true,
            });
            sketch.entities.push(line);
        }
        sketch.arrays.push(Array {
            id: ArrayId::new(),
            kind: ArrayKind::Linear {
                source: corners[0],
                count_expr: "2".into(),
                dx_expr: "10mm".into(),
                dy_expr: "0mm".into(),
            },
            numbering: NumberingScheme::default(),
        });
        let solve = solved(&sketch);
        let targets: Vec<(SketchEntityId, f64)> = corners.iter().map(|&c| (c, 1.0)).collect();

        let report = add_fillets(&mut sketch, &solve, &targets);

        assert_eq!(
            report,
            FilletReport {
                applied: 3,
                refused: 1
            }
        );
        let cutouts = crate::board::bake_board(&sketch, &[]).unwrap().cutouts;
        let mut lowest: Vec<i64> = cutouts
            .iter()
            .map(|c| {
                let x = c.boundary.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
                (x * 1e6).round() as i64
            })
            .collect();
        lowest.sort_unstable();
        assert_eq!(lowest, [0, 10_000_000], "both instances bake");
    }
}
