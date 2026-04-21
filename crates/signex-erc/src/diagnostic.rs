//! [`Diagnostic`] is the internal output type for every rule. It carries a
//! [`RuleId`] in addition to the legacy [`RuleKind`] so that DSL rules
//! (Phase 2) can emit violations without needing a `RuleKind` variant.
//!
//! [`Violation`] is kept as the public API type; [`From<Diagnostic>`] converts
//! for backward compatibility with the rest of the app.

use signex_types::schematic::{Point, SelectedItem};

use crate::{RuleKind, Severity, Violation};
use crate::rule::RuleId;

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule_id: RuleId,
    pub rule_kind: RuleKind,
    pub severity: Severity,
    pub message: String,
    pub location: Point,
    pub primary: Option<SelectedItem>,
    pub peer: Option<SelectedItem>,
}

impl Diagnostic {
    /// Shorthand for built-in rules: severity comes from the kind's default.
    pub fn new(
        kind: RuleKind,
        message: impl Into<String>,
        location: Point,
    ) -> Self {
        Diagnostic {
            rule_id: RuleId::builtin(kind),
            rule_kind: kind,
            severity: kind.default_severity(),
            message: message.into(),
            location,
            primary: None,
            peer: None,
        }
    }

    /// Override the severity (used by rules that hard-code a specific level
    /// different from the kind's default, e.g. MissingPowerFlag → Info).
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_primary(mut self, primary: SelectedItem) -> Self {
        self.primary = Some(primary);
        self
    }

    pub fn with_peer(mut self, peer: SelectedItem) -> Self {
        self.peer = Some(peer);
        self
    }
}

// ---------------------------------------------------------------------------
// Conversion to legacy Violation
// ---------------------------------------------------------------------------

impl From<Diagnostic> for Violation {
    fn from(d: Diagnostic) -> Self {
        Violation {
            rule: d.rule_kind,
            severity: d.severity,
            message: d.message,
            location: d.location,
            primary: d.primary,
            peer: d.peer,
        }
    }
}
