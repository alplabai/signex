//! End to end (#1291): the design checklist through the real `signex-cli`
//! binary. It is read, started from a template, edited (a waive without a
//! reason is refused), its checks are run, and a saved project reads back the
//! same checklist; a run without `--save` writes nothing.

mod support;

use serde_json::{Value, json};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root};

fn statuses(lines: &[Value]) -> Vec<&str> {
    lines
        .iter()
        .map(|line| line["status"].as_str().unwrap_or_default())
        .collect()
}

fn session() -> Vec<Value> {
    let doc = "demo.snxprj";
    vec![
        json!({"id": "project_checklist", "args": {"document": doc}}),
        json!({"id": "project_start_checklist",
               "args": {"document": doc, "template": "prototype_build"}}),
        json!({"id": "project_set_checklist_state",
               "args": {"document": doc, "item": "4", "state": "waived"}}),
        json!({"id": "project_set_checklist_state",
               "args": {"document": doc, "item": "4", "state": "waived",
                        "reason": "No enclosure"}}),
        json!({"id": "project_add_checklist_item",
               "args": {"document": doc, "stage": "fabrication", "text": "Stencil ordered"}}),
        json!({"id": "project_run_checklist", "args": {"document": doc}}),
        json!({"id": "save_document", "args": {"document": doc}}),
    ]
}

#[test]
fn the_checklist_is_started_edited_and_saved_through_the_cli() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project, "--save", "--keep-going"],
        jsonl(&session()),
    );
    assert_eq!(
        statuses(&out.lines),
        [
            "answered", "answered", "refused", "answered", "answered", "ran", "ran"
        ],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
    assert_eq!(out.lines[0]["data"]["started"], false, "{}", out.stdout);
    let started = &out.lines[1]["data"];
    assert_eq!(started["template"], "prototype_build", "{}", out.stdout);
    assert_eq!(
        started["items"][0]["link"],
        json!({"kind": "erc"}),
        "{}",
        out.stdout
    );
    let reason = out.lines[2]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("reason"), "{reason}");
    assert_eq!(
        out.lines[3]["data"]["counts"]["waived"], 1,
        "{}",
        out.stdout
    );
    assert_eq!(out.lines[4]["data"]["item"], "7", "{}", out.stdout);

    let saved = signex_types::project::parse_project(&fx.project).expect("the saved project");
    let checklist = saved
        .design_checklist()
        .expect("readable")
        .expect("a checklist");
    assert_eq!(checklist.items.len(), 7);
    assert_eq!(checklist.items[3].reason.as_deref(), Some("No enclosure"));
    assert_eq!(checklist.items[6].title, "Stencil ordered");
}

#[test]
fn a_run_without_save_writes_nothing() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let before = snapshot(&root);
    let project = fx.project.to_str().unwrap();
    let mut commands = session();
    commands.pop();
    let out = run_cli(&["run", project, "--keep-going"], jsonl(&commands));
    assert_eq!(
        statuses(&out.lines),
        [
            "answered", "answered", "refused", "answered", "answered", "ran"
        ],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
    assert_eq!(snapshot(&root), before, "nothing is written without --save");
}
