//! Electrical Rules Check — runs on a [`SchematicRenderSnapshot`] and returns
//! a list of [`Violation`]s flagged per [`RuleKind`]. v0.7 scope: 11 rule
//! kinds with Phase A shipping the three cheapest ones (hier port dangling,
//! duplicate reference, unused pin). Severity per rule is configurable via
//! [`Severity`]; defaults follow the Altium convention.

use serde::{Deserialize, Serialize};
use signex_render::schematic::SchematicRenderSnapshot;
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

mod rules;

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
        }
    }

    /// Default severity. Users can override per-rule in preferences (v0.7.1).
    pub fn default_severity(self) -> Severity {
        match self {
            // Hard errors — the netlist will be wrong if shipped.
            RuleKind::DuplicateRefDesignator
            | RuleKind::BusBitWidthMismatch
            | RuleKind::BadHierSheetPin
            | RuleKind::PowerPortShort => Severity::Error,
            // Warnings — likely user intent mistakes.
            RuleKind::UnusedPin
            | RuleKind::HierPortDisconnected
            | RuleKind::DanglingWire
            | RuleKind::NetLabelConflict
            | RuleKind::OrphanLabel
            | RuleKind::MissingPowerFlag => Severity::Warning,
            // Info — style hints.
            RuleKind::SymbolOutsideSheet => Severity::Info,
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
/// mm (same coordinate system as the schematic); the app uses it to center
/// the canvas when the user clicks the message.
#[derive(Debug, Clone)]
pub struct Violation {
    pub rule: RuleKind,
    pub severity: Severity,
    pub message: String,
    pub location: Point,
    /// The primary object the violation refers to. The app uses this to
    /// replace the current selection on click-to-zoom.
    pub primary: Option<SelectedItem>,
    /// Optional peer — the "other" object in two-object rules (duplicate
    /// designator, power-port short).
    pub peer: Option<SelectedItem>,
}

/// Run every enabled rule against the snapshot. Returns a flat list of
/// violations in rule order.
pub fn run(snapshot: &SchematicRenderSnapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    rules::unused_pin(snapshot, &mut out);
    rules::duplicate_ref_designator(snapshot, &mut out);
    rules::hier_port_disconnected(snapshot, &mut out);
    rules::dangling_wire(snapshot, &mut out);
    rules::net_label_conflict(snapshot, &mut out);
    rules::orphan_label(snapshot, &mut out);
    rules::bus_bit_width_mismatch(snapshot, &mut out);
    rules::bad_hier_sheet_pin(snapshot, &mut out);
    rules::missing_power_flag(snapshot, &mut out);
    rules::power_port_short(snapshot, &mut out);
    rules::symbol_outside_sheet(snapshot, &mut out);
    out
}

/// Helper for rules to build a [`SelectedItem`] when they only have a uuid.
pub(crate) fn sel(uuid: uuid::Uuid, kind: SelectedKind) -> SelectedItem {
    SelectedItem::new(uuid, kind)
}
