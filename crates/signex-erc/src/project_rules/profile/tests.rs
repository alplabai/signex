use super::*;
use crate::Severity;

#[test]
fn builtin_profile_lists_every_rule_and_hashes_stably() {
    let p = builtin_profile();
    assert_eq!(p.name, BUILTIN_PROFILE_NAME);
    assert_eq!(p.erc.severity.len(), RuleKind::ALL.len());
    assert_eq!(p.content_sha256(), builtin_profile().content_sha256());
}

#[test]
fn profile_ref_records_name_source_and_content_hash() {
    let p = builtin_profile();
    let r = profile_ref(&p, SOURCE_BUILTIN);
    assert_eq!(
        (r.name.as_str(), r.source.as_str()),
        ("Signex Default", "builtin")
    );
    assert_eq!(r.sha256, p.content_sha256());
    assert!(!has_drifted(&r, &p));
    let mut changed = p.clone();
    changed
        .erc
        .severity
        .insert("unused_pin".into(), "off".into());
    assert!(has_drifted(&r, &changed));
    let mut no_hash = r.clone();
    no_hash.sha256.clear();
    assert!(!has_drifted(&no_hash, &changed));
}

#[test]
fn diff_lists_only_differences_and_leaves_foreign_ids_alone() {
    let mut current = builtin_profile().erc;
    current
        .severity
        .insert("future_rule".into(), "error".into());
    current
        .pin_matrix
        .insert("future/axis".into(), "error".into());
    current.severity.insert("unused_pin".into(), "off".into());
    let profile = builtin_profile().erc;
    let diff = diff_against_profile(&current, &profile);
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert_eq!(diff[0].id, "unused_pin");
    assert_eq!(diff[0].from.as_deref(), Some("off"));
    assert_eq!(
        diff[0].to.as_deref(),
        Some(RuleKind::UnusedPin.default_severity().key())
    );
    assert!(
        diff[0]
            .describe()
            .starts_with("severity unused_pin: off -> ")
    );

    let after = adopt_profile(&current, &profile);
    assert_eq!(after.severity["future_rule"], "error");
    assert_eq!(after.pin_matrix["future/axis"], "error");
}

#[test]
fn identical_rules_diff_to_nothing_and_pin_cells_report_factory() {
    let base = builtin_profile().erc;
    assert!(diff_against_profile(&base, &base).is_empty());
    let mut profile = base.clone();
    profile
        .pin_matrix
        .insert("output/output".into(), Severity::Off.key().into());
    let diff = diff_against_profile(&base, &profile);
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].from, None);
    assert!(diff[0].describe().contains("factory -> off"));
}
