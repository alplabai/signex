use super::*;

fn rules(sev: &[(&str, &str)], pins: &[(&str, &str)]) -> ErcRules {
    let mut rules = ErcRules::default();
    for (k, v) in sev {
        rules.severity.insert((*k).into(), (*v).into());
    }
    for (k, v) in pins {
        rules.pin_matrix.insert((*k).into(), (*v).into());
    }
    rules
}

#[test]
fn every_rule_key_round_trips_and_is_unique() {
    for rule in RuleKind::ALL {
        assert_eq!(RuleKind::from_key(rule.key()), Some(rule));
    }
    let mut keys: Vec<_> = RuleKind::ALL.iter().map(|r| r.key()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), RuleKind::ALL.len());
}

#[test]
fn resolves_severities_and_pin_cells() {
    let r = resolve(&rules(
        &[("dangling_wire", "off"), ("unused_pin", "error")],
        &[("output/power_out", "warning")],
    ));
    assert_eq!(r.severity[&RuleKind::DanglingWire], Severity::Off);
    assert_eq!(r.severity[&RuleKind::UnusedPin], Severity::Error);
    assert_eq!(r.pin_matrix[&(1, 4)], Severity::Warning);
    assert!(r.unrecognised.is_empty());
}

#[test]
fn unknown_ids_are_reported_not_dropped_silently() {
    let r = resolve(&rules(
        &[("future_rule", "error"), ("unused_pin", "loud")],
        &[("input/hyper", "error")],
    ));
    assert_eq!(r.unrecognised.len(), 3);
    assert!(r.severity.is_empty());
}

#[test]
fn rules_the_project_lacks_are_listed_as_missing() {
    let r = resolve(&rules(&[("unused_pin", "error")], &[]));
    assert!(!r.missing.contains(&RuleKind::UnusedPin));
    assert!(r.missing.contains(&RuleKind::DanglingWire));
    assert_eq!(r.missing.len(), RuleKind::ALL.len() - 1);
}

#[test]
fn pin_matrix_key_matches_the_parser() {
    assert_eq!(pin_matrix_key(1, 4).as_deref(), Some("output/power_out"));
    assert_eq!(pin_matrix_key(6, 0), None);
    assert_eq!(parse_pin_matrix_key("output/power_out"), Some((1, 4)));
}
