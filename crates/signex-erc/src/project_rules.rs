//! Resolve a project's stored rule set (`.snxprj` `rules.erc`, #1085) into the
//! override maps the ERC run consumes.
//!
//! `signex-types` stores rule ids and severities as plain strings so the
//! project file can carry ids from a newer build. This module owns the
//! vocabulary: it maps those strings to [`RuleKind`] / [`Severity`] / pin
//! matrix cells, and reports the ids it could not map instead of dropping
//! them silently. Pin matrix keys are `"<row axis>/<column axis>"` using the
//! stable axis names in [`PinAxis::key`].

use std::collections::HashMap;

use signex_types::project::ErcRules;

use crate::{PinAxis, RuleKind, Severity};

impl RuleKind {
    /// Stable string id written to preferences and to the `.snxprj`.
    pub const fn key(self) -> &'static str {
        match self {
            RuleKind::UnusedPin => "unused_pin",
            RuleKind::DuplicateRefDesignator => "duplicate_ref_designator",
            RuleKind::HierPortDisconnected => "hier_port_disconnected",
            RuleKind::DanglingWire => "dangling_wire",
            RuleKind::NetLabelConflict => "net_label_conflict",
            RuleKind::OrphanLabel => "orphan_label",
            RuleKind::BusBitWidthMismatch => "bus_bit_width_mismatch",
            RuleKind::BadHierSheetPin => "bad_hier_sheet_pin",
            RuleKind::MissingPowerFlag => "missing_power_flag",
            RuleKind::PowerPortShort => "power_port_short",
            RuleKind::SymbolOutsideSheet => "symbol_outside_sheet",
            RuleKind::AmbiguousLabelAnchor => "ambiguous_label_anchor",
            RuleKind::PinTypeConflict => "pin_type_conflict",
            RuleKind::PinsTouchNotConnected => "pins_touch_not_connected",
            RuleKind::BusEntryUnlabeled => "bus_entry_unlabeled",
            RuleKind::BusMemberNotInBus => "bus_member_not_in_bus",
            RuleKind::BusUnnamed => "bus_unnamed",
            RuleKind::BusNameConflict => "bus_name_conflict",
            RuleKind::BusLabelOnWire => "bus_label_on_wire",
            RuleKind::NetMultipleNames => "net_multiple_names",
            RuleKind::LocalLabelNameOnOtherSheets => "local_label_name_on_other_sheets",
            RuleKind::PartWithoutFootprint => "part_without_footprint",
            RuleKind::PartWithoutValue => "part_without_value",
            RuleKind::PartWithoutMpn => "part_without_mpn",
            RuleKind::UnannotatedDesignator => "unannotated_designator",
            RuleKind::SinglePinNet => "single_pin_net",
            RuleKind::NetWithNoDriver => "net_with_no_driver",
            RuleKind::PowerInputNotDriven => "power_input_not_driven",
            RuleKind::UnitsNotPlaced => "units_not_placed",
            RuleKind::OffGrid => "off_grid",
            RuleKind::LabelsOnlyNet => "labels_only_net",
            RuleKind::SheetPinUnconnected => "sheet_pin_unconnected",
            RuleKind::SheetPinDirectionConflict => "sheet_pin_direction_conflict",
        }
    }

    /// The rule a stable id names, or `None` for an id this build lacks.
    pub fn from_key(key: &str) -> Option<RuleKind> {
        RuleKind::ALL.into_iter().find(|rule| rule.key() == key)
    }
}

impl Severity {
    /// Stable string id: `error`, `warning`, `info` or `off`.
    pub const fn key(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Off => "off",
        }
    }

    /// The severity a stable id names, or `None` for an unknown id.
    pub fn from_key(key: &str) -> Option<Severity> {
        [
            Severity::Error,
            Severity::Warning,
            Severity::Info,
            Severity::Off,
        ]
        .into_iter()
        .find(|sev| sev.key() == key)
    }
}

impl PinAxis {
    /// Stable string id used in pin matrix keys.
    pub const fn key(self) -> &'static str {
        match self {
            PinAxis::Input => "input",
            PinAxis::Output => "output",
            PinAxis::Bidir => "bidir",
            PinAxis::PowerIn => "power_in",
            PinAxis::PowerOut => "power_out",
            PinAxis::NotConnected => "not_connected",
        }
    }

    fn from_key(key: &str) -> Option<PinAxis> {
        PinAxis::ALL.into_iter().find(|axis| axis.key() == key)
    }
}

/// The pin matrix key of the `(row, col)` cell, or `None` outside the grid.
pub fn pin_matrix_key(row: u8, col: u8) -> Option<String> {
    let axis = |i: u8| PinAxis::ALL.get(usize::from(i)).copied();
    Some(format!("{}/{}", axis(row)?.key(), axis(col)?.key()))
}

fn parse_pin_matrix_key(key: &str) -> Option<(u8, u8)> {
    let (row, col) = key.split_once('/')?;
    Some((
        PinAxis::from_key(row)?.index() as u8,
        PinAxis::from_key(col)?.index() as u8,
    ))
}

/// A project's ERC rules in the form the run consumes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedRules {
    /// Per-rule severities. A rule the project does not list is absent, so it
    /// runs at its built-in default.
    pub severity: HashMap<RuleKind, Severity>,
    /// Pin matrix cells that differ from the factory matrix.
    pub pin_matrix: HashMap<(u8, u8), Severity>,
    /// Entries this build could not map (a rule id or axis from a newer
    /// build, or a bad severity). They stay in the file; the run ignores them.
    pub unrecognised: Vec<String>,
    /// Built-in rules the project does not list, e.g. added by a newer build
    /// than the one that wrote the file.
    pub missing: Vec<RuleKind>,
}

/// Resolve `rules` into override maps. Never consults the machine's
/// preferences: a project with a rule set gives the same result everywhere.
pub fn resolve(rules: &ErcRules) -> ResolvedRules {
    let mut out = ResolvedRules::default();
    for (id, sev) in &rules.severity {
        match (RuleKind::from_key(id), Severity::from_key(sev)) {
            (Some(rule), Some(sev)) => {
                out.severity.insert(rule, sev);
            }
            _ => out.unrecognised.push(format!("severity {id} = {sev}")),
        }
    }
    for (id, sev) in &rules.pin_matrix {
        match (parse_pin_matrix_key(id), Severity::from_key(sev)) {
            (Some(cell), Some(sev)) => {
                out.pin_matrix.insert(cell, sev);
            }
            _ => out.unrecognised.push(format!("pin_matrix {id} = {sev}")),
        }
    }
    out.missing = RuleKind::ALL
        .into_iter()
        .filter(|rule| !out.severity.contains_key(rule))
        .collect();
    out
}

mod edit;
pub mod profile;
pub use edit::{cycle_pin_cell, seed_from_overrides, set_severity};

#[cfg(test)]
mod tests;
