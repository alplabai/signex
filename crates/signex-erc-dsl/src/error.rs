//! Error types for the ERC DSL pipeline.

use thiserror::Error;

/// A span (byte offsets) inside the DSL source string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

/// All errors the DSL pipeline can produce.
#[derive(Debug, Error)]
pub enum DslError {
    // ── Parser errors ──────────────────────────────────────────────────────
    #[error("parse error at {0}:{1}: {2}")]
    Parse(usize, usize, String),

    // ── Validator errors ───────────────────────────────────────────────────
    #[error("rule '{rule}': predicate '{predicate}' is not valid for target '{target}'")]
    InvalidPredicateForTarget {
        rule: String,
        predicate: String,
        target: String,
    },

    #[error("rule '{rule}': unknown predicate '{predicate}'")]
    UnknownPredicate { rule: String, predicate: String },

    #[error("rule '{rule}': unknown field '{object}.{field}'")]
    UnknownField {
        rule: String,
        object: String,
        field: String,
    },

    #[error("rule '{rule}': '{predicate}' expects {expected} argument(s), got {got}")]
    WrongArgCount {
        rule: String,
        predicate: String,
        expected: usize,
        got: usize,
    },

    // ── Compiler errors ────────────────────────────────────────────────────
    #[error("rule '{rule}': invalid regex pattern '{pattern}': {source}")]
    InvalidRegex {
        rule: String,
        pattern: String,
        #[source]
        source: regex::Error,
    },
}
