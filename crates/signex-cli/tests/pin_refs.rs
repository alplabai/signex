//! End to end (#2199): a wire between two pin references through the real
//! `signex-cli` binary, its dry run, and what `--dry-run` refuses.

mod support;

use serde_json::{Value, json};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root};

fn wire() -> Value {
    json!({"id": "place_wire", "args": {"start": "R1.2", "end": "C1.1"}})
}

#[test]
fn a_wire_between_two_pin_references_answers_the_net_and_saves_with_save() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project, "--save"],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            wire(),
            json!({"id": "save_document", "args": {"document": "main.snxsch"}}),
        ]),
    );
    assert_eq!(out.code, Some(0), "{}", out.stdout);
    // On a project sheet in a fresh run the hierarchy gate does not refuse.
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    assert_eq!(out.lines[1]["data"]["dry_run"], false);
    assert_eq!(out.lines[1]["data"]["net"]["pins"], json!(["C1.1", "R1.2"]));
    assert_eq!(out.lines[2]["status"], "ran", "{}", out.stdout);

    // The saved sheet has the wire.
    let again = run_cli(
        &["run", project],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            json!({"id": "read_document", "args": {"document": "main.snxsch", "kinds": "wire"}}),
        ]),
    );
    assert_eq!(again.lines[1]["data"]["total"], 1, "{}", again.stdout);
}

#[test]
fn a_dry_run_reports_the_net_and_writes_nothing() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    let before = snapshot(&root);
    let out = run_cli(
        &["run", project],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            json!({"id": "place_wire", "args": {"start": "R1.2", "end": "C1.1", "dry_run": true}}),
        ]),
    );
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    assert_eq!(out.lines[1]["data"]["dry_run"], true);
    assert_eq!(out.lines[1]["data"]["net"]["pins"], json!(["C1.1", "R1.2"]));
    assert_eq!(out.lines[1]["changed"], json!([]), "{}", out.stdout);
    assert_eq!(snapshot(&root), before);
}

#[test]
fn the_dry_run_flag_sets_the_operand_and_refuses_what_has_none() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    // A loose sheet is open from the start, so no `open_document` is needed.
    let sheet = fx.sheet.to_str().unwrap();
    let before = snapshot(&root);
    let out = run_cli(
        &["run", sheet, "--dry-run", "--keep-going"],
        jsonl(&[
            json!({"id": "undo", "args": {"document": "main.snxsch"}}),
            wire(),
        ]),
    );
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    assert!(
        out.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("`dry_run` operand"),
        "{}",
        out.stdout
    );
    // The wire got `dry_run=true`: it reports and changes nothing.
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    assert_eq!(out.lines[1]["data"]["dry_run"], true);
    assert_eq!(out.lines[1]["changed"], json!([]));
    assert_eq!(snapshot(&root), before);
}

#[test]
fn the_dry_run_flag_with_save_is_a_usage_error() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    let out = run_cli(&["run", project, "--dry-run", "--save"], "");
    assert_eq!(out.code, Some(2), "{}", out.stderr);
    assert!(
        out.stderr.contains("--dry-run changes nothing"),
        "{}",
        out.stderr
    );
    assert!(out.lines.is_empty());
}
