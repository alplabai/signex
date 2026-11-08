//! End to end (#1923): the real `signex-cli` binary on a fixture project, with
//! commands that already run headless.

mod support;

use serde_json::{Value, json};
use support::{fixture, jsonl, run_cli, snapshot, temp_root};

fn statuses(lines: &[Value]) -> Vec<&str> {
    lines
        .iter()
        .map(|l| l["status"].as_str().unwrap())
        .collect()
}

/// open the sheet, colour the net under a point, assign a net class, place a
/// junction, undo it, then save.
fn edit_session() -> Vec<Value> {
    vec![
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "color_net", "args": {"at": [5, 0], "color": "#336699"}}),
        json!({"id": "assign_net_class", "args": {"net_name": "GND", "class": "Power", "path": "main.snxsch"}}),
        json!({"id": "place_junction", "args": {"at": [5.08, 0]}}),
        json!({"id": "undo"}),
        json!({"id": "redo"}),
        json!({"id": "save_document"}),
    ]
}

#[test]
fn a_run_without_save_reports_what_would_change_and_writes_nothing() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let before = snapshot(&root);
    let project = fx.project.to_str().unwrap();

    let out = run_cli(&["run", project, "--keep-going"], jsonl(&edit_session()));

    assert_eq!(
        statuses(&out.lines),
        ["ran", "ran", "ran", "ran", "ran", "ran", "refused"],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
    assert_eq!(out.code, Some(1), "the refused save makes the run not ok");
    let save = &out.lines[6];
    assert_eq!(
        save["reason"],
        "`save_document` needs the `save` capability; pass --save"
    );
    let sheet = fx.sheet.display().to_string();
    assert_eq!(
        out.lines[1]["changed"],
        json!([sheet]),
        "the colour is a change"
    );
    assert_eq!(save["changed"], json!([]));
    assert_eq!(snapshot(&root), before, "a dry run writes nothing");
    assert!(
        out.stderr
            .contains("changes NOT written (--save was not given)"),
        "{}",
        out.stderr
    );
}

#[test]
fn a_run_with_save_writes_the_saved_sheet_and_every_line_carries_the_run_id() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let before = snapshot(&root);
    let project = fx.project.to_str().unwrap();

    let out = run_cli(&["run", project, "--save"], jsonl(&edit_session()));

    assert_eq!(
        statuses(&out.lines),
        ["ran"; 7],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
    assert_eq!(out.code, Some(0));
    let after = snapshot(&root);
    let saved =
        signex_types::format::SnxSchematic::parse(&String::from_utf8_lossy(&after[&fx.sheet]))
            .expect("the saved sheet reads back")
            .sheet;
    assert_eq!(saved.net_colors.len(), 1, "the colour reached the file");
    assert_eq!(saved.junctions.len(), 1, "the junction reached the file");
    assert_ne!(before[&fx.project], after[&fx.project], "the net class too");
    assert_eq!(
        out.lines[6]["changed"],
        json!([
            fx.project.display().to_string(),
            fx.sheet.display().to_string()
        ]),
        "the save clears both unsaved flags"
    );

    let run = out.lines[0]["run"].as_str().unwrap();
    assert!(!run.is_empty());
    for (index, line) in out.lines.iter().enumerate() {
        assert_eq!(line["run"], run);
        assert_eq!(line["seq"], index as u64 + 1);
        assert!(line["messages"].is_array());
    }
    for command in ["open_document", "color_net", "save_document"] {
        assert!(
            out.stderr.contains(&format!("[run {run}]")) && out.stderr.contains(command),
            "the audit log names the run and `{command}`:\n{}",
            out.stderr
        );
    }
}

#[test]
fn one_command_runs_from_arguments_and_a_failed_open_is_failed_not_ran() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();

    let ok = run_cli(
        &["run", project, "open_document", "--arg", "path=main.snxsch"],
        "",
    );
    assert_eq!(statuses(&ok.lines), ["ran"], "{}", ok.stderr);
    assert_eq!(ok.code, Some(0));

    // Inside the project, but not a file: the app reports an error notice.
    let missing = run_cli(
        &[
            "run",
            project,
            "open_document",
            "--arg",
            "path=nothing.snxsch",
        ],
        "",
    );
    assert_eq!(statuses(&missing.lines), ["failed"], "{}", missing.stderr);
    assert!(
        missing.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("nothing.snxsch")
    );
    assert_eq!(missing.code, Some(1));

    // An operand the command does not take is refused with the app's reason.
    let bad = run_cli(&["run", project, "undo", "--arg", "at=1,2"], "");
    assert_eq!(statuses(&bad.lines), ["refused"]);
    assert_eq!(bad.lines[0]["reason"], "`undo` takes no operand `at`");
}

#[test]
fn a_run_stops_at_the_first_command_that_is_not_ok_unless_keep_going() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let commands = jsonl(&[json!({"id": "zq_no_such_command"}), json!({"id": "undo"})]);

    let stopped = run_cli(&["run", project], &commands);
    assert_eq!(statuses(&stopped.lines), ["refused"]);
    assert_eq!(
        stopped.lines[0]["reason"],
        "unknown command `zq_no_such_command`"
    );
    assert_eq!(stopped.code, Some(1));

    let kept = run_cli(&["run", project, "--keep-going"], &commands);
    assert_eq!(statuses(&kept.lines).len(), 2);
    assert_eq!(kept.lines[1]["id"], "undo");
}

#[test]
fn blank_lines_are_skipped_and_do_not_count() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--keep-going"],
        "\n   \n{\"id\":\"select_all\"}\n\n",
    );
    assert_eq!(out.lines.len(), 1, "{}", out.stdout);
    assert_eq!(out.lines[0]["seq"], 1);
}

#[test]
fn the_catalog_lists_every_command_with_its_operands_and_capability() {
    let out = run_cli(&["commands", "--json"], "");
    assert_eq!(out.code, Some(0));
    assert_eq!(out.lines.len(), 1);
    assert_eq!(out.lines[0], signex_cli::catalog::catalog_json());
    let label = out.lines[0]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "place_net_label")
        .unwrap();
    assert_eq!(label["headless"], "runnable");
    assert_eq!(label["operands"][1]["name"], "text");
}

#[test]
fn usage_errors_exit_two_with_a_message_and_help_and_version_answer() {
    let bad = run_cli(&["frobnicate"], "");
    assert_eq!(bad.code, Some(2));
    assert!(
        bad.stderr.contains("unknown subcommand `frobnicate`"),
        "{}",
        bad.stderr
    );
    let help = run_cli(&["--help"], "");
    assert_eq!(help.code, Some(0));
    assert!(help.stdout.contains("Nothing is written without --save"));
    let version = run_cli(&["--version"], "");
    assert!(
        version.stdout.starts_with("signex-cli "),
        "{}",
        version.stdout
    );
    let missing = run_cli(&["run", "/no/such/project.snxprj"], "");
    assert_eq!(missing.code, Some(2), "{}", missing.stderr);
    assert!(missing.lines.is_empty());
}
