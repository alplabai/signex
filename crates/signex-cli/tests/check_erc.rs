//! End to end (#2199 step D): `check_erc` through the real `signex-cli`
//! binary answers with a check report, compares a later run with an earlier
//! one, refuses a baseline it does not keep and writes nothing.

mod support;

use serde_json::json;
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

#[test]
fn check_erc_answers_with_a_report_and_compares_with_its_baseline() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    let before = without_journals(snapshot(&root));
    let check = |baseline: Option<&str>| match baseline {
        Some(run) => {
            json!({"id": "check_erc", "args": {"document": "main.snxsch", "baseline": run}})
        }
        None => json!({"id": "check_erc", "args": {"document": "main.snxsch"}}),
    };
    // The sheet stays closed: the run reads it.
    let out = run_cli(
        &["run", project],
        jsonl(&[check(None), check(Some("1")), check(Some("9"))]),
    );
    let first = &out.lines[0];
    assert_eq!(first["status"], "answered", "{}", out.stdout);
    assert_eq!(first["data"]["check"], "erc", "{}", out.stdout);
    assert_eq!(first["data"]["run"], 1, "{}", out.stdout);
    let findings = first["data"]["findings"].as_array().expect("findings");
    assert!(
        findings
            .iter()
            .any(|f| f["rule"] == "builtin::unused_pin" && f["sheet"].as_str().is_some()),
        "{}",
        out.stdout
    );

    let second = &out.lines[1];
    assert_eq!(second["status"], "answered", "{}", out.stdout);
    assert_eq!(second["data"]["run"], 2, "{}", out.stdout);
    assert_eq!(
        second["data"]["findings"], first["data"]["findings"],
        "{}",
        out.stdout
    );
    assert_eq!(
        second["data"]["baseline"],
        json!({"run": 1, "new": [], "gone": []}),
        "{}",
        out.stdout
    );

    let third = &out.lines[2];
    assert_eq!(third["status"], "refused", "{}", out.stdout);
    let reason = third["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("no such run"), "{reason}");

    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}
