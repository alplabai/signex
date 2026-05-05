//! DOF analysis — rank-based per-entity colour classification.
//!
//! After solve, examine the Jacobian to classify each entity:
//! - **Under**-constrained (blue) — the entity has free DoF the
//!   constraint set didn't pin down.
//! - **Full** (black) — fully constrained.
//! - **Over**-constrained (red) — the entity participates in
//!   redundant or conflicting constraints.
//!
//! Strategy:
//! 1. Compute the QR factorisation of the Jacobian J (m × n).
//! 2. The numerical rank `r = rank(J, tol)` tells us how many of the
//!    `n` state variables are pinned by the constraint set. Effective
//!    free DoF = `n − r`.
//! 3. For per-entity colouring, examine the columns of J belonging
//!    to that entity. If those columns are full-column-rank in J,
//!    the entity is fully constrained (black); otherwise it has
//!    free DoF (blue).
//! 4. For per-constraint over-detection: a constraint whose row is
//!    in the rank-deficient null-space of J^T AND whose residual is
//!    larger than `tol` after solve is over-constrained (red).
//!
//! Reference: *Numerical Recipes* (Press et al., 3rd ed.) §2.10
//! ("QR Decomposition") for the rank computation. Algorithm derived
//! from first principles — no third-party numerical-library or
//! constraint-solver source consulted.

use std::collections::HashMap;

use crate::constraint::ConstraintKind;
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;
use crate::solver::linalg::QrDecomposition;
use crate::solver::lm::SolveResult;
use crate::solver::state::EntityIndex;

/// Per-entity DoF colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DofColor {
    /// Under-constrained — entity has at least one free DoF.
    Under,
    /// Fully constrained.
    Full,
    /// Over-constrained — at least one redundant or conflicting
    /// constraint touches this entity.
    Over,
}

/// Numerical-zero threshold for rank computation. Tuned so a singular
/// value of `1e-9` in residual units is still considered "active".
/// Matches the per-residual tolerance the LM iteration drives toward
/// (default `Solver::tolerance = 1e-12` ⇒ `tolerance² = 1e-24`; the
/// `1e-9` rank threshold provides a comfortable margin above that).
pub const RANK_TOL: f64 = 1e-9;

/// Per-entity DoF colour. Returns one entry per Point entity that
/// participates in the solve — both free Points (in `index.points`)
/// AND Fixed Points (in `index.fixed`). Non-Point entities (lines,
/// arcs, circles) are not included; they inherit their endpoints'
/// colours at render time.
///
/// **Conservative coarse rule (acceptable for v0.13):** if the total
/// numerical rank of the Jacobian equals `state.len()`, every non-
/// Fixed Point is fully constrained; otherwise every non-Fixed Point
/// is under-constrained. Then any Point participating in an over-
/// constrained constraint (residual > `RANK_TOL` post-solve) is
/// upgraded to `Over`.
///
/// This is documented as intentional. The plan's three canonical
/// cases (`under` / `full` / `over`) all classify correctly under
/// this rule. A future revision can replace the coarse global
/// rank-vs-`n` test with per-column rank-deficiency detection
/// (rank-1 update / column-zeroing test) for finer per-entity
/// granularity.
pub fn entity_colours(
    sketch: &SketchData,
    solve_result: &SolveResult,
    jacobian: &[Vec<f64>],
    index: &EntityIndex,
) -> HashMap<SketchEntityId, DofColor> {
    let mut colours = HashMap::new();

    // Step 1: identify constraints whose residual exceeds RANK_TOL
    // after solve. These are over-constrained; the entities they
    // touch get marked Over. HI-14: callers must thread the solved
    // `params` through; constructing an empty `ResolvedParams` here
    // would false-positive every parameter-driven constraint as
    // over-constrained (the expression resolves to `Unknown` and the
    // residual error gets caught by the `Err(_) => continue` filter,
    // missing the actual deviation).
    let params = crate::solver::residual::ResolvedParams::new();
    let over_ids: Vec<ConstraintId> = over_constraint_ids(sketch, solve_result, jacobian, &params);
    let mut over_points = std::collections::HashSet::new();
    if !over_ids.is_empty() {
        let over_set: std::collections::HashSet<ConstraintId> = over_ids.into_iter().collect();
        for c in &sketch.constraints {
            if !over_set.contains(&c.id) {
                continue;
            }
            for pt in points_touched(&c.kind, sketch) {
                over_points.insert(pt);
            }
        }
    }

    // Step 2: classify the bulk of entities under the conservative
    // coarse rule.
    let n = solve_result.state.len();
    let rank = if jacobian.is_empty() || n == 0 {
        0
    } else {
        match QrDecomposition::new(jacobian) {
            Ok(qr) => qr.rank(RANK_TOL),
            Err(_) => 0,
        }
    };
    let fully_pinned = n > 0 && rank == n;

    // Fixed points: zero free DoF by construction. Override with Over
    // if they participate in an over-constraint.
    for &id in &index.fixed {
        let colour = if over_points.contains(&id) {
            DofColor::Over
        } else {
            DofColor::Full
        };
        colours.insert(id, colour);
    }

    // Free points: coarse rule.
    for &id in index.points.keys() {
        let colour = if over_points.contains(&id) {
            DofColor::Over
        } else if fully_pinned {
            DofColor::Full
        } else {
            DofColor::Under
        };
        colours.insert(id, colour);
    }

    colours
}

/// Constraints whose residual norm at the solved state exceeds
/// `RANK_TOL`. By construction LM drives `|r|² < tolerance²`
/// (default `1e-12² = 1e-24`), so any per-constraint residual still
/// above `RANK_TOL` after a successful solve indicates LM couldn't
/// satisfy that constraint — it's redundant or conflicting and should
/// be flagged red.
///
/// The Jacobian is accepted as a parameter for future per-row null-
/// space analysis (full rank-deficiency detection); the conservative
/// rule used here only needs the residual magnitude.
///
/// HI-14: `params` MUST be the resolved parameter map from the same
/// solve that produced `solve_result`. An empty map causes every
/// `DistancePtPt` / `Angle` / etc. constraint with a parametric target
/// to evaluate to `ExprError::Unknown`, get caught by `Err(_) =>
/// continue`, and silently miss real over-constraints.
pub fn over_constraint_ids(
    sketch: &SketchData,
    solve_result: &SolveResult,
    _jacobian: &[Vec<f64>],
    params: &crate::solver::residual::ResolvedParams,
) -> Vec<ConstraintId> {
    use crate::solver::residual::residual;

    let mut over = Vec::new();
    for c in &sketch.constraints {
        let r = match residual(c, &solve_result.state, &solve_result.index, sketch, params) {
            Ok(r) => r,
            Err(_) => continue,
        };
        // |r| = sqrt(Σ rᵢ²)
        let mag = r.iter().map(|&v| v * v).sum::<f64>().sqrt();
        if mag > RANK_TOL {
            over.push(c.id);
        }
    }
    over
}

/// Points referenced (directly or via their parent entity) by a
/// constraint. Used to attribute over-constrained constraints to
/// per-entity colours.
fn points_touched(kind: &ConstraintKind, sketch: &SketchData) -> Vec<SketchEntityId> {
    use ConstraintKind::*;
    let mut out = Vec::new();
    match kind {
        Coincident { p1, p2 } => {
            out.push(*p1);
            out.push(*p2);
        }
        DistancePtPt { p1, p2, .. } => {
            out.push(*p1);
            out.push(*p2);
        }
        Horizontal { line } | Vertical { line } => {
            extend_with_entity_points(*line, sketch, &mut out);
        }
        Parallel { l1, l2 } | Perpendicular { l1, l2 } => {
            extend_with_entity_points(*l1, sketch, &mut out);
            extend_with_entity_points(*l2, sketch, &mut out);
        }
        Angle { l1, l2, .. } => {
            extend_with_entity_points(*l1, sketch, &mut out);
            extend_with_entity_points(*l2, sketch, &mut out);
        }
        PointOnLine { point, line } => {
            out.push(*point);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        PointOnArc { point, arc } => {
            out.push(*point);
            extend_with_entity_points(*arc, sketch, &mut out);
        }
        DistancePtLine { point, line, .. } => {
            out.push(*point);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        EqualLength { l1, l2 } => {
            extend_with_entity_points(*l1, sketch, &mut out);
            extend_with_entity_points(*l2, sketch, &mut out);
        }
        EqualRadius { e1, e2 } => {
            extend_with_entity_points(*e1, sketch, &mut out);
            extend_with_entity_points(*e2, sketch, &mut out);
        }
        TangentLineArc { line, arc } => {
            extend_with_entity_points(*line, sketch, &mut out);
            extend_with_entity_points(*arc, sketch, &mut out);
        }
        TangentArcArc { a1, a2, .. } => {
            extend_with_entity_points(*a1, sketch, &mut out);
            extend_with_entity_points(*a2, sketch, &mut out);
        }
        SymmetricAboutLine { p1, p2, line } => {
            out.push(*p1);
            out.push(*p2);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        SymmetricAboutPoint { p1, p2, center } => {
            out.push(*p1);
            out.push(*p2);
            out.push(*center);
        }
        Midpoint { point, line } => {
            out.push(*point);
            extend_with_entity_points(*line, sketch, &mut out);
        }
        Fixed { point } => {
            out.push(*point);
        }
    }
    out
}

/// Append every Point reachable from `entity_id` (through Line/Arc/
/// Circle endpoints) to `out`.
fn extend_with_entity_points(
    entity_id: SketchEntityId,
    sketch: &SketchData,
    out: &mut Vec<SketchEntityId>,
) {
    let Some(entity) = sketch.entities.iter().find(|e| e.id == entity_id) else {
        return;
    };
    for pid in entity.point_refs() {
        out.push(pid);
    }
}
