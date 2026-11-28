//! End to end (#2836): a schematic dimension placed through the real
//! `signex-cli` binary from the operands that stand for the tool's three
//! clicks and its Properties form, saved, and refused off the grid.

mod support;

use serde_json::{Value, json};
use support::{fixture, jsonl, run_cli, snapshot, temp_root};

fn dimension(start: &str, extra: Value) -> Value {
    let mut args = json!({
        "start": start, "end": "33.02,20.32", "at": "25.4,15.24", "document": "main.snxsch"
    });
    if let (Some(args), Value::Object(extra)) = (args.as_object_mut(), extra) {
        args.extend(extra);
    }
    json!({"id": "place_linear_dimension", "args": args})
}

#[test]
fn a_dimension_placed_from_operands_answers_with_its_text_and_is_saved() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project, "--save"],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            dimension("20.32,20.32", json!({"units": "mil", "precision": "0"})),
            json!({"id": "save_document", "args": {"document": "main.snxsch"}}),
        ]),
    );
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    let placed = &out.lines[1]["data"];
    assert_eq!(placed["kind"], "dimension", "{}", out.stdout);
    assert_eq!(placed["label"], "500 mil", "{}", out.stdout);
    assert_eq!(
        placed["dimension"]["direction"], "horizontal",
        "{}",
        out.stdout
    );
    assert_eq!(out.lines[2]["status"], "ran", "{}", out.stdout);
    let id = placed["id"].as_str().expect("an id");
    let saved = std::fs::read_to_string(&fx.sheet).expect("the saved sheet");
    assert!(saved.contains(id), "the dimension is in the saved sheet");
}

#[test]
fn a_dimension_with_a_precision_it_cannot_write_is_refused_and_nothing_is_written() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let before = snapshot(&root);
    let out = run_cli(
        &["run", project],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            dimension("20.32,20.32", json!({"precision": "9"})),
        ]),
    );
    assert_eq!(out.lines[1]["status"], "refused", "{}", out.stdout);
    let reason = out.lines[1]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("from 0 to 3"), "{reason}");
    assert_eq!(snapshot(&root), before);
}
