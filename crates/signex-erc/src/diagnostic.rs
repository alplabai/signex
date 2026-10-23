//! [`Diagnostic`] is the internal output type for every rule. It carries a
//! [`RuleId`] in addition to the legacy [`RuleKind`] so that DSL rules
//! (Phase 2) can emit violations without needing a `RuleKind` variant.
//!
//! [`Violation`] is kept as the public API type; [`From<Diagnostic>`] converts
//! for backward compatibility with the rest of the app.

use signex_types::schematic::{Point, SelectedItem};

use crate::rule::RuleId;
use crate::{RuleKind, Severity, Violation};

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// Which coordinate space a [`Diagnostic::location`] lives in. Only
/// [`LocationSpace::ThisSheet`] locations can be matched against the sheet's
/// No-ERC directives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationSpace {
    /// A real point on the sheet being checked.
    ThisSheet,
    /// A placeholder: the finding has no point (net-scoped DSL rules).
    Unplaced,
    /// A point in a child sheet's own coordinate space.
    ChildSheet,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule_id: RuleId,
    pub rule_kind: RuleKind,
    pub severity: Severity,
    pub message: String,
    pub location: Point,
    /// Coordinate space of `location`; gates positional No-ERC suppression.
    pub location_space: LocationSpace,
    pub primary: Option<SelectedItem>,
    pub peer: Option<SelectedItem>,
}

impl Diagnostic {
    /// Shorthand for built-in rules: severity comes from the kind's default.
    pub fn new(kind: RuleKind, message: impl Into<String>, location: Point) -> Self {
        Diagnostic {
            rule_id: RuleId::builtin(kind),
            rule_kind: kind,
            severity: kind.default_severity(),
            message: message.into(),
            location,
            location_space: LocationSpace::ThisSheet,
            primary: None,
            peer: None,
        }
    }

    /// Mark `location` as a placeholder or a foreign-sheet point so a No-ERC
    /// directive on this sheet never matches it.
    pub fn in_space(mut self, space: LocationSpace) -> Self {
        self.location_space = space;
        self
    }

    /// Grade one finding below the kind's default. Only `BadHierSheetPin`
    /// does this; every other rule takes [`RuleKind::default_severity`].
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
            rule_id: d.rule_id,
            rule: d.rule_kind,
            severity: d.severity,
            message: d.message,
            location: d.location,
            primary: d.primary,
            peer: d.peer,
        }
    }
}
