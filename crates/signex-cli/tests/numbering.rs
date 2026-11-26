//! #2470 end to end: a new board and the numbering commands run from their
//! operands through the real `signex-cli` binary, with the default `edit`
//! capability, answer with JSON and write no file.

mod support;

use serde_json::{Value, json};
use support::{fixture, jsonl, run_cli, snapshot, temp_root};

fn statuses(lines: &[Value]) -> Vec<&str> {
    lines
        .iter()
        .map(|l| l["status"].as_str().unwrap())
        .collect()
}

#[test]
fn a_new_board_and_the_numbering_commands_answer_without_save() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let before = snapshot(&root);
    let project = fx.project.to_str().unwrap();
    let board = fx.dir.join("Main.snxpcb");
    let board_text = board.display().to_string();

    let out = run_cli(
        &["run", project, "--keep-going"],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            json!({"id": "new_pcb_document", "args": {"document": "demo.snxprj", "name": "Main"}}),
            json!({"id": "annotate_schematic_quietly", "args": {"document": "main.snxsch"}}),
            json!({"id": "force_annotate_all_schematics", "args": {"document": "main.snxsch"}}),
            json!({"id": "reannotate_board", "args": {"document": "Main.snxpcb"}}),
            json!({"id": "back_annotate_schematics", "args": {"document": "Main.snxpcb", "dry_run": true}}),
        ]),
    );

    assert_eq!(
        statuses(&out.lines),
        [
            "answered", "answered", "answered", "answered", "answered", "answered"
        ],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
    let made = &out.lines[1]["data"];
    assert_eq!(made["kind"], "pcb", "{made}");
    assert_eq!(made["path"], json!(board_text), "{made}");
    assert!(out.lines[2]["data"]["changes"].is_array());
    assert_eq!(out.lines[3]["data"]["mode"], "reset_and_renumber");
    assert_eq!(out.lines[4]["data"]["applied"], false, "an empty board");
    assert!(out.lines[5]["data"]["rows"].is_array());
    assert_eq!(out.lines[5]["data"]["applied"], false);
    assert!(!board.exists(), "the board stays unsaved");
    assert_eq!(snapshot(&root), before, "`edit` writes nothing");
}

/// New PCB has no `dry_run` operand, so `--dry-run` refuses it rather than
/// make a board it was told not to.
#[test]
fn a_dry_run_refuses_new_pcb() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();

    let out = run_cli(
        &["run", project, "--dry-run"],
        jsonl(&[json!({"id": "new_pcb_document", "args": {"document": "demo.snxprj"}})]),
    );

    assert_eq!(
        statuses(&out.lines),
        ["refused"],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
}
