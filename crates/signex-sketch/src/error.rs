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
}

#[derive(Debug, Error)]
pub enum SolveError {
    #[error("did not converge after {iters} iterations")]
    DidNotConverge { iters: usize },
    #[error("over-constrained: redundant constraint {0}")]
    OverConstrained(crate::id::ConstraintId),
    #[error("solve exceeded time budget ({elapsed_ms} ms > {budget_ms} ms)")]
    Timeout { elapsed_ms: u64, budget_ms: u64 },
}
