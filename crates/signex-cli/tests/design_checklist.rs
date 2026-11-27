//! End to end (#1291): the design checklist through the real `signex-cli`
//! binary. It is read, started from a template, edited (a waive without a
//! reason is refused), its checks are run, and a saved project reads back the
//! same checklist; a run without `--save` writes nothing. Past the edits: the
//! Mine filter, the link picker's rows, a link's Open, one item's Run check,
//! Save as template and the template list, Export CSV, Reset from template
//! and Import from another project.

mod support;

use serde_json::{Value, json};
use signex_types::project::{ChecklistStage, EditStamp, NewChecklistItem};
use support::{Fixture, jsonl, pins_fixture, run_cli, snapshot, temp_root};

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

/// Another project in the fixture's folder, `old/old.snxprj`, whose
/// checklist has two layout items: what an import reads.
fn old_project(fx: &Fixture) -> std::path::PathBuf {
    let mut data = signex_types::project::parse_project(&fx.project).expect("the fixture");
    let item = |title: &str| NewChecklistItem {
        stage: ChecklistStage::Layout,
        title: title.into(),
        link: None,
    };
    data.start_design_checklist(
        Some("blank"),
        vec![item("Hot loop short"), item("Clearance checked")],
        &EditStamp::default(),
    )
    .expect("a checklist");
    let path = fx.dir.join("old").join("old.snxprj");
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    signex_types::project::write_project(&path, &data).expect("the old project");
    path
}

#[test]
fn templates_files_and_links_reach_the_cli() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    old_project(&fx);
    let project = fx.project.to_str().unwrap();
    let doc = "demo.snxprj";
    let session = [
        json!({"id": "project_start_checklist",
               "args": {"document": doc, "template": "prototype_build"}}),
        json!({"id": "project_set_checklist_item",
               "args": {"document": doc, "item": "2", "field": "owner", "value": "jb"}}),
        json!({"id": "project_checklist",
               "args": {"document": doc, "filter": "mine", "owner": "jb"}}),
        json!({"id": "project_checklist_link_choices", "args": {"document": doc}}),
        json!({"id": "project_set_checklist_item",
               "args": {"document": doc, "item": "4", "field": "link",
                        "value": "object:sheet:main.snxsch"}}),
        json!({"id": "project_open_checklist_link", "args": {"document": doc, "item": "4"}}),
        json!({"id": "project_run_checklist", "args": {"document": doc, "item": "3"}}),
        json!({"id": "project_save_checklist_template",
               "args": {"document": doc, "name": "Fixture review"}}),
        json!({"id": "project_checklist_templates", "args": {"document": doc}}),
        json!({"id": "project_export_checklist_csv",
               "args": {"document": doc, "path": "checklist.csv"}}),
        json!({"id": "project_reset_checklist", "args": {"document": doc}}),
        json!({"id": "project_import_checklist",
               "args": {"document": doc, "path": "old/old.snxprj"}}),
        json!({"id": "save_document", "args": {"document": doc}}),
    ];
    let out = run_cli(
        &[
            "run",
            project,
            "--save",
            "--allow",
            "export",
            "--keep-going",
        ],
        jsonl(&session),
    );
    let (o, e) = (&out.stdout, &out.stderr);
    assert_eq!(
        statuses(&out.lines),
        [
            "answered", "answered", "answered", "answered", "answered", "answered", "ran",
            "answered", "answered", "answered", "answered", "refused", "ran"
        ],
        "{o}\n{e}"
    );
    let mine = &out.lines[2]["data"];
    assert_eq!(mine["shown"], json!(["2"]), "{o}");
    assert_eq!(mine["chips"]["mine"], 1, "{o}");
    let choices = out.lines[3]["data"]["choices"].as_array().cloned();
    let choices = choices.unwrap_or_default();
    assert!(
        choices.contains(&json!({"group": "sheet", "link": "object:sheet:main.snxsch"})),
        "{o}"
    );
    assert!(
        choices.contains(&json!({"group": "check", "link": "erc"})),
        "{o}"
    );
    assert_eq!(out.lines[5]["data"]["opens"], "sheet", "{o}");
    assert_eq!(
        out.lines[7]["data"]["template"], "user:Fixture review",
        "{o}"
    );
    let saved = out.lines[8]["data"]["saved"].as_array().cloned();
    assert!(
        saved
            .unwrap_or_default()
            .iter()
            .any(|t| t["template"] == "user:Fixture review" && t["items"] == 6),
        "{o}"
    );
    assert_eq!(out.lines[9]["data"]["rows"], 6, "{o}");
    let csv = std::fs::read_to_string(fx.dir.join("checklist.csv")).expect("the CSV");
    assert!(csv.starts_with("id,stage,state,title,owner,link,reason,notes,updated\r\n"));
    assert!(
        csv.contains("2,schematic,open,Title block filled in,jb,title_block,"),
        "{csv}"
    );
    assert_eq!(out.lines[10]["data"]["changed"], true, "{o}");
    let reason = out.lines[11]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("already"), "{reason}");

    let saved = signex_types::project::parse_project(&fx.project).expect("the saved project");
    let checklist = saved
        .design_checklist()
        .expect("readable")
        .expect("a checklist");
    assert_eq!(checklist.items.len(), 6);
    assert_eq!(
        checklist.items[1].owner, None,
        "the reset put the template back"
    );
}

#[test]
fn an_import_reads_another_projects_checklist_and_a_template_stays_in_its_run() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    old_project(&fx);
    let project = fx.project.to_str().unwrap();
    let doc = "demo.snxprj";
    let session = [
        json!({"id": "project_start_checklist",
               "args": {"document": doc, "template": "user:Fixture review"}}),
        json!({"id": "project_import_checklist",
               "args": {"document": doc, "path": "old/old.snxprj"}}),
        json!({"id": "save_document", "args": {"document": doc}}),
    ];
    let out = run_cli(&["run", project, "--save", "--keep-going"], jsonl(&session));
    let (o, e) = (&out.stdout, &out.stderr);
    assert_eq!(
        statuses(&out.lines),
        ["refused", "answered", "ran"],
        "{o}\n{e}"
    );
    // Each run has a config folder of its own: no template carries over.
    let reason = out.lines[0]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("Fixture review"), "{reason}");
    let imported = &out.lines[1]["data"];
    assert_eq!(imported["items"][0]["state"], "open", "{o}");
    assert_eq!(
        imported["items"][0]["history"][0]["event"], "imported",
        "{o}"
    );
    assert_eq!(imported["items"][0]["history"][0]["value"], "old", "{o}");

    let saved = signex_types::project::parse_project(&fx.project).expect("the saved project");
    let checklist = saved
        .design_checklist()
        .expect("readable")
        .expect("a checklist");
    let titles: Vec<&str> = checklist.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Hot loop short", "Clearance checked"]);
}
