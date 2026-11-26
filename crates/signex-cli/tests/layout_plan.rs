//! End to end (#2942): the board's layout plan through the real `signex-cli`
//! binary. `layout_plan_set` sets one stage as one undo step and answers with
//! the plan, `layout_plan_get` and `layout_plan_status` answer and change
//! nothing, a refused setting changes nothing, and a saved plan is read back
//! by the next run from a board that writes `snxpcb/1.12`.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use support::{jsonl, pins_fixture, run_cli};

/// A Standard two-layer board with no plan, listed as the project's board.
fn write_board(project: &Path, path: &Path) {
    let board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write board");
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.pcb_file = Some("board.snxpcb".into());
    signex_types::project::write_project(project, &data).expect("project");
}

fn call(id: &str, args: Value) -> Value {
    json!({"id": id, "args": args})
}

fn board() -> Value {
    json!({"document": "board.snxpcb"})
}

fn with(extra: Value) -> Value {
    let mut args = board();
    for (k, v) in extra.as_object().expect("an object") {
        args[k] = v.clone();
    }
    args
}

#[test]
fn the_plan_is_set_read_judged_undone_and_saved_through_the_cli() {
    let (_guard, root) = support::temp_root();
    let fx = pins_fixture(&root);
    let path = fx.dir.join("board.snxpcb");
    write_board(&fx.project, &path);
    // `--keep-going`: the refused setting of another stage does not end the
    // run, so the undo and the read after it run too.
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--keep-going"],
        jsonl(&[
            call("open_document", json!({"path": "board.snxpcb"})),
            call(
                "layout_plan_set",
                with(json!({"stage": "placement", "weights": "connection:3,edges:5"})),
            ),
            call("layout_plan_get", board()),
            call("layout_plan_status", board()),
            // A setting of another stage is refused and changes nothing.
            call(
                "layout_plan_set",
                with(json!({"stage": "routing", "weights": "edges:1"})),
            ),
            call("undo", board()),
            call("layout_plan_get", board()),
        ]),
    );
    let line = |n: usize| &out.lines[n];
    let done = |n: usize| matches!(line(n)["status"].as_str(), Some("ran" | "answered"));
    assert!(done(0), "the board opens: {}", out.stdout);

    let set = line(1);
    assert_eq!(set["status"], "answered", "{}", out.stdout);
    assert_eq!(set["data"]["changed"], true, "{}", out.stdout);
    assert_eq!(set["data"]["stage"], "placement");
    assert_eq!(
        set["data"]["stages"][0]["settings"]["weights"],
        json!({"connection": 3.0, "edges": 5.0}),
        "{}",
        out.stdout
    );
    assert_eq!(set["changed"].as_array().map(Vec::len), Some(1));

    let read = line(2);
    assert_eq!(read["status"], "answered", "{}", out.stdout);
    assert_eq!(read["data"]["stages"], set["data"]["stages"]);
    assert_eq!(read["changed"], json!([]), "a read changes nothing");

    let status = line(3);
    assert_eq!(status["status"], "answered", "{}", out.stdout);
    let stages: Vec<&str> = status["data"]["stages"]
        .as_array()
        .expect("stages")
        .iter()
        .map(|s| s["stage"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(stages, ["placement", "routing", "pours", "checks"]);
    assert_eq!(status["data"]["stages"][0]["set"], true);
    assert_eq!(status["changed"], json!([]));

    let refused = line(4);
    assert_eq!(refused["status"], "refused", "{}", out.stdout);
    let reason = refused["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("placement stage"), "{reason}");

    assert!(done(5), "undo runs: {}", out.stdout);
    let after = line(6);
    assert_eq!(after["data"]["empty"], true, "one undo takes the set back");
}

#[test]
fn a_saved_plan_is_read_back_by_the_next_run() {
    let (_guard, root) = support::temp_root();
    let fx = pins_fixture(&root);
    let path = fx.dir.join("board.snxpcb");
    write_board(&fx.project, &path);
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project, "--save"],
        jsonl(&[
            call("open_document", json!({"path": "board.snxpcb"})),
            call(
                "layout_plan_set",
                with(json!({"stage": "routing", "nets": "class:Power,GND", "skip": "NC_1"})),
            ),
            call("save_document", board()),
        ]),
    );
    assert_eq!(out.code, Some(0), "{}\n{}", out.stdout, out.stderr);
    let text = std::fs::read_to_string(&path).expect("board");
    assert!(
        text.starts_with("format = \"snxpcb/1.12\""),
        "a plan writes 1.12: {text}"
    );

    let again = run_cli(
        &["run", project],
        jsonl(&[
            call("open_document", json!({"path": "board.snxpcb"})),
            call("layout_plan_get", board()),
        ]),
    );
    let read = &again.lines[1];
    assert_eq!(read["status"], "answered", "{}", again.stdout);
    assert_eq!(
        read["data"]["stages"][1]["settings"],
        json!({"nets": ["class:Power", "GND"], "skip": ["NC_1"]}),
        "{}",
        again.stdout
    );
    // The plan names what the board does not have: said, not refused.
    let warnings = read["data"]["warnings"].as_array().expect("warnings");
    assert!(!warnings.is_empty(), "{}", again.stdout);
}
