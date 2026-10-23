//! Electrical Rules Check. Runs on a [`SchematicSheet`] and returns a
//! list of [`Violation`]s. Internally the sheet is projected into an
//! [`ErcContext`] first, so rule logic stays independent from renderer internals.
//!
//! # Architecture
//!
//! ```text
//! SchematicSheet
//!        ↓  (projection)
//!    ErcContext
//!        ↓  (engine::run_all)
//!  Vec<Diagnostic>
//!        ↓  (From<Diagnostic>)
//!  Vec<Violation>   ← public API
//! ```
//!
//! ERC reads from `signex_types::SchematicSheet` directly.

use serde::{Deserialize, Serialize};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};

pub mod context;
pub mod diagnostic;
pub mod engine;
#[cfg(test)]
mod fixtures;
mod overrides;
pub mod rule;
mod rules;

pub use context::ErcContext;
pub use diagnostic::{Diagnostic, LocationSpace};
pub use overrides::apply_overrides;
pub use rule::{AnalysisScope, Applicability, RuleDefinition, RuleId, RuleTarget};

/// What kind of violation this is. Stable identifier that maps to a severity
/// in the user's configuration. Ordered by the Altium ERC matrix conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleKind {
    /// A pin has no connected wire, no NC, and no matching pin on another part.
    UnusedPin,
    /// Two symbols share the same non-blank reference designator.
    DuplicateRefDesignator,
    /// A hierarchical port / label has no connected wire or bus segment.
    HierPortDisconnected,
    /// A wire endpoint dangles — no matching pin, junction, label, or port.
    DanglingWire,
    /// A net name appears on incompatible label types (e.g. a local label and
    /// a global label both carrying different nets).
    NetLabelConflict,
    /// A label sits in free space — neither on a wire endpoint nor a pin.
    OrphanLabel,
    /// A bus segment connects two buses whose bit-widths differ.
    BusBitWidthMismatch,
    /// A sheet symbol's port pin doesn't match a label/port on the child sheet.
    BadHierSheetPin,
    /// A net containing a power pin lacks a power flag (PWR_FLAG).
    MissingPowerFlag,
    /// Two power ports with incompatible nets are shorted together.
    PowerPortShort,
    /// A symbol or wire sits outside the active sheet's page boundary.
    SymbolOutsideSheet,
    /// A label sits where two or more wires cross with no junction. The
    /// netlist anchors it to exactly one of them by a deterministic tiebreak
    /// (issue #402) — a defensible runtime answer, but not one the user can
    /// predict from the geometry, so it is surfaced rather than assumed.
    AmbiguousLabelAnchor,
}

impl RuleKind {
    /// Human-readable rule name for the Messages panel and preferences UI.
    pub fn label(self) -> &'static str {
        match self {
            RuleKind::UnusedPin => "Unused pin",
            RuleKind::DuplicateRefDesignator => "Duplicate reference designator",
            RuleKind::HierPortDisconnected => "Hierarchical port disconnected",
            RuleKind::DanglingWire => "Dangling wire endpoint",
            RuleKind::NetLabelConflict => "Net label conflict",
            RuleKind::OrphanLabel => "Orphan label",
            RuleKind::BusBitWidthMismatch => "Bus bit-width mismatch",
            RuleKind::BadHierSheetPin => "Bad hierarchical sheet pin",
            RuleKind::MissingPowerFlag => "Missing power flag",
            RuleKind::PowerPortShort => "Power port short",
            RuleKind::SymbolOutsideSheet => "Symbol outside sheet boundary",
            RuleKind::AmbiguousLabelAnchor => "Ambiguous label anchor",
        }
    }

    /// Every built-in rule, in the order the ERC dialog lists them.
    pub const ALL: [RuleKind; 12] = [
        RuleKind::UnusedPin,
        RuleKind::DuplicateRefDesignator,
        RuleKind::HierPortDisconnected,
        RuleKind::DanglingWire,
        RuleKind::NetLabelConflict,
        RuleKind::OrphanLabel,
        RuleKind::BusBitWidthMismatch,
        RuleKind::BadHierSheetPin,
        RuleKind::MissingPowerFlag,
        RuleKind::PowerPortShort,
        RuleKind::SymbolOutsideSheet,
        RuleKind::AmbiguousLabelAnchor,
    ];

    /// The one source of truth for a rule's severity: [`Diagnostic::new`]
    /// stamps it on every diagnostic and the ERC dialog highlights it.
    /// `BadHierSheetPin` is the sole rule that grades individual findings
    /// below this level. Users override per rule via
    /// `ui_state.erc_severity_override`; see [`apply_overrides`].
    pub fn default_severity(self) -> Severity {
        match self {
            RuleKind::DuplicateRefDesignator
            | RuleKind::BusBitWidthMismatch
            | RuleKind::BadHierSheetPin
            | RuleKind::PowerPortShort => Severity::Error,
            RuleKind::UnusedPin
            | RuleKind::HierPortDisconnected
            | RuleKind::DanglingWire
            | RuleKind::NetLabelConflict
            | RuleKind::OrphanLabel
            | RuleKind::AmbiguousLabelAnchor => Severity::Warning,
            RuleKind::MissingPowerFlag | RuleKind::SymbolOutsideSheet => Severity::Info,
        }
    }
}

/// Severity level — maps to colours and sort order in the Messages panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    /// Rule is disabled — emit nothing.
    Off,
}

/// A concrete violation emitted by a rule run. `location` is in world-space
/// mm; the app uses it to centre the canvas when the user clicks the message.
#[derive(Debug, Clone)]
pub struct Violation {
    /// Which rule fired. Authoritative for identity: built-ins are
    /// `builtin::<slug>`, DSL rules `user::<name>`.
    pub rule_id: RuleId,
    /// Built-in kind this violation is filed under. For a DSL violation it is
    /// only the compiler's placeholder (see `fallback_kind` in
    /// `signex-erc-dsl`) — nothing may read it to decide labels, overrides or
    /// quick fixes; use [`Violation::builtin_kind`] instead.
    pub rule: RuleKind,
    pub severity: Severity,
    pub message: String,
    pub location: Point,
    /// The primary object the violation refers to.
    pub primary: Option<SelectedItem>,
    /// Optional peer — the "other" object in two-object rules.
    pub peer: Option<SelectedItem>,
}

impl Violation {
    /// A violation with no `primary` / `peer` object attached. For a `user::`
    /// rule `rule` is only a placeholder; `rule_id` carries identity.
    pub fn new(
        rule_id: RuleId,
        rule: RuleKind,
        severity: Severity,
        message: impl Into<String>,
        location: Point,
    ) -> Self {
        Self {
            rule_id,
            rule,
            severity,
            message: message.into(),
            location,
            primary: None,
            peer: None,
        }
    }

    /// The built-in rule that produced this violation, or `None` for a DSL
    /// (`user::`) rule.
    pub fn builtin_kind(&self) -> Option<RuleKind> {
        self.rule_id.builtin_kind()
    }

    /// Name for the Messages panel: the built-in's label, or a DSL rule's own
    /// id.
    pub fn display_name(&self) -> String {
        match self.builtin_kind() {
            Some(kind) => kind.label().to_string(),
            None => self.rule_id.to_string(),
        }
    }
}

/// Run every enabled rule against the snapshot. Returns a flat list of
/// violations in rule order.
pub fn run(snapshot: &SchematicSheet) -> Vec<Violation> {
    let ctx = ErcContext::from_snapshot(snapshot);
    engine::run_all(&ctx)
        .into_iter()
        .map(Violation::from)
        .collect()
}

/// Run built-in ERC rules plus caller-provided DSL evaluator functions.
pub fn run_with_dsl(snapshot: &SchematicSheet, dsl_rules: &[engine::EvalFn]) -> Vec<Violation> {
    let ctx = ErcContext::from_snapshot(snapshot);
    engine::run_all_with_dsl(&ctx, dsl_rules)
        .into_iter()
        .map(Violation::from)
        .collect()
}

/// Run ERC for a schematic in the context of a whole project. Cross-sheet
/// rules consult `resolved` — THIS sheet's own `ChildSheet.filename ->
/// SheetKey` submap — resolved through the shared `sheets` table. Pass an
/// empty `resolved` map for top-only runs.
///
/// `resolved` must be the caller's per-parent submap for `snapshot`, not a
/// single project-wide map: keying it that way is what stops two parents in
/// different directories that reference a child by the same filename string
/// from resolving to the wrong file (#466), and is what keeps this in step
/// with `signex_net::build_project_netlist`, which takes the same shape.
pub fn run_with_project(
    snapshot: &SchematicSheet,
    resolved: &std::collections::HashMap<String, signex_net::SheetKey>,
    sheets: &std::collections::HashMap<signex_net::SheetKey, SchematicSheet>,
) -> Vec<Violation> {
    let ctx = ErcContext::from_snapshot_with_children(snapshot, resolved, sheets);
    engine::run_all(&ctx)
        .into_iter()
        .map(Violation::from)
        .collect()
}

/// Run project-scoped ERC with built-in rules plus caller-provided DSL rules.
/// See [`run_with_project`] for what `resolved` / `sheets` must be.
pub fn run_with_project_and_dsl(
    snapshot: &SchematicSheet,
    resolved: &std::collections::HashMap<String, signex_net::SheetKey>,
    sheets: &std::collections::HashMap<signex_net::SheetKey, SchematicSheet>,
    dsl_rules: &[engine::EvalFn],
) -> Vec<Violation> {
    let ctx = ErcContext::from_snapshot_with_children(snapshot, resolved, sheets);
    engine::run_all_with_dsl(&ctx, dsl_rules)
        .into_iter()
        .map(Violation::from)
        .collect()
}

/// Helper for rules to build a [`SelectedItem`] when they only have a uuid.
pub(crate) fn sel(uuid: uuid::Uuid, kind: SelectedKind) -> SelectedItem {
    SelectedItem::new(uuid, kind)
}
