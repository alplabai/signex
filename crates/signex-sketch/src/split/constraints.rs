//! Constraint carry-over for a completed [`super::SplitCtx`] split —
//! rewrites, duplicates, or drops every constraint that named the
//! retired Line so none dangles. See [`super::split_line`]'s doc
//! comment for the per-kind table this module implements.

use crate::constraint::{Constraint, ConstraintKind};
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;

use super::{SplitCtx, entity_point_xy};

/// Rewrite every constraint in `sketch.constraints` against a
/// completed split. Returns the full replacement list plus the ids of
/// any constraint dropped outright (today only a `Midpoint` naming the
/// retired line — see [`is_dropped_midpoint`]).
pub(super) fn split_constraints(
    sketch: &SketchData,
    ctx: &SplitCtx,
) -> (Vec<Constraint>, Vec<ConstraintId>) {
    let mut kept = Vec::with_capacity(sketch.constraints.len());
    let mut dropped = Vec::new();
    for c in &sketch.constraints {
        if is_dropped_midpoint(c, ctx) {
            dropped.push(c.id);
            continue;
        }
        kept.extend(split_constraint(c, sketch, ctx));
    }
    (kept, dropped)
}

/// `Midpoint` names the retired line — its referent is destroyed by
/// the split (the original line's midpoint is the midpoint of NEITHER
/// half), unlike `Parallel` / `EqualLength` / etc. where `line_a`
/// genuinely still holds the relation. Dropped rather than re-pointed;
/// the id is surfaced via `SplitResult::dropped_constraints` instead
/// of silently vanishing.
fn is_dropped_midpoint(c: &Constraint, ctx: &SplitCtx) -> bool {
    matches!(&c.kind, ConstraintKind::Midpoint { line, .. } if *line == ctx.line)
}

/// Rewrite one constraint (already confirmed not a dropped `Midpoint`)
/// against a completed split. Returns 1 or 2 replacement constraints.
/// A constraint whose fields never named `line` (including every kind
/// that can only reference a Point / Arc / Circle — `Coincident`,
/// `DistancePtPt`, `Fixed`, `PointOnArc`, `DistancePtCircle`,
/// `EqualRadius`, `TangentArcArc`, `SymmetricAboutPoint`) is returned
/// unchanged: none of their fields can dangle from this split, since
/// the original Line's endpoint Points keep their ids and coordinates
/// untouched — only the Line entity itself is retired.
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
/// fresh DOF the original 2-point Line never had. Duplicating spends 2
/// residuals total (1 per half): the copy on `line_a` pins `mid`'s
/// axis coordinate to `start`'s (the one genuinely new relation), and
/// the copy on `line_b` re-derives `end`'s axis coordinate from
/// `mid`'s — already implied once `line_a`'s copy holds, but harmless
/// to restate, and it is what keeps `line_b` independently straight if
/// `mid` is later dragged off both lines by an unrelated edit. Net DOF
/// goes from 3 free (the original Line's endpoints, one axis pinned)
/// to 4 free (both halves' endpoints, one axis pinned on each) — not
/// "exactly balanced" against the 2 new residuals, but rank 2 over the
/// 2 new (`mid.x`, `mid.y`) variables with `mid.x` left free is
/// correct and cannot over-constrain by itself.
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
/// actually falls on, compared against the split parameter `t`.
///
/// `PointOnLine`'s residual is the signed perpendicular distance to
/// the INFINITE line through `line`'s endpoints
/// (`solver::residuals::point_on::point_on_line`), not a
/// segment-bounded containment check, so leaving it pointed at either
/// half wouldn't literally fail to resolve. What makes re-pointing the
/// right choice is that the two halves stop being collinear once
/// either can hinge independently around `mid` on a later solve — "the
/// infinite line through `line_a`" and "the infinite line through
/// `line_b`" are no longer the same line. Following the point to
/// whichever half it currently sits nearest keeps the constraint
/// pinning the SAME relationship the user authored, instead of quietly
/// re-defining it against a line that may end up passing somewhere
/// else entirely.
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
/// `TangentLineArc` / `SymmetricAboutLine` / `DistancePtLine` —
/// re-pointed onto `line_a` only; everything else (including a
/// `Midpoint` NOT naming the retired line) passes through unchanged.
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
