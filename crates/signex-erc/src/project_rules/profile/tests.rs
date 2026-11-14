use super::*;
use crate::Severity;

#[test]
fn builtin_profile_lists_every_rule_and_hashes_stably() {
    let p = builtin_profile();
    assert_eq!(p.name, BUILTIN_PROFILE_NAME);
    assert_eq!(p.erc.severity.len(), RuleKind::ALL.len());
    assert_eq!(p.content_sha256(), builtin_profile().content_sha256());
}

/// The shipped profile's hash is recorded in existing projects, so a DRC
/// section (#2307) must not move it: pinned at the value trunk shipped.
#[test]
fn builtin_profile_hash_and_bytes_are_unchanged_by_the_drc_section() {
    let p = builtin_profile();
    assert_eq!(
        p.content_sha256(),
        "89b152e97b10f513191914dc00133755a6fd3a57271b43193ac062f95aebdd76"
    );
    assert!(
        !String::from_utf8(p.to_json_bytes())
            .unwrap()
            .contains("drc")
    );
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

/// A change to the profile's `drc` section alone is drift (#2307 step 2b),
/// and a project recorded against a profile with `drc` before the section
/// entered the hash (the ERC hash only) reads as drifted once.
#[test]
fn a_drc_only_profile_change_is_drift() {
    let mut p = builtin_profile();
    p.drc.dsl = Some("rule a on clearance then error".into());
    let r = profile_ref(&p, SOURCE_USER);
    assert!(!has_drifted(&r, &p));
    let mut changed = p.clone();
    changed
        .drc
        .severity
        .insert("builtin::unconnected".into(), "error".into());
    assert!(has_drifted(&r, &changed));

    let recorded_before = RuleProfileRef {
        sha256: p.erc.sha256_hex(),
        ..r
    };
    assert!(has_drifted(&recorded_before, &p));
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
