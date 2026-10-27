//! Per-solve cache for parametric dimension targets.
//!
//! A dimension driven by an expression (`= width / 2`) used to be parsed,
//! and the parameter table rebuilt into an evaluation context, on every
//! residual evaluation: `2n` times per Jacobian, every iteration. The solve
//! builds a [`DimCache`] once, inside [`crate::solver::state::pack`], and the
//! residual layer reads it through the [`crate::solver::state::EntityIndex`].
//!
//! The cache never changes a result. An expression it did not see, or a
//! parameter set other than the one it captured, takes the uncached path.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use crate::constraint::DimTarget;
use crate::expr::ExprError;
use crate::expr::ast::ExprNode;
use crate::expr::eval::EvalContext;
use crate::expr::parse::parse as parse_expr;
use crate::sketch::SketchData;
use crate::solver::residual::ResolvedParams;
use crate::unit::Quantity;

/// The text of an expression target as the parser sees it.
fn expression_body(source: &str) -> &str {
    source.trim().trim_start_matches('=').trim()
}

/// Parse `body`, counting the call in tests.
pub(crate) fn parse_counted(body: &str) -> Result<ExprNode, ExprError> {
    #[cfg(test)]
    PARSE_CALLS.with(|calls| calls.set(calls.get() + 1));
    parse_expr(body)
}

#[cfg(test)]
thread_local! {
    static PARSE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many expression parses this thread has run (tests only).
#[cfg(test)]
pub(crate) fn parse_calls() -> usize {
    PARSE_CALLS.with(std::cell::Cell::get)
}

/// The evaluation context of one parameter set. `ResolvedParams` values are
/// canonical-unit f64 with the unit family implied by the constraint
/// context; they are injected as Length (mm) since Distance is the dominant
/// expression-driven case. Angle-only constraints with parameter-driven
/// targets typically reference the parameter directly via the bare-name
/// fast path in `resolve_dim`.
pub(crate) fn build_context(params: &ResolvedParams) -> EvalContext {
    let params_ast: BTreeMap<String, ExprNode> = params
        .iter()
        .map(|(name, value)| (name.clone(), ExprNode::Literal(Quantity::length(*value))))
        .collect();
    EvalContext {
        params: params_ast,
        array_index: None,
    }
}

#[derive(Debug, Clone)]
struct CapturedContext {
    values: ResolvedParams,
    context: EvalContext,
}

/// Expressions parsed once and the parameter context built once for a solve.
#[derive(Debug, Default, Clone)]
pub struct DimCache {
    parsed: HashMap<String, Result<ExprNode, ExprError>>,
    captured: OnceLock<CapturedContext>,
}

impl DimCache {
    /// Parse every expression target of `sketch` once.
    pub fn for_sketch(sketch: &SketchData) -> Self {
        let mut parsed = HashMap::new();
        for target in sketch
            .constraints
            .iter()
            .filter_map(|c| c.kind.dim_target())
        {
            if let DimTarget::Expr(source) = target {
                let body = expression_body(source);
                if !parsed.contains_key(body) {
                    parsed.insert(body.to_owned(), parse_counted(body));
                }
            }
        }
        Self {
            parsed,
            captured: OnceLock::new(),
        }
    }

    /// The parse of `body`, when the sketch had that expression.
    pub(crate) fn parsed(&self, body: &str) -> Option<Result<&ExprNode, ExprError>> {
        self.parsed
            .get(body)
            .map(|result| result.as_ref().map_err(Clone::clone))
    }

    /// The evaluation context for `params`, built on first use. `None` when
    /// the cache already holds a different parameter set.
    pub(crate) fn context(&self, params: &ResolvedParams) -> Option<&EvalContext> {
        let captured = self.captured.get_or_init(|| CapturedContext {
            values: params.clone(),
            context: build_context(params),
        });
        (captured.values == *params).then_some(&captured.context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::{Constraint, ConstraintKind};
    use crate::entity::{Entity, EntityKind};
    use crate::id::{ConstraintId, SketchEntityId};
    use crate::solver::Solver;
    use crate::solver::residual::resolve_dim;

    /// Two points tied by one expression-driven distance and a horizontal
    /// line, so the solve iterates and evaluates the expression many times.
    fn expression_sketch(expr: &str) -> (SketchData, SketchEntityId, SketchEntityId) {
        let mut data = SketchData::default();
        let plane = crate::plane::PlaneId::new();
        let (a, b, line) = (
            SketchEntityId::new(),
            SketchEntityId::new(),
            SketchEntityId::new(),
        );
        data.entities.extend([
            Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(b, plane, EntityKind::Point { x: 1.0, y: 0.7 }),
            Entity::new(line, plane, EntityKind::Line { start: a, end: b }),
        ]);
        data.constraints.extend([
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Fixed { point: a },
            },
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Horizontal { line },
            },
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::DistancePtPt {
                    p1: a,
                    p2: b,
                    target: DimTarget::Expr(expr.to_owned()),
                },
            },
        ]);
        (data, a, b)
    }

    fn params() -> ResolvedParams {
        ResolvedParams::from([("width".to_owned(), 8.0), ("gap".to_owned(), 0.5)])
    }

    fn point(
        out: &crate::solver::FullSolveOutput,
        data: &SketchData,
        id: SketchEntityId,
    ) -> (f64, f64) {
        crate::solver::state::point_xy(id, &out.result.state, &out.result.index, data)
            .expect("point")
    }

    #[test]
    fn solve_parses_each_expression_once() {
        let (data, _, _) = expression_sketch("= width / 2 + gap");
        let before = parse_calls();
        Solver::default().solve(&data, &params()).expect("solves");
        // One parse for the one expression, however many residual
        // evaluations the Jacobian and the iterations ran.
        assert_eq!(parse_calls() - before, 1);
    }

    #[test]
    fn cached_solve_matches_uncached_resolution() {
        let (data, a, b) = expression_sketch("= width / 2 + gap");
        let solved = Solver::default().solve(&data, &params()).expect("solves");
        let (ax, ay) = point(&solved, &data, a);
        let (bx, by) = point(&solved, &data, b);
        let target =
            resolve_dim(&DimTarget::Expr("= width / 2 + gap".into()), &params()).expect("resolves");
        assert_eq!(target, 4.5);
        assert!(((bx - ax).hypot(by - ay) - target).abs() < 1e-9);
        assert!((by - ay).abs() < 1e-9, "horizontal");
        // Pinned positions: the cached path must keep producing these.
        assert!((ax - 0.0).abs() < 1e-12 && (ay - 0.0).abs() < 1e-12);
        assert!((bx - 4.5).abs() < 1e-9, "bx = {bx}");
    }

    #[test]
    fn a_different_parameter_set_takes_the_uncached_path() {
        let (data, _, _) = expression_sketch("= width / 2 + gap");
        let cache = DimCache::for_sketch(&data);
        assert!(cache.context(&params()).is_some());
        let other = ResolvedParams::from([("width".to_owned(), 1.0)]);
        assert!(cache.context(&other).is_none());
        assert!(cache.context(&params()).is_some());
    }

    #[test]
    fn a_parse_error_is_cached_and_reported() {
        let (data, _, _) = expression_sketch("= width +");
        let cache = DimCache::for_sketch(&data);
        assert!(matches!(cache.parsed("width +"), Some(Err(_))));
    }
}
