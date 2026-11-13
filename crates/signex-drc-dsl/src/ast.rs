//! What the parser produces: the rule text's structure with the byte span
//! of each part. Nothing here is checked; words the language may grow
//! (targets, objects, scope subjects, fields, units) stay plain [`Word`]s
//! so the validator, not the parser, names what is wrong with them.

use std::ops::Range;

/// Byte offsets into the rule text.
pub type Span = Range<usize>;

/// An identifier or a quoted string, and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleAst {
    pub name: Word,
    /// `clearance`, `track` or `via` in v1.
    pub target: Word,
    /// `between <object> and <object>`.
    pub pair: Option<[Word; 2]>,
    /// `when <cond> and <cond> ...`; empty for a board-wide rule. A region
    /// scope (v2) is one more subject here, not a new clause.
    pub when: Vec<CondAst>,
    /// `require <limit> and <limit> ...`.
    pub require: Vec<LimitAst>,
    pub severity: SeverityAst,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityAst {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondOp {
    /// `==`
    Is,
    /// `matches`
    Matches,
}

/// `<subject> == <value>` or `<subject> matches <value>`.
#[derive(Debug, Clone, PartialEq)]
pub struct CondAst {
    pub subject: Word,
    pub op: CondOp,
    pub value: Word,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    /// `>=`, a minimum.
    AtLeast,
    /// `<=`, a maximum.
    AtMost,
}

/// `<field> >= <number><unit>` or `<field> <= <number><unit>`.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitAst {
    pub field: Word,
    pub bound: Bound,
    /// The number as written.
    pub number: Word,
    /// Written right after the number; `None` when missing.
    pub unit: Option<Word>,
}
