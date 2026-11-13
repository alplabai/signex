use signex_types::project::DrcRuleSet;

use crate::{DrcRuleSection, adopt_profile_drc, diff_drc_against_profile};

fn drc(severity: &[(&str, &str)], dsl: Option<&str>, unknown: &str) -> DrcRuleSet {
    DrcRuleSet {
        severity: severity
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        dsl: dsl.map(str::to_string),
        unknown: serde_json::from_str(unknown).unwrap(),
    }
}

#[test]
fn two_empty_sections_diff_to_nothing_and_stay_empty() {
    let empty = DrcRuleSet::default();
    assert!(diff_drc_against_profile(&empty, &empty).is_empty());
    assert!(adopt_profile_drc(&empty, &empty).is_empty());
}

#[test]
fn adopting_takes_the_profiles_rules_and_text() {
    let project = drc(
        &[("builtin::clearance", "warning")],
        Some("rule mine"),
        "{}",
    );
    let profile = drc(
        &[("builtin::unconnected", "error")],
        Some("rule a\nrule b"),
        "{}",
    );
    assert_eq!(adopt_profile_drc(&project, &profile), profile);
}

/// An id from a newer build and a foreign key survive an update; a
/// `user::` severity follows the replaced rule text and does not.
#[test]
fn adopting_keeps_what_this_build_cannot_read() {
    let project = drc(
        &[
            ("builtin::future_kind", "error"),
            ("vendor::x", "off"),
            ("user::mine", "off"),
        ],
        None,
        r#"{"future_key": [1], "shared": 1}"#,
    );
    let profile = drc(&[], None, r#"{"shared": 2}"#);
    let after = adopt_profile_drc(&project, &profile);
    assert_eq!(after.severity["builtin::future_kind"], "error");
    assert_eq!(after.severity["vendor::x"], "off");
    assert!(!after.severity.contains_key("user::mine"));
    assert_eq!(
        after.unknown.get("future_key"),
        Some(&serde_json::json!([1]))
    );
    assert_eq!(after.unknown.get("shared"), Some(&serde_json::json!(2)));
}

#[test]
fn the_diff_lists_severities_text_and_foreign_keys_readably() {
    let project = drc(
        &[("builtin::clearance", "warning"), ("user::mine", "off")],
        Some("rule a"),
        r#"{"shared": 1, "kept": 0}"#,
    );
    let profile = drc(
        &[("builtin::unconnected", "error")],
        Some("rule a\nrule b"),
        r#"{"shared": 2}"#,
    );
    let lines: Vec<String> = diff_drc_against_profile(&project, &profile)
        .iter()
        .map(|c| c.describe())
        .collect();
    assert_eq!(
        lines,
        [
            "DRC severity builtin::clearance: warning -> default",
            "DRC severity builtin::unconnected: default -> error",
            "DRC severity user::mine: off -> the rule's own",
            "DRC rule text: changed (1 line -> 2 lines)",
            "DRC other setting shared: 1 -> 2",
        ]
    );
}

#[test]
fn removing_the_rule_text_is_listed() {
    let project = drc(&[], Some("rule a"), "{}");
    let changes = diff_drc_against_profile(&project, &DrcRuleSet::default());
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].section, DrcRuleSection::RuleText);
    assert_eq!(changes[0].describe(), "DRC rule text: removed (1 line)");
    let added = diff_drc_against_profile(&DrcRuleSet::default(), &project);
    assert_eq!(added[0].describe(), "DRC rule text: added (1 line)");
}

#[test]
fn a_changed_text_with_the_same_line_count_is_still_listed() {
    let project = drc(&[], Some("rule a"), "{}");
    let profile = drc(&[], Some("rule b"), "{}");
    let changes = diff_drc_against_profile(&project, &profile);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].from.as_deref(), Some("rule a"));
    assert_eq!(changes[0].to.as_deref(), Some("rule b"));
    assert_eq!(
        changes[0].describe(),
        "DRC rule text: changed (1 line -> 1 line)"
    );
}
