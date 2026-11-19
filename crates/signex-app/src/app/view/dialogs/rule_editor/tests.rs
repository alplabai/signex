use std::collections::HashMap;

use signex_erc::project_rules::{ResolvedRules, profile::builtin_profile, resolve};
use signex_erc::{RuleGroup, RuleKind, Severity};
use signex_i18n::ids;

use super::*;

fn shipped() -> ResolvedRules {
    resolve(&builtin_profile().erc)
}

fn listed(sections: &[RuleSection]) -> Vec<RuleKind> {
    sections
        .iter()
        .flat_map(|s| s.rows.iter().map(|r| r.rule))
        .collect()
}

#[test]
fn the_unfiltered_list_holds_every_rule_under_its_group() {
    let sections = rule_sections(&HashMap::new(), &shipped(), false, "");
    let groups: Vec<RuleGroup> = sections.iter().map(|s| s.group).collect();
    assert_eq!(groups, RuleGroup::ALL);
    assert_eq!(listed(&sections).len(), RuleKind::ALL.len());
    for section in &sections {
        for row in &section.rows {
            assert_eq!(row.rule.group(), section.group);
            assert_eq!(row.severity, row.rule.default_severity());
            assert!(!row.changed, "{:?}", row.rule);
        }
    }
}

#[test]
fn changed_only_keeps_only_the_rules_that_differ_from_the_shipped_profile() {
    let overrides = HashMap::from([
        (RuleKind::UnusedPin, Severity::Off),
        (RuleKind::PartWithoutMpn, Severity::Warning),
        // Set to what it ships at: not a change.
        (
            RuleKind::DanglingWire,
            RuleKind::DanglingWire.default_severity(),
        ),
        // An on/off-only rule switched "on" at another level is still on.
        (RuleKind::PinTypeConflict, Severity::Warning),
    ]);
    let sections = rule_sections(&overrides, &shipped(), true, "");
    assert_eq!(
        listed(&sections),
        [RuleKind::UnusedPin, RuleKind::PartWithoutMpn]
    );
    assert!(sections.iter().flat_map(|s| &s.rows).all(|r| r.changed));
    let groups: Vec<RuleGroup> = sections.iter().map(|s| s.group).collect();
    assert_eq!(
        groups,
        [RuleGroup::Pins, RuleGroup::Parts],
        "empty groups drop"
    );
    assert_eq!(changed_rule_count(&overrides, &shipped()), 2);
}

#[test]
fn changed_only_with_nothing_changed_lists_nothing() {
    assert!(rule_sections(&HashMap::new(), &shipped(), true, "").is_empty());
}

#[test]
fn the_filter_matches_rule_names_ignoring_case() {
    let sections = rule_sections(&HashMap::new(), &shipped(), false, "  BUS LABEL ");
    assert_eq!(listed(&sections), [RuleKind::BusLabelOnWire]);
    let sections = rule_sections(&HashMap::new(), &shipped(), false, "sheet pin");
    assert_eq!(
        listed(&sections),
        [
            RuleKind::BadHierSheetPin,
            RuleKind::SheetPinUnconnected,
            RuleKind::SheetPinDirectionConflict
        ]
    );
    assert!(rule_sections(&HashMap::new(), &shipped(), false, "no such rule").is_empty());
}

#[test]
fn the_filter_and_changed_only_combine() {
    let overrides = HashMap::from([
        (RuleKind::UnusedPin, Severity::Off),
        (RuleKind::OrphanLabel, Severity::Error),
    ]);
    let sections = rule_sections(&overrides, &shipped(), true, "label");
    assert_eq!(listed(&sections), [RuleKind::OrphanLabel]);
}

#[test]
fn only_the_lower_triangle_of_the_matrix_is_drawn() {
    let cells = lower_triangle(&HashMap::new(), &shipped());
    assert_eq!(cells.len(), signex_erc::PinAxis::ALL.len());
    for (row, line) in cells.iter().enumerate() {
        assert_eq!(line.len(), row + 1, "row {row}");
        for (col, cell) in line.iter().enumerate() {
            assert_eq!((cell.row, cell.col), (row as u8, col as u8));
            assert!(cell.col <= cell.row);
            assert_eq!(
                cell.severity,
                signex_erc::pin_matrix::baseline_cell(cell.row, cell.col)
            );
            assert!(!cell.changed);
        }
    }
    assert_eq!(cells.iter().flatten().count(), 21);
}

#[test]
fn a_changed_matrix_cell_is_marked_and_upper_cells_are_not_drawn() {
    // PowerOut x PowerOut ships as Error; NC x Input ships as Off.
    let overrides = HashMap::from([
        ((4, 4), Severity::Warning),
        ((5, 0), Severity::Warning),
        ((0, 5), Severity::Warning),
    ]);
    let cells = lower_triangle(&overrides, &shipped());
    let changed: Vec<(u8, u8)> = cells
        .iter()
        .flatten()
        .filter(|c| c.changed)
        .map(|c| (c.row, c.col))
        .collect();
    assert_eq!(changed, [(4, 4), (5, 0)]);
    assert_eq!(cells[4][4].severity, Severity::Warning);
}

#[test]
fn a_rule_control_offers_four_levels_and_an_on_off_rule_two() {
    let segments = severity_segments(RuleKind::UnusedPin, Severity::Warning);
    let values: Vec<Severity> = segments.iter().map(|s| s.value).collect();
    assert_eq!(
        values,
        [
            Severity::Error,
            Severity::Warning,
            Severity::Info,
            Severity::Off
        ]
    );
    let selected: Vec<bool> = segments.iter().map(|s| s.selected).collect();
    assert_eq!(selected, [false, true, false, false]);

    let on_off = severity_segments(RuleKind::PinTypeConflict, Severity::Error);
    assert_eq!(on_off.len(), 2);
    assert_eq!(on_off[0].label, ids::DIALOG_RULES_ON);
    assert_eq!(
        on_off[0].value,
        RuleKind::PinTypeConflict.default_severity()
    );
    assert!(on_off[0].selected && !on_off[1].selected);
    let off = severity_segments(RuleKind::PinTypeConflict, Severity::Off);
    assert!(!off[0].selected && off[1].selected);
}

#[test]
fn every_group_has_a_catalog_heading() {
    let headings: std::collections::HashSet<_> =
        RuleGroup::ALL.into_iter().map(group_heading).collect();
    assert_eq!(headings.len(), RuleGroup::ALL.len());
}
