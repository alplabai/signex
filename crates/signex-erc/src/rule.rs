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
        // Explicit per variant: the id is persisted in saved profiles and
        // project rule sets, so a label rewording must never rename it.
        let id = match kind {
            RuleKind::UnusedPin => "builtin::unused_pin",
            RuleKind::DuplicateRefDesignator => "builtin::duplicate_reference_designator",
            RuleKind::HierPortDisconnected => "builtin::hierarchical_port_disconnected",
            RuleKind::DanglingWire => "builtin::dangling_wire_endpoint",
            RuleKind::NetLabelConflict => "builtin::net_label_conflict",
            RuleKind::OrphanLabel => "builtin::orphan_label",
            RuleKind::BusBitWidthMismatch => "builtin::bus_bit-width_mismatch",
            RuleKind::BadHierSheetPin => "builtin::bad_hierarchical_sheet_pin",
            RuleKind::MissingPowerFlag => "builtin::missing_power_flag",
            RuleKind::PowerPortShort => "builtin::power_port_short",
            RuleKind::SymbolOutsideSheet => "builtin::symbol_outside_sheet_boundary",
            RuleKind::AmbiguousLabelAnchor => "builtin::ambiguous_label_anchor",
            RuleKind::PinTypeConflict => "builtin::pin_type_conflict",
            RuleKind::PinsTouchNotConnected => "builtin::pins_touch_but_are_not_connected",
            RuleKind::BusEntryUnlabeled => "builtin::bus_entry_without_a_member_label",
            RuleKind::BusMemberNotInBus => "builtin::label_is_not_a_member_of_its_bus",
            RuleKind::BusUnnamed => "builtin::bus_name_missing_or_invalid",
            RuleKind::BusNameConflict => "builtin::bus_names_disagree",
            RuleKind::BusLabelOnWire => "builtin::bus_label_on_a_wire",
            RuleKind::NetMultipleNames => "builtin::net_has_multiple_names",
            RuleKind::LocalLabelNameOnOtherSheets => "builtin::local_label_name_on_other_sheets",
            RuleKind::PartWithoutFootprint => "builtin::part_without_footprint",
            RuleKind::PartWithoutValue => "builtin::part_without_value",
            RuleKind::PartWithoutMpn => "builtin::part_without_mpn",
            RuleKind::UnannotatedDesignator => "builtin::unannotated_designator",
            RuleKind::SinglePinNet => "builtin::single_pin_net",
            RuleKind::NetWithNoDriver => "builtin::net_with_no_driver",
            RuleKind::PowerInputNotDriven => "builtin::power_input_not_driven",
            RuleKind::UnitsNotPlaced => "builtin::part_units_not_placed",
            RuleKind::OffGrid => "builtin::off_grid_pin_or_wire",
            RuleKind::LabelsOnlyNet => "builtin::net_carries_only_labels",
        };
        RuleId(id.to_string())
    }

    pub fn user(name: &str) -> Self {
        RuleId(format!("user::{name}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The built-in rule this id names, or `None` for a `user::` id (or any
    /// id no built-in produces).
    pub fn builtin_kind(&self) -> Option<RuleKind> {
        RuleKind::ALL
            .into_iter()
            .find(|kind| RuleId::builtin(*kind) == *self)
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
/// The engine does not filter on `applicability` or `scope`: the DSL
/// validator rejects any non-`All` applicability and any non-sheet scope.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every built-in id string is persisted in saved profiles and project
    /// rule sets, so it must never follow a label rewording.
    const PINNED_IDS: [(RuleKind, &str); 31] = [
        (RuleKind::UnusedPin, "builtin::unused_pin"),
        (
            RuleKind::DuplicateRefDesignator,
            "builtin::duplicate_reference_designator",
        ),
        (
            RuleKind::HierPortDisconnected,
            "builtin::hierarchical_port_disconnected",
        ),
        (RuleKind::DanglingWire, "builtin::dangling_wire_endpoint"),
        (RuleKind::NetLabelConflict, "builtin::net_label_conflict"),
        (RuleKind::OrphanLabel, "builtin::orphan_label"),
        (
            RuleKind::BusBitWidthMismatch,
            "builtin::bus_bit-width_mismatch",
        ),
        (
            RuleKind::BadHierSheetPin,
            "builtin::bad_hierarchical_sheet_pin",
        ),
        (RuleKind::MissingPowerFlag, "builtin::missing_power_flag"),
        (RuleKind::PowerPortShort, "builtin::power_port_short"),
        (
            RuleKind::SymbolOutsideSheet,
            "builtin::symbol_outside_sheet_boundary",
        ),
        (
            RuleKind::AmbiguousLabelAnchor,
            "builtin::ambiguous_label_anchor",
        ),
        (RuleKind::PinTypeConflict, "builtin::pin_type_conflict"),
        (
            RuleKind::PinsTouchNotConnected,
            "builtin::pins_touch_but_are_not_connected",
        ),
        (
            RuleKind::BusEntryUnlabeled,
            "builtin::bus_entry_without_a_member_label",
        ),
        (
            RuleKind::BusMemberNotInBus,
            "builtin::label_is_not_a_member_of_its_bus",
        ),
        (RuleKind::BusUnnamed, "builtin::bus_name_missing_or_invalid"),
        (RuleKind::BusNameConflict, "builtin::bus_names_disagree"),
        (RuleKind::BusLabelOnWire, "builtin::bus_label_on_a_wire"),
        (
            RuleKind::NetMultipleNames,
            "builtin::net_has_multiple_names",
        ),
        (
            RuleKind::LocalLabelNameOnOtherSheets,
            "builtin::local_label_name_on_other_sheets",
        ),
        (
            RuleKind::PartWithoutFootprint,
            "builtin::part_without_footprint",
        ),
        (RuleKind::PartWithoutValue, "builtin::part_without_value"),
        (RuleKind::PartWithoutMpn, "builtin::part_without_mpn"),
        (
            RuleKind::UnannotatedDesignator,
            "builtin::unannotated_designator",
        ),
        (RuleKind::SinglePinNet, "builtin::single_pin_net"),
        (RuleKind::NetWithNoDriver, "builtin::net_with_no_driver"),
        (
            RuleKind::PowerInputNotDriven,
            "builtin::power_input_not_driven",
        ),
        (RuleKind::UnitsNotPlaced, "builtin::part_units_not_placed"),
        (RuleKind::OffGrid, "builtin::off_grid_pin_or_wire"),
        (RuleKind::LabelsOnlyNet, "builtin::net_carries_only_labels"),
    ];

    #[test]
    fn builtin_ids_are_pinned_for_every_rule() {
        assert_eq!(PINNED_IDS.len(), RuleKind::ALL.len());
        for kind in RuleKind::ALL {
            let pinned = PINNED_IDS
                .iter()
                .find(|(k, _)| *k == kind)
                .unwrap_or_else(|| panic!("{kind:?} has no pinned id"));
            assert_eq!(RuleId::builtin(kind).as_str(), pinned.1, "{kind:?}");
            assert_eq!(RuleId::builtin(kind).builtin_kind(), Some(kind));
        }
    }
}
