//! Abstract syntax tree produced by the DSL parser.
//! These types are purely structural — no validation or compilation done yet.

// ---------------------------------------------------------------------------
// Top-level rule declaration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RuleAst {
    pub id: String,
    pub applicability: ApplicabilityAst,
    pub target: TargetKind,
    pub scope: ScopeKind,
    pub when: ExprAst,
    pub severity: SeverityKind,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Applicability
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ApplicabilityAst {
    All,
    TaggedSheets(Vec<String>),
    NamedSheets(Vec<String>),
}

// ---------------------------------------------------------------------------
// Target / scope / severity (enumerated terminals)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Net,
    Pin,
    Component,
    Sheet,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Net => "net",
            TargetKind::Pin => "pin",
            TargetKind::Component => "component",
            TargetKind::Sheet => "sheet",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Local,
    Sheet,
    Hierarchical,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityKind {
    Error,
    Warning,
    Info,
}

// ---------------------------------------------------------------------------
// Expression AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ExprAst {
    And(Box<ExprAst>, Box<ExprAst>),
    Or(Box<ExprAst>, Box<ExprAst>),
    Not(Box<ExprAst>),
    /// `name(arg, …)` — built-in predicate helper.
    /// Examples: `has_driver()`, `has_pin_kind(OpenDrain)`.
    HelperCall {
        name: String,
        args: Vec<LiteralAst>,
    },
    /// `object.field == value` or `object.field != value`.
    /// Examples: `pin.kind == Input`, `pin.connected == false`.
    FieldCmp {
        field: FieldExprAst,
        op: CmpOp,
        value: LiteralAst,
    },
    /// `object.field matches "pattern"`.
    /// Example: `net.name matches "^I2C_"`.
    FieldMatches {
        field: FieldExprAst,
        pattern: String,
    },
}

// ---------------------------------------------------------------------------
// Field expression (LHS of a comparison)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FieldExprAst {
    /// `pin.kind`, `net.name`, `net.class`.
    Access { object: String, field: String },
    /// `component.attr("class")`.
    MethodCall {
        object: String,
        method: String,
        args: Vec<LiteralAst>,
    },
}

// ---------------------------------------------------------------------------
// Comparison operator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
}

// ---------------------------------------------------------------------------
// Literal values (arguments and RHS of comparisons)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum LiteralAst {
    /// A double-quoted string: `"I2C"`.
    Str(String),
    /// An unquoted identifier used as a value: `Input`, `OpenDrain`.
    Ident(String),
    /// Boolean literal: `true` / `false`.
    Bool(bool),
}

impl LiteralAst {
    /// Returns the string representation of the literal for comparisons.
    pub fn as_str_value(&self) -> &str {
        match self {
            LiteralAst::Str(s) | LiteralAst::Ident(s) => s.as_str(),
            LiteralAst::Bool(b) => {
                if *b {
                    "true"
                } else {
                    "false"
                }
            }
        }
    }
}
