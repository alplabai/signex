use serde_json::json;

use super::*;

fn finding(key: &str) -> Finding {
    Finding {
        rule: "builtin::unused_pin".into(),
        severity: Severity::Warning,
        message: format!("finding {key}"),
        evidence: json!({"z": 1, "a": 2}),
        items: vec!["0190a1b2-0000-7000-8000-000000000001".into()],
        sheet: "/p/top.snxsch".into(),
        key: key.into(),
    }
}

fn report(keys: &[&str]) -> CheckReport {
    CheckReport::new(
        "erc",
        keys.iter().map(|k| finding(k)).collect(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn a_report_serialises_to_the_shared_shape() {
    let mut report = report(&["k1"]);
    report.run = 3;
    let value = serde_json::to_value(&report).expect("serialises");
    assert_eq!(
        value,
        json!({
            "check": "erc",
            "run": 3,
            "findings": [{
                "rule": "builtin::unused_pin",
                "severity": "warning",
                "message": "finding k1",
                "evidence": {"a": 2, "z": 1},
                "items": ["0190a1b2-0000-7000-8000-000000000001"],
                "sheet": "/p/top.snxsch",
                "key": "k1",
            }],
            "waived": [],
            "lapsed": [],
            "warnings": [],
        })
    );
    let back: CheckReport = serde_json::from_value(value).expect("reads back");
    assert_eq!(back, report);
}

#[test]
fn the_same_report_serialises_to_the_same_text() {
    let a = serde_json::to_string(&report(&["k1", "k2"])).unwrap();
    let b = serde_json::to_string(&report(&["k1", "k2"])).unwrap();
    assert_eq!(a, b);
    // The evidence object's keys come out sorted, whatever order they were given in.
    assert!(a.contains(r#""evidence":{"a":2,"z":1}"#), "{a}");
}

#[test]
fn a_repeated_key_is_numbered_in_report_order() {
    let report = report(&["k1", "k2", "k1", "k1"]);
    assert_eq!(report.keys(), ["k1", "k1#2", "k1#3", "k2"]);
}

#[test]
fn findings_given_in_any_order_make_the_same_report() {
    let mut error = finding("e");
    error.severity = Severity::Error;
    let mut other_sheet = finding("a");
    other_sheet.sheet = "/p/a.snxsch".into();
    let mut moved = finding("k1");
    moved.evidence = json!({"at": [1, 2]});
    let given = [finding("k2"), error, finding("k1"), other_sheet, moved];
    let forwards = CheckReport::new("erc", given.to_vec(), Vec::new(), Vec::new(), Vec::new());
    let mut reversed = given.to_vec();
    reversed.reverse();
    let backwards = CheckReport::new("erc", reversed, Vec::new(), Vec::new(), Vec::new());
    assert_eq!(forwards, backwards);
    // By sheet, then errors first; two findings with one key are told apart
    // by their evidence, so each keeps the same numbered key.
    assert_eq!(forwards.keys(), ["a", "e", "k1", "k1#2", "k2"]);
    assert_eq!(forwards.findings[3].evidence, json!({"at": [1, 2]}));
}

#[test]
fn waived_keys_are_unique_among_the_waived() {
    let waived = vec![
        WaivedFinding {
            finding: finding("w"),
            waiver: json!({}),
        },
        WaivedFinding {
            finding: finding("w"),
            waiver: json!({}),
        },
    ];
    let report = CheckReport::new("erc", vec![finding("w")], waived, Vec::new(), Vec::new());
    assert_eq!(report.keys(), ["w"]);
    let waived: Vec<_> = report
        .waived
        .iter()
        .map(|w| w.finding.key.as_str())
        .collect();
    assert_eq!(waived, ["w", "w#2"]);
}

#[test]
fn a_comparison_lists_the_new_keys_and_the_gone_ones() {
    let earlier = vec!["k1".to_string(), "k2".to_string(), "k9".to_string()];
    let compared = report(&["k3", "k1", "k4"]).compared_with(7, &earlier);
    assert_eq!(
        compared.baseline,
        Some(BaselineDiff {
            run: 7,
            new: vec!["k3".into(), "k4".into()],
            gone: vec!["k2".into(), "k9".into()],
        })
    );
    assert_eq!(
        serde_json::to_value(&compared).unwrap()["baseline"],
        json!({"run": 7, "new": ["k3", "k4"], "gone": ["k2", "k9"]})
    );
}

#[test]
fn a_comparison_with_the_same_findings_has_nothing_new_or_gone() {
    let first = report(&["k1", "k2"]);
    let compared = report(&["k2", "k1"]).compared_with(1, &first.keys());
    let diff = compared.baseline.expect("compared");
    assert!(diff.new.is_empty() && diff.gone.is_empty(), "{diff:?}");
}

#[test]
fn a_report_names_its_rule_set_only_when_given_one() {
    let plain = serde_json::to_value(report(&["k1"])).unwrap();
    assert!(plain.get("rule_set").is_none(), "{plain}");
    let named = report(&["k1"]).with_rule_set(json!({"sha256": "ab"}));
    let value = serde_json::to_value(&named).unwrap();
    assert_eq!(value["rule_set"], json!({"sha256": "ab"}));
    let back: CheckReport = serde_json::from_value(value).expect("reads back");
    assert_eq!(back, named);
}
