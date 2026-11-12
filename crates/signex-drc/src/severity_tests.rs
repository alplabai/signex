use std::collections::BTreeSet;

use signex_types::project::DrcRuleSet;
use signex_types::violation::Severity;

use super::*;

fn layer(entries: &[(&str, &str)]) -> DrcRuleSet {
    let mut set = DrcRuleSet::default();
    for (id, sev) in entries {
        set.severity.insert((*id).into(), (*sev).into());
    }
    set
}

#[test]
fn every_kind_has_a_stable_builtin_rule_id() {
    let ids: Vec<_> = ViolationKind::ALL.iter().map(|k| k.rule_id()).collect();
    assert_eq!(
        ids,
        [
            "builtin::short",
            "builtin::clearance",
            "builtin::unconnected",
            "builtin::keepout",
            "builtin::track_width",
            "builtin::via_diameter",
            "builtin::via_drill"
        ]
    );
    for kind in ViolationKind::ALL {
        assert_eq!(ViolationKind::from_rule_id(kind.rule_id()), Some(kind));
    }
    assert_eq!(ViolationKind::from_rule_id("short"), None);
    assert_eq!(ViolationKind::from_rule_id("user::short"), None);
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
}

#[test]
fn builtin_severities_make_unconnected_a_warning_and_the_rest_errors() {
    let builtin = DrcSeverities::builtin();
    assert_eq!(builtin.of(ViolationKind::Short), Some(Severity::Error));
    assert_eq!(builtin.of(ViolationKind::Clearance), Some(Severity::Error));
    assert_eq!(builtin.of(ViolationKind::Keepout), Some(Severity::Error));
    for kind in [
        ViolationKind::TrackWidth,
        ViolationKind::ViaDiameter,
        ViolationKind::ViaDrill,
    ] {
        assert_eq!(builtin.of(kind), Some(Severity::Error), "{kind:?}");
    }
    assert_eq!(
        builtin.of(ViolationKind::Unconnected),
        Some(Severity::Warning)
    );
    assert!(builtin.unrecognised.is_empty());
    assert_eq!(resolve_severities(None, None), builtin);
    assert_eq!(DrcRules::new(1).severities, builtin);
}

#[test]
fn the_profile_overrides_the_builtin_and_the_project_overrides_the_profile() {
    let profile = layer(&[
        ("builtin::unconnected", "error"),
        ("builtin::keepout", "info"),
    ]);
    let project = layer(&[("builtin::keepout", "warning")]);

    let from_profile = resolve_severities(Some(&profile), None);
    assert_eq!(
        from_profile.of(ViolationKind::Unconnected),
        Some(Severity::Error)
    );
    assert_eq!(
        from_profile.of(ViolationKind::Keepout),
        Some(Severity::Info)
    );

    let both = resolve_severities(Some(&profile), Some(&project));
    assert_eq!(both.of(ViolationKind::Keepout), Some(Severity::Warning));
    assert_eq!(both.of(ViolationKind::Unconnected), Some(Severity::Error));
    assert_eq!(both.of(ViolationKind::Short), Some(Severity::Error));

    let project_only = resolve_severities(None, Some(&project));
    assert_eq!(
        project_only.of(ViolationKind::Unconnected),
        Some(Severity::Warning)
    );
}

#[test]
fn off_switches_a_rule_off_and_a_later_layer_can_switch_it_back_on() {
    let profile = layer(&[("builtin::short", "off")]);
    assert_eq!(
        resolve_severities(Some(&profile), None).of(ViolationKind::Short),
        None
    );
    let project = layer(&[("builtin::short", "warning")]);
    assert_eq!(
        resolve_severities(Some(&profile), Some(&project)).of(ViolationKind::Short),
        Some(Severity::Warning)
    );
}

#[test]
fn entries_this_build_cannot_map_are_reported_not_applied() {
    let project = layer(&[
        ("builtin::clearance", "fatal"),
        ("user::width", "error"),
        ("clearance", "warning"),
    ]);
    let got = resolve_severities(None, Some(&project));
    assert_eq!(got.of(ViolationKind::Clearance), Some(Severity::Error));
    assert_eq!(got.unrecognised.len(), 3, "{:?}", got.unrecognised);
}

#[test]
fn a_report_gives_each_violation_its_resolved_severity() {
    use crate::tests::{board, track};
    // Two crossing tracks of nets 1 and 2: one short.
    let b = board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (5.0, -5.0), (5.0, 5.0), 2, "TopCopper"),
        ],
        vec![],
    );
    let mut rules = DrcRules::new(200_000);
    let plain = run(&b, &rules);
    assert_eq!(plain.rules.severities, DrcSeverities::builtin());
    let short = &plain.violations[0];
    assert_eq!(short.kind, ViolationKind::Short);
    assert_eq!(plain.severity_of(short), Some(Severity::Error));

    rules.severities = resolve_severities(None, Some(&layer(&[("builtin::short", "info")])));
    let report = run(&b, &rules);
    assert_eq!(
        report.violations, plain.violations,
        "severity never filters"
    );
    assert_eq!(
        report.severity_of(&report.violations[0]),
        Some(Severity::Info)
    );
}
