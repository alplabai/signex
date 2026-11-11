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

/// List `filename` as a page of the fixture's project.
fn add_page(project: &std::path::Path, filename: &str) {
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.sheets.push(signex_types::project::SheetEntry {
        name: "Extra".into(),
        filename: filename.into(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    signex_types::project::write_project(project, &data).expect("project");
}

/// `check_erc` on the fixture's sheet: its answer line.
fn check_main(project: &std::path::Path) -> serde_json::Value {
    let out = run_cli(
        &["run", project.to_str().unwrap()],
        jsonl(&[json!({"id": "check_erc", "args": {"document": "main.snxsch"}})]),
    );
    assert_eq!(out.lines[0]["status"], "answered", "{}", out.stdout);
    out.lines[0]["data"].clone()
}

fn warnings_naming(report: &serde_json::Value, names: &[&str]) -> Vec<String> {
    report["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .filter_map(|w| w.as_str())
        .filter(|w| names.iter().all(|name| w.contains(name)))
        .map(str::to_string)
        .collect()
}

/// A page the project names outside its folder is not read by a headless
/// check, as a query does not read it; the report says so.
#[test]
fn a_page_outside_the_project_folder_is_not_read_and_is_named_in_warnings() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    std::fs::copy(&fx.sheet, root.join("outside.snxsch")).expect("copy");
    add_page(&fx.project, "../outside.snxsch");
    let report = check_main(&fx.project);
    let findings = report["findings"].as_array().expect("findings");
    assert!(
        findings.iter().all(|f| !f["sheet"]
            .as_str()
            .unwrap_or_default()
            .ends_with("outside.snxsch")),
        "{report}"
    );
    let named = warnings_naming(&report, &["outside.snxsch", "outside the project folder"]);
    assert!(!named.is_empty(), "{report}");
}

/// A FIFO named as a page does not hang a headless check, and is named as a
/// file a headless run does not read.
#[cfg(unix)]
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "`mkfifo` makes the pipe; the standard library has no call for it"
)]
fn a_fifo_page_does_not_hang_a_check() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let pipe = fx.sheet.with_file_name("pipe.snxsch");
    let made = std::process::Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .unwrap();
    assert!(made.success());
    add_page(&fx.project, "pipe.snxsch");
    let report = check_main(&fx.project);
    let named = warnings_naming(&report, &["pipe.snxsch", "a headless run does not read it"]);
    assert!(!named.is_empty(), "{report}");
}

/// The project's `erc.dsl` that links out of the folder is not read.
#[cfg(unix)]
#[test]
fn an_erc_dsl_linking_out_of_the_folder_is_not_read() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let outside = root.join("rules.dsl");
    std::fs::write(&outside, "not a rule").expect("write");
    std::os::unix::fs::symlink(&outside, fx.dir.join("erc.dsl")).expect("link");
    let report = check_main(&fx.project);
    let named = warnings_naming(&report, &["ERC DSL", "outside the project folder"]);
    assert_eq!(named.len(), 1, "{report}");
}
