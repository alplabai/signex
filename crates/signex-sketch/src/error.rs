use thiserror::Error;

#[derive(Debug, Error)]
pub enum SketchError {
    #[error("entity {0} not found")]
    EntityNotFound(crate::id::SketchEntityId),
    #[error("parameter {0} not found")]
    ParameterNotFound(String),
    #[error("expression error: {0}")]
    Expr(#[from] crate::expr::ExprError),
    #[error("unit error: {0}")]
    Unit(#[from] crate::unit::UnitError),
    /// A failure from the LM solver.
    #[error("solve failed: {0}")]
    SolveFailed(#[from] SolveError),
    /// A constraint needs the direction of a line whose endpoints
    /// coincide. The solver reports it as [`SolveError::DegenerateLine`].
    #[error("{}", degenerate_line_message(.0))]
    DegenerateLine(crate::id::SketchEntityId),
}

#[derive(Clone, Debug, Error)]
pub enum SolveError {
    /// LO-13: carries the final residual norm so a UI / monitoring layer
    /// can show "almost there (|r|=1e-6)" vs "completely stuck (|r|=1e3)"
    /// after divergence, not just the iteration count.
    #[error("did not converge after {iters} iterations (|r|={final_residual_norm})")]
    DidNotConverge {
        iters: usize,
        final_residual_norm: f64,
    },
    /// The constraints cannot all hold: LM stalled at a least-squares
    /// minimum with these constraints still violated. Empty when the
    /// normal equations went singular and no constraint can be named.
    #[error("over-constrained: conflicting constraints {}", join_ids(.0))]
    OverConstrained(Vec<crate::id::ConstraintId>),
    /// The sketch names entities it does not contain (a hand-edited or
    /// corrupt file, or a buggy edit). Nothing was solved; deleting or
    /// restoring the named entities makes the sketch solvable again.
    #[error("sketch references missing entities: {}", join_ids(.0))]
    DanglingReference(Vec<crate::id::SketchEntityId>),
    /// A constraint needs the direction of a line whose endpoints
    /// coincide (zero length). Nothing was solved; moving an endpoint
    /// apart or deleting the line makes the sketch solvable again.
    #[error("{}", degenerate_line_message(.0))]
    DegenerateLine(crate::id::SketchEntityId),
    /// The solve used up its work budget (work units: residual-row
    /// evaluations and the dense linear algebra, a count that depends only
    /// on the sketch, never on the clock) before converging. The same sketch
    /// always stops at the same point.
    #[error("solve exceeded its work budget ({spent} > {budget} work units)")]
    BudgetExceeded { spent: u64, budget: u64 },
    /// One solver step needs more work than the whole budget, so nothing was
    /// attempted. Raising [`crate::solver::Solver::work_budget`] or splitting
    /// the sketch is the fix; `spent` would be meaningless here.
    #[error(
        "sketch too large for the solve budget: one step needs {needed} work units, \
         the budget is {budget}"
    )]
    TooLarge { needed: u64, budget: u64 },
    /// LO-12: covers genuine internal failures (LU dimension mismatch,
    /// lu.solve back-sub error, total_residual error on initial pass).
    /// Reserve `OverConstrained` for the semantic case where a real
    /// constraint id can be named.
    #[error("solver internal error: {0}")]
    Internal(String),
}

fn degenerate_line_message(line: &crate::id::SketchEntityId) -> String {
    format!(
        "line {line} has zero length, so a constraint that needs its direction \
         cannot be solved; move an endpoint away from the other or delete the line"
    )
}

fn join_ids<T: ToString>(ids: &[T]) -> String {
    let names: Vec<String> = ids.iter().map(ToString::to_string).collect();
    format!("[{}]", names.join(", "))
}
