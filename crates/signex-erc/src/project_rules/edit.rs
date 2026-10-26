//! Writing a project's ERC rule set (#1085): seeding it from the machine's
//! preferences and applying single edits. Pure functions over [`ErcRules`], so
//! the app can snapshot the section before an edit and restore it on undo.

use std::collections::HashMap;

use signex_types::project::ErcRules;

use super::{pin_matrix_key, resolve};
use crate::pin_matrix::baseline_cell;
use crate::{RuleKind, Severity};

impl Severity {
    /// The next level of the pin-matrix cell cycle: Error, Warning, Info, Off,
    /// then back to Error.
    pub const fn cycled(self) -> Severity {
        match self {
            Severity::Error => Severity::Warning,
            Severity::Warning => Severity::Info,
            Severity::Info => Severity::Off,
            Severity::Off => Severity::Error,
        }
    }
}

/// A full snapshot of the given machine settings: every rule id known to this
/// build gets an entry (the override when there is one, the built-in default
/// otherwise) and every pin matrix cell that differs from the factory matrix
/// is listed.
pub fn seed_from_overrides(
    severity: &HashMap<RuleKind, Severity>,
    pin_matrix: &HashMap<(u8, u8), Severity>,
) -> ErcRules {
    let mut rules = ErcRules::default();
    for rule in RuleKind::ALL {
        let sev = severity
            .get(&rule)
            .copied()
            .map_or_else(|| rule.default_severity(), |s| rule.normalize_override(s));
        rules
            .severity
            .insert(rule.key().to_string(), sev.key().to_string());
    }
    for (&(row, col), &sev) in pin_matrix {
        if sev == baseline_cell(row, col) {
            continue;
        }
        if let Some(key) = pin_matrix_key(row, col) {
            rules.pin_matrix.insert(key, sev.key().to_string());
        }
    }
    rules
}

/// Set one rule's severity. An on/off-only rule keeps just the on/off choice.
/// Keys written by a newer build are left alone.
pub fn set_severity(rules: &mut ErcRules, rule: RuleKind, severity: Severity) {
    let chosen = rule.normalize_override(severity);
    rules
        .severity
        .insert(rule.key().to_string(), chosen.key().to_string());
}

/// Cycle one pin matrix cell and its mirror (a pair of pin types is
/// unordered). A level equal to the factory matrix drops the entry.
pub fn cycle_pin_cell(rules: &mut ErcRules, row: u8, col: u8) {
    let current = resolve(rules)
        .pin_matrix
        .get(&(row, col))
        .copied()
        .unwrap_or_else(|| baseline_cell(row, col));
    let next = current.cycled();
    for (r, c) in [(row, col), (col, row)] {
        let Some(key) = pin_matrix_key(r, c) else {
            continue;
        };
        if next == baseline_cell(r, c) {
            rules.pin_matrix.remove(&key);
        } else {
            rules.pin_matrix.insert(key, next.key().to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_lists_every_rule_with_the_override_or_default() {
        let overrides = HashMap::from([(RuleKind::DanglingWire, Severity::Off)]);
        let rules = seed_from_overrides(&overrides, &HashMap::new());
        assert_eq!(rules.severity.len(), RuleKind::ALL.len());
        assert_eq!(rules.severity["dangling_wire"], "off");
        assert_eq!(
            rules.severity["unused_pin"],
            RuleKind::UnusedPin.default_severity().key()
        );
        assert!(resolve(&rules).missing.is_empty());
    }

    #[test]
    fn seed_keeps_only_pin_cells_that_differ_from_the_factory_matrix() {
        let differs = baseline_cell(1, 1).cycled();
        let pins = HashMap::from([((1, 1), differs), ((0, 0), baseline_cell(0, 0))]);
        let rules = seed_from_overrides(&HashMap::new(), &pins);
        assert_eq!(rules.pin_matrix.len(), 1);
        assert_eq!(rules.pin_matrix["output/output"], differs.key());
    }

    #[test]
    fn set_severity_preserves_unknown_ids() {
        let mut rules = ErcRules::default();
        rules.severity.insert("future_rule".into(), "error".into());
        set_severity(&mut rules, RuleKind::UnusedPin, Severity::Info);
        assert_eq!(rules.severity["future_rule"], "error");
        assert_eq!(rules.severity["unused_pin"], "info");
    }

    #[test]
    fn cycling_a_pin_cell_writes_the_mirror_and_returns_to_baseline() {
        let mut rules = ErcRules::default();
        cycle_pin_cell(&mut rules, 0, 1);
        let r = resolve(&rules);
        assert_eq!(r.pin_matrix[&(0, 1)], r.pin_matrix[&(1, 0)]);
        for _ in 0..3 {
            cycle_pin_cell(&mut rules, 0, 1);
        }
        assert!(rules.pin_matrix.is_empty(), "four steps are a full cycle");
    }
}
