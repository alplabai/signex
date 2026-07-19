//! `split_line` — divide a sketch `Line` at a parameter into two Lines
//! sharing a new mid `Point`.
//!
//! Pure model primitive (issue #360) — no `signex-app` / UI dependency.
//! The footprint editor's Break Track action (issue #372) is the
//! consumer: it hit-tests a click against a Line, projects it to a
//! parameter `t`, and calls [`split_line`] directly.

use crate::constraint::{Constraint, ConstraintKind};
use crate::entity::{Entity, EntityKind};
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;

/// Parametric epsilon for `t`. A split within this distance of an
/// endpoint would mint a Point coincident with an existing endpoint
/// and leave one half zero-length — never useful, and it would poison
/// the zero-length guard on a later split of that degenerate half. So
/// it is treated as a no-op, not as "split at the endpoint".
const T_EPS: f64 = 1e-9;

/// Result of a successful [`split_line`] — the new mid `Point` and the
/// two replacement `Line`s (`start -> mid`, `mid -> end`), so a caller
/// can select or further constrain them without re-querying the sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitResult {
    pub mid_point: SketchEntityId,
    /// `start -> mid_point`.
    pub line_a: SketchEntityId,
    /// `mid_point -> end`.
    pub line_b: SketchEntityId,
}

/// Split the `Line` entity `line` at parameter `t` (`0.0` = start,
/// `1.0` = end) into two Lines meeting at a new mid `Point`.
///
/// Returns `None` — leaving `sketch` byte-for-byte unchanged — when:
/// - `line` does not resolve to an entity, or resolves to a non-`Line`
///   entity;
/// - either endpoint `Point` cannot be resolved;
/// - the segment is degenerate (zero length);
/// - `t` is not finite, lies outside `[0.0, 1.0]`, or is within
///   [`T_EPS`] of `0.0`/`1.0` — a split at (or past) an existing
///   endpoint is a no-op, not two lines of which one is zero-length.
///
/// All validation runs against read-only lookups before any mutation,
/// so a `None` return never leaves a half-applied split behind.
///
/// On success the original `Line` is removed and replaced by two new
/// `Line`s that inherit its `construction` / `centerline` flags and
/// every bake attribute (silk, courtyard, mask, paste, pour, keepout,
/// board cutout, v-score) — so splitting e.g. a silk line yields two
/// silk lines rather than dropping the halves out of the bake.
/// Constraints that named the retired line id are rewritten or
/// duplicated per kind; see [`split_duplicated`] and
/// [`retarget_relational`] for the carry-over rule and the reason for
/// each.
pub fn split_line(sketch: &mut SketchData, line: SketchEntityId, t: f64) -> Option<SplitResult> {
    let line_idx = sketch.entities.iter().position(|e| e.id == line)?;
    let (start_id, end_id) = match sketch.entities[line_idx].kind {
        EntityKind::Line { start, end } => (start, end),
        _ => return None,
    };
    if !(t > T_EPS && t < 1.0 - T_EPS) {
        return None; // NaN, infinite, out-of-range, or endpoint-adjacent
    }

    let start_xy = entity_point_xy(sketch, start_id)?;
    let end_xy = entity_point_xy(sketch, end_id)?;
    let dx = end_xy.0 - start_xy.0;
    let dy = end_xy.1 - start_xy.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        return None; // degenerate zero-length line
    }
    let mid_xy = (start_xy.0 + t * dx, start_xy.1 + t * dy);

    let mid_id = SketchEntityId::new();
    let line_a_id = SketchEntityId::new();
    let line_b_id = SketchEntityId::new();

    let template = sketch.entities[line_idx].clone();
    let mid_point = Entity::new(
        mid_id,
        template.plane,
        EntityKind::Point {
            x: mid_xy.0,
            y: mid_xy.1,
        },
    );
    let mut line_a = template.clone();
    line_a.id = line_a_id;
    line_a.kind = EntityKind::Line {
        start: start_id,
        end: mid_id,
    };
    let mut line_b = template;
    line_b.id = line_b_id;
    line_b.kind = EntityKind::Line {
        start: mid_id,
        end: end_id,
    };

    let ctx = SplitCtx {
        line,
        line_a: line_a_id,
        line_b: line_b_id,
        start_xy,
        dx,
        dy,
        len_sq,
        t,
    };
    let new_constraints: Vec<Constraint> = sketch
        .constraints
        .iter()
        .flat_map(|c| split_constraint(c, sketch, &ctx))
        .collect();

    sketch.entities.remove(line_idx);
    sketch.entities.push(mid_point);
    sketch.entities.push(line_a);
    sketch.entities.push(line_b);
    sketch.constraints = new_constraints;

    Some(SplitResult {
        mid_point: mid_id,
        line_a: line_a_id,
        line_b: line_b_id,
    })
}

/// Coordinates of a `Point` entity, or `None` if `id` doesn't resolve
/// or resolves to a non-`Point` entity.
fn entity_point_xy(sketch: &SketchData, id: SketchEntityId) -> Option<(f64, f64)> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
}

/// Bundled split parameters threaded through constraint carry-over so
/// the per-kind helpers don't each take eight arguments.
struct SplitCtx {
    line: SketchEntityId,
    line_a: SketchEntityId,
    line_b: SketchEntityId,
    start_xy: (f64, f64),
    dx: f64,
    dy: f64,
    len_sq: f64,
    t: f64,
}

/// Rewrite one constraint against a completed split. Returns 0, 1, or
/// 2 replacement constraints. A constraint whose fields never named
/// `line` (including every kind that can only reference a Point / Arc
/// / Circle — `Coincident`, `DistancePtPt`, `Fixed`, `PointOnArc`,
/// `DistancePtCircle`, `EqualRadius`, `TangentArcArc`,
/// `SymmetricAboutPoint`) is returned unchanged: none of their fields
/// can dangle from this split, since the original Line's endpoint
/// Points keep their ids and coordinates untouched — only the Line
/// entity itself is retired.
fn split_constraint(c: &Constraint, sketch: &SketchData, ctx: &SplitCtx) -> Vec<Constraint> {
    if let Some(v) = split_duplicated(c, ctx) {
        return v;
    }
    if let Some(v) = split_point_on_line(c, sketch, ctx) {
        return v;
    }
    vec![retarget_relational(c, ctx)]
}

/// `Horizontal` / `Vertical` — duplicated onto both halves (the
/// original id keeps `line_a`, a freshly minted id covers `line_b`).
///
/// Both are absolute-frame, single-line predicates: pinning one
/// endpoint's y (or x) equal to the other's. The new mid Point has 2
/// fresh DOF the original 2-point Line never had, and duplicating the
/// constraint spends exactly one residual per half pinning those 2
/// DOF, with no dependency on any other movable entity — it cannot
/// introduce a new infeasibility beyond what the original constraint
/// already implied (`start`'s and `end`'s shared axis coordinate).
/// This is what keeps a split silk outline from collapsing on the
/// next solve.
fn split_duplicated(c: &Constraint, ctx: &SplitCtx) -> Option<Vec<Constraint>> {
    use ConstraintKind::*;
    Some(match &c.kind {
        Horizontal { line } if *line == ctx.line => duplicate(
            c.id,
            Horizontal { line: ctx.line_a },
            Horizontal { line: ctx.line_b },
        ),
        Vertical { line } if *line == ctx.line => duplicate(
            c.id,
            Vertical { line: ctx.line_a },
            Vertical { line: ctx.line_b },
        ),
        _ => return None,
    })
}

fn duplicate(id: ConstraintId, a: ConstraintKind, b: ConstraintKind) -> Vec<Constraint> {
    vec![
        Constraint { id, kind: a },
        Constraint {
            id: ConstraintId::new(),
            kind: b,
        },
    ]
}

/// `PointOnLine` — re-points to whichever half the constrained point
/// actually falls on, compared against the split parameter `t`. This
/// is the one kind where duplicating OR always picking a fixed half
/// would be wrong: the point can only physically sit on one of the
/// two new segments, so the constraint must follow it there or it
/// either dangles on nothing or is silently satisfied by luck.
fn split_point_on_line(
    c: &Constraint,
    sketch: &SketchData,
    ctx: &SplitCtx,
) -> Option<Vec<Constraint>> {
    let ConstraintKind::PointOnLine { point, line } = &c.kind else {
        return None;
    };
    if *line != ctx.line {
        return None;
    }
    let half = if point_param(sketch, *point, ctx) <= ctx.t {
        ctx.line_a
    } else {
        ctx.line_b
    };
    Some(vec![Constraint {
        id: c.id,
        kind: ConstraintKind::PointOnLine {
            point: *point,
            line: half,
        },
    }])
}

/// Raw (unclamped) parametric position of `point`'s coordinates
/// projected onto the original line. An unresolvable `point` id (a
/// pre-existing malformed sketch — the id doesn't resolve to a Point)
/// falls back to `0.0`, landing the constraint on `line_a`; that's a
/// safe default, not a correctness claim about a sketch that was
/// already broken before this split ran.
fn point_param(sketch: &SketchData, point: SketchEntityId, ctx: &SplitCtx) -> f64 {
    let Some((px, py)) = entity_point_xy(sketch, point) else {
        return 0.0;
    };
    ((px - ctx.start_xy.0) * ctx.dx + (py - ctx.start_xy.1) * ctx.dy) / ctx.len_sq
}

/// `Parallel` / `Perpendicular` / `Angle` / `EqualLength` /
/// `TangentLineArc` / `SymmetricAboutLine` / `Midpoint` /
/// `DistancePtLine` — re-pointed onto `line_a` only; everything else
/// passes through unchanged.
///
/// Each of these relates the split line to a SECOND, independent
/// entity (another line, an arc, or a scalar target). Duplicating
/// would assert the identical numeric relationship for two different
/// physical segments against one external reference — generally
/// infeasible the instant the mid point or the partner entity moves
/// independently after the split (two segments can't both equal
/// `l2`'s length, or both sit at `target` degrees from `l2`, unless
/// the split happened to land exactly at the midpoint by coincidence).
/// Re-pointing to one half preserves exactly what the original
/// constraint asserted about the one physical line it still
/// describes, and leaves the other half's extra DOF free — which is
/// the expected price of turning one rigid line into a two-segment
/// hinge at the mid point.
fn retarget_relational(c: &Constraint, ctx: &SplitCtx) -> Constraint {
    use ConstraintKind::*;
    let sub = |id: SketchEntityId| if id == ctx.line { ctx.line_a } else { id };
    let kind = match &c.kind {
        Parallel { l1, l2 } => Parallel {
            l1: sub(*l1),
            l2: sub(*l2),
        },
        Perpendicular { l1, l2 } => Perpendicular {
            l1: sub(*l1),
            l2: sub(*l2),
        },
        Angle { l1, l2, target } => Angle {
            l1: sub(*l1),
            l2: sub(*l2),
            target: target.clone(),
        },
        EqualLength { l1, l2 } => EqualLength {
            l1: sub(*l1),
            l2: sub(*l2),
        },
        TangentLineArc { line, arc } => TangentLineArc {
            line: sub(*line),
            arc: *arc,
        },
        SymmetricAboutLine { p1, p2, line } => SymmetricAboutLine {
            p1: *p1,
            p2: *p2,
            line: sub(*line),
        },
        Midpoint { point, line } => Midpoint {
            point: *point,
            line: sub(*line),
        },
        DistancePtLine {
            point,
            line,
            target,
        } => DistancePtLine {
            point: *point,
            line: sub(*line),
            target: target.clone(),
        },
        _ => return c.clone(),
    };
    Constraint { id: c.id, kind }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::DimTarget;
    use crate::plane::PlaneId;

    /// A single Line `start -> end` plus its two Point entities.
    /// Returns `(sketch, line, start, end)`.
    fn line_sketch(
        start: (f64, f64),
        end: (f64, f64),
    ) -> (SketchData, SketchEntityId, SketchEntityId, SketchEntityId) {
        let plane = PlaneId::new();
        let start_id = SketchEntityId::new();
        let end_id = SketchEntityId::new();
        let line_id = SketchEntityId::new();
        let mut sketch = SketchData::default();
        sketch.entities.push(Entity::new(
            start_id,
            plane,
            EntityKind::Point {
                x: start.0,
                y: start.1,
            },
        ));
        sketch.entities.push(Entity::new(
            end_id,
            plane,
            EntityKind::Point { x: end.0, y: end.1 },
        ));
        sketch.entities.push(Entity::new(
            line_id,
            plane,
            EntityKind::Line {
                start: start_id,
                end: end_id,
            },
        ));
        (sketch, line_id, start_id, end_id)
    }

    fn line_endpoints(
        sketch: &SketchData,
        line: SketchEntityId,
    ) -> (SketchEntityId, SketchEntityId) {
        match sketch.entities.iter().find(|e| e.id == line).unwrap().kind {
            EntityKind::Line { start, end } => (start, end),
            _ => panic!("not a line"),
        }
    }

    // ─── Plain split ───

    #[test]
    fn mid_split_creates_two_lines_and_drops_original() {
        let (mut sketch, line, start, end) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let result = split_line(&mut sketch, line, 0.5).expect("mid split must succeed");

        assert!(
            sketch.entities.iter().all(|e| e.id != line),
            "original line must be gone"
        );
        assert!(sketch.entities.iter().any(|e| e.id == result.mid_point));
        assert!(sketch.entities.iter().any(|e| e.id == result.line_a));
        assert!(sketch.entities.iter().any(|e| e.id == result.line_b));

        assert_eq!(
            line_endpoints(&sketch, result.line_a),
            (start, result.mid_point)
        );
        assert_eq!(
            line_endpoints(&sketch, result.line_b),
            (result.mid_point, end)
        );

        let (mx, my) = entity_point_xy(&sketch, result.mid_point).unwrap();
        assert!(
            (mx - 5.0).abs() < 1e-9,
            "mid x should interpolate to 5.0, got {mx}"
        );
        assert!(
            (my - 0.0).abs() < 1e-9,
            "mid y should interpolate to 0.0, got {my}"
        );
    }

    #[test]
    fn split_at_non_half_t_interpolates_correctly() {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 20.0));
        let result = split_line(&mut sketch, line, 0.25).unwrap();
        let (mx, my) = entity_point_xy(&sketch, result.mid_point).unwrap();
        assert!((mx - 2.5).abs() < 1e-9);
        assert!((my - 5.0).abs() < 1e-9);
    }

    #[test]
    fn endpoints_shared_not_duplicated() {
        let (mut sketch, line, start, end) = line_sketch((0.0, 0.0), (4.0, 0.0));
        let result = split_line(&mut sketch, line, 0.5).unwrap();

        let mid_count = sketch
            .entities
            .iter()
            .filter(|e| e.id == result.mid_point)
            .count();
        assert_eq!(mid_count, 1, "mid point must exist exactly once");

        // Total entity count: 2 original points + 1 new mid point + 2 new lines.
        assert_eq!(sketch.entities.len(), 5);
        assert!(sketch.entities.iter().any(|e| e.id == start));
        assert!(sketch.entities.iter().any(|e| e.id == end));
    }

    #[test]
    fn bake_attributes_and_flags_carry_onto_both_halves() {
        use crate::attr::SilkAttr;
        use signex_types::layer::SignexLayer;

        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        {
            let e = sketch.entities.iter_mut().find(|e| e.id == line).unwrap();
            e.construction = true;
            e.silk = Some(SilkAttr {
                layer: SignexLayer::TopSilk,
            });
        }
        let result = split_line(&mut sketch, line, 0.5).unwrap();
        for id in [result.line_a, result.line_b] {
            let e = sketch.entities.iter().find(|e| e.id == id).unwrap();
            assert!(e.construction, "construction flag must carry over to {id}");
            assert!(e.silk.is_some(), "silk attribute must carry over to {id}");
        }
    }

    // ─── Constraint carry-over ───

    #[test]
    fn horizontal_constraint_duplicates_onto_both_halves() {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let original_cid = ConstraintId::new();
        sketch.constraints.push(Constraint {
            id: original_cid,
            kind: ConstraintKind::Horizontal { line },
        });

        let result = split_line(&mut sketch, line, 0.5).unwrap();

        let horiz: Vec<&Constraint> = sketch
            .constraints
            .iter()
            .filter(|c| matches!(c.kind, ConstraintKind::Horizontal { .. }))
            .collect();
        assert_eq!(horiz.len(), 2, "Horizontal must duplicate onto both halves");
        let lines: Vec<SketchEntityId> = horiz
            .iter()
            .map(|c| match c.kind {
                ConstraintKind::Horizontal { line } => line,
                _ => unreachable!(),
            })
            .collect();
        assert!(lines.contains(&result.line_a));
        assert!(lines.contains(&result.line_b));
        assert!(
            sketch.constraints.iter().all(|c| !references_line(c, line)),
            "no surviving constraint may reference the retired line id"
        );

        // DOF budget: the split minted exactly one new Point (2 fresh
        // DOF: x, y). The duplicated Horizontal pair spends exactly 2
        // residuals (1 each) — it must not exceed the new DOF budget.
        let residuals: usize = horiz.iter().map(|c| c.kind.residual_count()).sum();
        assert_eq!(
            residuals, 2,
            "duplicated Horizontal pair must spend exactly the 2 new DOF"
        );
    }

    #[test]
    fn equal_length_constraint_repoints_to_one_half_only() {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let (other_sketch, other_line, _, _) = line_sketch((0.0, 5.0), (3.0, 5.0));
        sketch.entities.extend(other_sketch.entities);
        let original_cid = ConstraintId::new();
        sketch.constraints.push(Constraint {
            id: original_cid,
            kind: ConstraintKind::EqualLength {
                l1: line,
                l2: other_line,
            },
        });

        let result = split_line(&mut sketch, line, 0.5).unwrap();

        let matches: Vec<&Constraint> = sketch
            .constraints
            .iter()
            .filter(|c| matches!(c.kind, ConstraintKind::EqualLength { .. }))
            .collect();
        assert_eq!(matches.len(), 1, "EqualLength must not duplicate");
        match matches[0].kind {
            ConstraintKind::EqualLength { l1, l2 } => {
                assert_eq!(l1, result.line_a, "must repoint to line_a, not line_b");
                assert_eq!(
                    l2, other_line,
                    "the untouched partner line must be unchanged"
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(
            matches[0].id, original_cid,
            "id is preserved for the single surviving copy"
        );
        assert!(sketch.constraints.iter().all(|c| !references_line(c, line)));
        assert!(
            sketch
                .constraints
                .iter()
                .all(|c| !references_line(c, result.line_b)),
            "EqualLength must not have been duplicated onto line_b"
        );
    }

    #[test]
    fn distance_pt_pt_on_endpoints_is_untouched() {
        let (mut sketch, line, start, end) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let dist_cid = ConstraintId::new();
        let dist_constraint = Constraint {
            id: dist_cid,
            kind: ConstraintKind::DistancePtPt {
                p1: start,
                p2: end,
                target: DimTarget::Literal(10.0),
            },
        };
        sketch.constraints.push(dist_constraint.clone());

        split_line(&mut sketch, line, 0.5).unwrap();

        let survivors: Vec<&Constraint> = sketch
            .constraints
            .iter()
            .filter(|c| c.id == dist_cid)
            .collect();
        assert_eq!(survivors.len(), 1);
        assert_eq!(
            *survivors[0], dist_constraint,
            "DistancePtPt on endpoints must be byte-identical"
        );
    }

    #[test]
    fn point_on_line_repoints_to_the_half_the_point_falls_on() {
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let plane = sketch.entities[0].plane;
        let near_start = SketchEntityId::new();
        let near_end = SketchEntityId::new();
        sketch.entities.push(Entity::new(
            near_start,
            plane,
            EntityKind::Point { x: 2.0, y: 0.0 },
        ));
        sketch.entities.push(Entity::new(
            near_end,
            plane,
            EntityKind::Point { x: 8.0, y: 0.0 },
        ));
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::PointOnLine {
                point: near_start,
                line,
            },
        });
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::PointOnLine {
                point: near_end,
                line,
            },
        });

        // Split at t=0.5 (x=5.0): near_start (x=2) falls on line_a,
        // near_end (x=8) falls on line_b.
        let result = split_line(&mut sketch, line, 0.5).unwrap();

        let find_target = |point: SketchEntityId| -> SketchEntityId {
            sketch
                .constraints
                .iter()
                .find_map(|c| match c.kind {
                    ConstraintKind::PointOnLine { point: p, line } if p == point => Some(line),
                    _ => None,
                })
                .expect("constraint must survive")
        };
        assert_eq!(find_target(near_start), result.line_a);
        assert_eq!(find_target(near_end), result.line_b);
    }

    fn references_line(c: &Constraint, line: SketchEntityId) -> bool {
        use ConstraintKind::*;
        match &c.kind {
            PointOnLine { line: l, .. }
            | Horizontal { line: l }
            | Vertical { line: l }
            | DistancePtLine { line: l, .. }
            | TangentLineArc { line: l, .. }
            | SymmetricAboutLine { line: l, .. }
            | Midpoint { line: l, .. } => *l == line,
            Parallel { l1, l2 }
            | Perpendicular { l1, l2 }
            | Angle { l1, l2, .. }
            | EqualLength { l1, l2 } => *l1 == line || *l2 == line,
            _ => false,
        }
    }

    // ─── Degenerate inputs — every one must return None and leave
    // `sketch` byte-for-byte unchanged. ───

    #[test]
    fn missing_line_id_returns_none() {
        let (mut sketch, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let before = sketch.clone();
        let bogus = SketchEntityId::new();
        assert!(split_line(&mut sketch, bogus, 0.5).is_none());
        assert_eq!(sketch, before);
    }

    #[test]
    fn wrong_kind_returns_none() {
        let (mut sketch, _line, start, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        let before = sketch.clone();
        // `start` resolves to a Point, not a Line.
        assert!(split_line(&mut sketch, start, 0.5).is_none());
        assert_eq!(sketch, before);
    }

    #[test]
    fn zero_length_line_returns_none() {
        let (mut sketch, line, ..) = line_sketch((3.0, 3.0), (3.0, 3.0));
        let before = sketch.clone();
        assert!(split_line(&mut sketch, line, 0.5).is_none());
        assert_eq!(sketch, before);
    }

    #[test]
    fn t_at_or_beyond_an_endpoint_returns_none() {
        for t in [0.0, 1.0, -0.1, 1.1, -5.0, 5.0] {
            let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
            let before = sketch.clone();
            assert!(
                split_line(&mut sketch, line, t).is_none(),
                "t={t} must be rejected"
            );
            assert_eq!(sketch, before, "t={t} must leave sketch unchanged");
        }
    }

    #[test]
    fn t_within_epsilon_of_an_endpoint_returns_none() {
        for t in [1e-12, 1.0 - 1e-12] {
            let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
            let before = sketch.clone();
            assert!(
                split_line(&mut sketch, line, t).is_none(),
                "t={t} must be rejected"
            );
            assert_eq!(sketch, before);
        }
    }

    #[test]
    fn t_nan_or_infinite_returns_none() {
        for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
            let before = sketch.clone();
            assert!(split_line(&mut sketch, line, t).is_none());
            assert_eq!(sketch, before);
        }
    }

    #[test]
    fn unresolvable_endpoint_returns_none() {
        let plane = PlaneId::new();
        let start_id = SketchEntityId::new();
        let dangling_end = SketchEntityId::new(); // never inserted as an entity
        let line_id = SketchEntityId::new();
        let mut sketch = SketchData::default();
        sketch.entities.push(Entity::new(
            start_id,
            plane,
            EntityKind::Point { x: 0.0, y: 0.0 },
        ));
        sketch.entities.push(Entity::new(
            line_id,
            plane,
            EntityKind::Line {
                start: start_id,
                end: dangling_end,
            },
        ));
        let before = sketch.clone();
        assert!(split_line(&mut sketch, line_id, 0.5).is_none());
        assert_eq!(sketch, before);
    }

    #[test]
    fn failed_split_leaves_sketch_byte_identical() {
        // Belt-and-braces: a constraint present pre-call must survive
        // untouched too, not just the entity list.
        let (mut sketch, line, ..) = line_sketch((0.0, 0.0), (10.0, 0.0));
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::Horizontal { line },
        });
        let before = sketch.clone();
        assert!(split_line(&mut sketch, line, 1.5).is_none());
        assert_eq!(
            sketch, before,
            "a rejected split must not touch entities or constraints"
        );
    }
}
