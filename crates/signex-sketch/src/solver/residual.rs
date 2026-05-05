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

use std::collections::{BTreeMap, HashMap};

use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::error::SketchError;
use crate::expr::ast::ExprNode;
use crate::expr::eval::{EvalContext, eval as eval_expr};
use crate::expr::parse::parse as parse_expr;
use crate::sketch::SketchData;
use crate::solver::residuals::{equal_tangent, parallel_perp_angle, point_on, symmetric_midpoint};
use crate::solver::state::{EntityIndex, line_endpoints, point_xy};
use crate::unit::{Quantity, UnitFamily};

/// Resolved-parameter table — `name → f64` in canonical units (mm
/// for lengths, rad for angles, raw for counts). Produced by
/// [`crate::parameter::resolve`] from the user-authored
/// [`crate::parameter::ParameterTable`]. The Solver's pipeline
/// resolves the table once before the LM iteration starts and reuses
/// the resulting `ResolvedParams` across all per-residual calls.
///
/// This stays a flat `HashMap<String, f64>` rather than a typed
/// quantity table so the residual layer keeps a tight, allocation-
/// free hot path. The unit family of each value is implied by the
/// constraint that consumes it (Distance → mm, Angle → rad).
pub type ResolvedParams = HashMap<String, f64>;

/// Resolve a [`DimTarget`] against the parameter table.
///
/// `Literal` values pass through unchanged. `Expr` strings are
/// parsed and evaluated through the [`crate::expr`] machinery —
/// supporting arithmetic, comparisons, ternaries, and `lookup(...)`
/// calls. The optional Altium-style `=` prefix is stripped before
/// parsing.
///
/// `ResolvedParams` values are injected into the eval context as
/// `Literal(Quantity::length(v))`. This works cleanly for length-
/// family expressions (Distance constraints) where each parameter
/// is a length in mm. Angle-family parameters work the same way at
/// the bare-name lookup level (`= apex_angle`); for parameter-driven
/// arithmetic on angles, write the literal unit explicitly
/// (e.g. `= apex_angle * 1rad / 1rad` reduces back to the raw
/// canonical value, but the more idiomatic form is to keep the
/// expression inline rather than chained through a Length-typed
/// parameter table).
pub fn resolve_dim(target: &DimTarget, params: &ResolvedParams) -> Result<f64, SketchError> {
    match target {
        DimTarget::Literal(v) => Ok(*v),
        DimTarget::Expr(s) => {
            let body = s.trim().trim_start_matches('=').trim();
            // Fast path: single bare identifier.
            if !body.is_empty()
                && body.chars().all(|c| c.is_alphanumeric() || c == '_')
                && !body.chars().next().unwrap().is_ascii_digit()
            {
                if let Some(v) = params.get(body) {
                    return Ok(*v);
                }
                // Bare-name with no entry — fall through to full parse so
                // the error path produces a structured ExprError.
            }

            let ast = parse_expr(body).map_err(SketchError::Expr)?;
            let mut params_ast: BTreeMap<String, ExprNode> = BTreeMap::new();
            for (name, value) in params {
                // ResolvedParams values are canonical-unit f64 with the
                // unit family implied by the constraint context. We
                // inject as Length(mm) since Distance is the dominant
                // expression-driven case; Angle-only constraints with
                // parameter-driven targets typically reference the
                // parameter directly via the bare-name fast path above.
                params_ast.insert(name.clone(), ExprNode::Literal(Quantity::length(*value)));
            }
            let ctx = EvalContext {
                params: params_ast,
                array_index: None,
            };
            let q = eval_expr(&ast, &ctx).map_err(SketchError::Expr)?;
            match q.unit.family() {
                UnitFamily::Length => q.as_mm().map_err(SketchError::Unit),
                UnitFamily::Angle => q.as_rad().map_err(SketchError::Unit),
                UnitFamily::Count => Ok(q.value),
            }
        }
    }
}

/// Total residual vector for an entire sketch — concatenates the
/// per-constraint residual vectors in `sketch.constraints` order.
///
/// `total_residual` is the function the Levenberg–Marquardt iteration
/// (Phase 3) drives toward zero. The output length is the sum of
/// `ConstraintKind::residual_count()` across all constraints. The
/// state vector length is unrelated; the Jacobian is `(m × n)` where
/// `m = total_residual.len()` and `n = state.len()`.
pub fn total_residual(
    sketch: &SketchData,
    state: &[f64],
    index: &EntityIndex,
    params: &ResolvedParams,
) -> Result<Vec<f64>, SketchError> {
    let mut out = Vec::new();
    for c in &sketch.constraints {
        out.extend(residual(c, state, index, sketch, params)?);
    }
    Ok(out)
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
            let (x1, y1) =
                point_xy(*p1, state, index, sketch).ok_or(SketchError::EntityNotFound(*p1))?;
            let (x2, y2) =
                point_xy(*p2, state, index, sketch).ok_or(SketchError::EntityNotFound(*p2))?;
            Ok(vec![x2 - x1, y2 - y1])
        }
        DistancePtPt { p1, p2, target } => {
            let (x1, y1) =
                point_xy(*p1, state, index, sketch).ok_or(SketchError::EntityNotFound(*p1))?;
            let (x2, y2) =
                point_xy(*p2, state, index, sketch).ok_or(SketchError::EntityNotFound(*p2))?;
            let d = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
            let t = resolve_dim(target, params)?;
            Ok(vec![d - t])
        }
        Horizontal { line } => {
            let (s, e) = line_endpoints(*line, sketch).ok_or(SketchError::EntityNotFound(*line))?;
            let (_, y1) =
                point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
            let (_, y2) =
                point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
            Ok(vec![y2 - y1])
        }
        Vertical { line } => {
            let (s, e) = line_endpoints(*line, sketch).ok_or(SketchError::EntityNotFound(*line))?;
            let (x1, _) =
                point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
            let (x2, _) =
                point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
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
        DistancePtLine {
            point,
            line,
            target,
        } => {
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
