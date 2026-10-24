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
    /// Phase 5.4 dispatcher — a non-Timeout failure from the LM
    /// solver. Timeout is consumed silently by the dispatcher (it
    /// feeds the auto-pause state machine instead).
    #[error("solve failed: {0}")]
    SolveFailed(#[from] SolveError),
}

#[derive(Debug, Error)]
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
    #[error("solve exceeded time budget ({elapsed_ms} ms > {budget_ms} ms)")]
    Timeout { elapsed_ms: u64, budget_ms: u64 },
    /// LO-12: covers genuine internal failures (LU dimension mismatch,
    /// lu.solve back-sub error, total_residual error on initial pass).
    /// Reserve `OverConstrained` for the semantic case where a real
    /// constraint id can be named.
    #[error("solver internal error: {0}")]
    Internal(String),
}

fn join_ids<T: ToString>(ids: &[T]) -> String {
    let names: Vec<String> = ids.iter().map(ToString::to_string).collect();
    format!("[{}]", names.join(", "))
}
