//! Rule metadata types. Describe *what* a rule checks and *where* it applies.
//! The built-in rules are registered in [`crate::engine`]; DSL rules will
//! produce [`RuleDefinition`] values at compile time (Phase 2).

use crate::{RuleKind, Severity};

// ---------------------------------------------------------------------------
// RuleId
// ---------------------------------------------------------------------------

/// Stable, namespaced rule identifier.
/// Built-in rules use the `"builtin::"` prefix; DSL rules use `"user::"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleId(pub String);

impl RuleId {
    pub fn builtin(kind: RuleKind) -> Self {
        let slug = kind.label().to_ascii_lowercase().replace(' ', "_");
        RuleId(format!("builtin::{slug}"))
    }

    pub fn user(name: &str) -> Self {
        RuleId(format!("user::{name}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Target
// ---------------------------------------------------------------------------

/// The schematic element kind that a rule evaluates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTarget {
    Net,
    Pin,
    Component,
    Sheet,
    /// Rule evaluates multiple element kinds (e.g. dangling wire touches
    /// pins, wires, labels — it doesn't fit a single target).
    Any,
}

// ---------------------------------------------------------------------------
// AnalysisScope
// ---------------------------------------------------------------------------

/// How far across the design hierarchy a rule looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisScope {
    /// Only the immediate sheet context (single snapshot).
    Local,
    /// All elements on one sheet.
    Sheet,
    /// Sheet + all directly attached child sheets.
    Hierarchical,
    /// Every sheet in the project.
    Global,
}

// ---------------------------------------------------------------------------
// Applicability
// ---------------------------------------------------------------------------

/// Which sheets a rule should run on. Defaults to [`Applicability::All`].
#[derive(Debug, Clone, Default)]
pub enum Applicability {
    /// Run on every sheet (default).
    #[default]
    All,
    /// Run only on sheets whose name is exactly one of these strings.
    ExactSheets(Vec<String>),
    /// Run only on sheets that carry all listed tags.
    TaggedSheets(Vec<String>),
    /// Run only on sheets whose name matches this regex pattern.
    PatternSheets(String),
}

// ---------------------------------------------------------------------------
// RuleDefinition
// ---------------------------------------------------------------------------

/// Metadata record that describes a rule — both built-in and DSL-compiled.
/// The rule engine uses this to filter applicability and scope before calling
/// the rule's evaluation function.
#[derive(Debug, Clone)]
pub struct RuleDefinition {
    pub id: RuleId,
    pub name: String,
    pub description: String,
    pub target: RuleTarget,
    pub scope: AnalysisScope,
    pub applicability: Applicability,
    pub default_severity: Severity,
}

impl RuleDefinition {
    /// Convenience constructor for built-in rules.
    pub fn builtin(kind: RuleKind) -> Self {
        RuleDefinition {
            id: RuleId::builtin(kind),
            name: kind.label().to_string(),
            description: String::new(),
            target: RuleTarget::Any,
            scope: AnalysisScope::Sheet,
            applicability: Applicability::All,
            default_severity: kind.default_severity(),
        }
    }
}
