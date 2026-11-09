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
        json!({"id": "color_net", "args": {"at": [5, 0], "color": "#336699", "document": "main.snxsch"}}),
        json!({"id": "assign_net_class", "args": {"net_name": "GND", "class": "Power", "document": "main.snxsch"}}),
        json!({"id": "place_junction", "args": {"at": [5.08, 0]}}),
        json!({"id": "undo", "args": {"document": "main.snxsch"}}),
        json!({"id": "redo", "args": {"document": "main.snxsch"}}),
        json!({"id": "save_document", "args": {"document": "main.snxsch"}}),
        json!({"id": "save_document", "args": {"document": "demo.snxprj"}}),
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
        [
            "answered", "ran", "ran", "ran", "ran", "ran", "refused", "refused"
        ],
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
        ["answered", "ran", "ran", "ran", "ran", "ran", "ran", "ran"],
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
    // A save names its document: the sheet's clears the sheet's flag, the
    // project's the project's.
    assert_eq!(
        out.lines[6]["changed"],
        json!([fx.sheet.display().to_string()]),
        "the sheet's save clears the sheet's flag"
    );
    assert_eq!(
        out.lines[7]["changed"],
        json!([fx.project.display().to_string()]),
        "the project's save clears the project's flag"
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
    assert_eq!(statuses(&ok.lines), ["answered"], "{}", ok.stderr);
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

/// #2172: the queries answer with data (`status: answered`), and a document
/// is named by its id or by a path the run confines like any other path.
#[test]
fn queries_answer_with_data_and_a_document_is_named_by_id_or_by_path() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let id = signex_types::format::document_id_in_header(
        "snxsch",
        &std::fs::read(&fx.sheet).expect("sheet"),
    )
    .expect("the sheet carries an id")
    .to_string();
    let before = snapshot(&root);

    let out = run_cli(
        &["run", project],
        jsonl(&[
            json!({"id": "list_documents"}),
            json!({"id": "read_document", "args": {"document": id}}),
            json!({"id": "read_document", "args": {"document": "main.snxsch", "kinds": "wire"}}),
            json!({"id": "document_info", "args": {"document": fx.sheet}}),
            json!({"id": "find_items", "args": {"designator": "R1"}}),
            json!({"id": "resolve", "args": {"id": id}}),
        ]),
    );

    assert_eq!(
        statuses(&out.lines),
        ["answered"; 6],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
    assert_eq!(out.code, Some(0));
    let listed = out.lines[0]["data"]["documents"].as_array().unwrap();
    assert!(
        listed
            .iter()
            .any(|d| d["id"] == id.as_str() && d["kind"] == "schematic")
    );
    assert_eq!(out.lines[1]["data"]["document"]["id"], id.as_str());
    assert!(out.lines[1]["data"]["total"].as_u64().unwrap() > 0);
    assert_eq!(
        out.lines[2]["data"]["total"], 1,
        "by a relative path, one wire"
    );
    assert_eq!(out.lines[3]["data"]["id"], id.as_str());
    assert_eq!(
        out.lines[4]["data"]["count"], 1,
        "R1 is found by its designator"
    );
    assert_eq!(out.lines[5]["data"]["what"], "document");
    assert_eq!(snapshot(&root), before, "a query writes nothing");
}

/// A `Document` operand given as a path is a path: refused outside the
/// project like a `path` operand. An id is not a path and is never confined.
#[test]
fn a_document_given_as_a_path_is_confined_but_an_id_is_not() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    std::fs::copy(&fx.sheet, other.join("outside.snxsch")).expect("copy");
    let outside = other.join("outside.snxsch");
    let project = fx.project.to_str().unwrap();

    let out = run_cli(
        &["run", project, "--keep-going"],
        jsonl(&[
            json!({"id": "read_document", "args": {"document": outside}}),
            json!({"id": "document_info", "args": {"document": "../../etc/hosts"}}),
            json!({"id": "document_info", "args": {"document": "00000000-0000-7000-8000-00000000dead"}}),
        ]),
    );

    assert_eq!(
        statuses(&out.lines)[..2],
        ["refused", "refused"],
        "{}",
        out.stdout
    );
    for index in 0..2 {
        let reason = out.lines[index]["reason"].as_str().unwrap();
        assert!(reason.starts_with("operand `document`:"), "{reason}");
    }
    // An id reaches the app untouched: it names no open document, and says so.
    let unknown = out.lines[2]["reason"].as_str().unwrap();
    assert!(unknown.contains("no document has the id"), "{unknown}");
}
