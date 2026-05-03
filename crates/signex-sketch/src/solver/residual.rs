//! Constraint residual functions.
//!
//! Each [`ConstraintKind`] variant maps to a residual `f(state) -> R^k`
//! where `k = ConstraintKind::residual_count()`. The solver drives every
//! component of the concatenated residual vector toward zero via
//! Levenberg–Marquardt iteration (Phase 3).
//!
//! The dispatcher in [`residual`] is thin — it routes to per-family
//! helpers in [`crate::solver::residuals`]. Each family is in its own
//! module so the residual implementations can grow without this file
//! becoming a single shared bottleneck.

use std::collections::HashMap;

use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::error::SketchError;
use crate::sketch::SketchData;
use crate::solver::residuals::{equal_tangent, point_on, parallel_perp_angle, symmetric_midpoint};
use crate::solver::state::{point_xy, EntityIndex, line_endpoints};

/// Resolved-parameter table — `name → f64` in canonical units (mm
/// for lengths, rad for angles). Phase 4 produces this table from the
/// [`crate::expr`] AST evaluator. Phase 2 honours
/// `DimTarget::Literal` only and looks up `DimTarget::Expr` strings as
/// bare parameter names.
pub type ResolvedParams = HashMap<String, f64>;

/// Resolve a [`DimTarget`] against the parameter table. Literal
/// values pass through; expression strings are stripped of a leading
/// `=` and then looked up by name in `params`. Phase 4 replaces the
/// lookup with full expression evaluation.
pub fn resolve_dim(target: &DimTarget, params: &ResolvedParams) -> Result<f64, SketchError> {
    match target {
        DimTarget::Literal(v) => Ok(*v),
        DimTarget::Expr(s) => {
            let key = s.trim().trim_start_matches('=').trim();
            params
                .get(key)
                .copied()
                .ok_or_else(|| SketchError::ParameterNotFound(s.clone()))
        }
    }
}

/// Top-level residual dispatcher. Returns the residual vector for the
/// given constraint, of length `c.kind.residual_count()`.
pub fn residual(
    c: &Constraint,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
    params: &ResolvedParams,
) -> Result<Vec<f64>, SketchError> {
    use ConstraintKind::*;
    match &c.kind {
        // ─── Task 2.3: Coincident, DistancePtPt, Horizontal, Vertical, Fixed ───
        Coincident { p1, p2 } => {
            let (x1, y1) = point_xy(*p1, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(*p1))?;
            let (x2, y2) = point_xy(*p2, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(*p2))?;
            Ok(vec![x2 - x1, y2 - y1])
        }
        DistancePtPt { p1, p2, target } => {
            let (x1, y1) = point_xy(*p1, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(*p1))?;
            let (x2, y2) = point_xy(*p2, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(*p2))?;
            let d = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
            let t = resolve_dim(target, params)?;
            Ok(vec![d - t])
        }
        Horizontal { line } => {
            let (s, e) = line_endpoints(*line, sketch)
                .ok_or(SketchError::EntityNotFound(*line))?;
            let (_, y1) = point_xy(s, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(s))?;
            let (_, y2) = point_xy(e, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(e))?;
            Ok(vec![y2 - y1])
        }
        Vertical { line } => {
            let (s, e) = line_endpoints(*line, sketch)
                .ok_or(SketchError::EntityNotFound(*line))?;
            let (x1, _) = point_xy(s, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(s))?;
            let (x2, _) = point_xy(e, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(e))?;
            Ok(vec![x2 - x1])
        }
        Fixed { .. } => Ok(vec![]),

        // ─── Task 2.4: parallel_perp_angle ───
        Parallel { l1, l2 } => parallel_perp_angle::parallel(*l1, *l2, state, index, sketch),
        Perpendicular { l1, l2 } => {
            parallel_perp_angle::perpendicular(*l1, *l2, state, index, sketch)
        }
        Angle { l1, l2, target } => {
            let t = resolve_dim(target, params)?;
            parallel_perp_angle::angle(*l1, *l2, t, state, index, sketch)
        }

        // ─── Task 2.5: point_on ───
        PointOnLine { point, line } => point_on::point_on_line(*point, *line, state, index, sketch),
        PointOnArc { point, arc } => point_on::point_on_arc(*point, *arc, state, index, sketch),
        DistancePtLine { point, line, target } => {
            let t = resolve_dim(target, params)?;
            point_on::distance_pt_line(*point, *line, t, state, index, sketch)
        }

        // ─── Task 2.6: equal_tangent ───
        EqualLength { l1, l2 } => equal_tangent::equal_length(*l1, *l2, state, index, sketch),
        EqualRadius { e1, e2 } => equal_tangent::equal_radius(*e1, *e2, state, index, sketch),
        TangentLineArc { line, arc } => {
            equal_tangent::tangent_line_arc(*line, *arc, state, index, sketch)
        }
        TangentArcArc { a1, a2, internal } => {
            equal_tangent::tangent_arc_arc(*a1, *a2, *internal, state, index, sketch)
        }

        // ─── Task 2.7: symmetric_midpoint ───
        SymmetricAboutLine { p1, p2, line } => {
            symmetric_midpoint::symmetric_about_line(*p1, *p2, *line, state, index, sketch)
        }
        SymmetricAboutPoint { p1, p2, center } => {
            symmetric_midpoint::symmetric_about_point(*p1, *p2, *center, state, index, sketch)
        }
        Midpoint { point, line } => {
            symmetric_midpoint::midpoint(*point, *line, state, index, sketch)
        }
    }
}
