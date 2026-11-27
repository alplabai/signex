//! End to end (#1273): `query_cross_references` through the real `signex-cli`
//! binary names the page a Global label's net continues on, reading the page
//! that is not open from disk, and writes nothing.

mod support;

use std::path::Path;

use serde_json::json;
use signex_types::project::{SheetEntry, parse_project, write_project};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Wire};
use support::{Fixture, fixture, jsonl, run_cli, snapshot, temp_root, without_journals};

/// The fixture project (GND, a Global label on `main.snxsch`) plus a second
/// listed page whose Global label GND sits on a wire at (100, 80): one net on
/// two pages.
fn two_pages(root: &Path) -> Fixture {
    let fx = fixture(root);
    let at = Point::new(100.0, 80.0);
    let content = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        wires: vec![Wire {
            uuid: uuid::Uuid::now_v7(),
            start: at,
            end: Point::new(110.0, 80.0),
            ..Default::default()
        }],
        labels: vec![Label::new("GND", at, LabelType::Global)],
        ..Default::default()
    };
    signex_engine::Engine::new_with_path(content, Some(fx.dir.join("second.snxsch")))
        .expect("engine")
        .save()
        .expect("seed the second page");
    let mut data = parse_project(&fx.project).expect("read the project");
    data.sheets.push(SheetEntry {
        name: "Second".into(),
        filename: "second.snxsch".into(),
        symbols_count: 0,
        wires_count: 1,
        labels_count: 1,
        unknown: Default::default(),
    });
    write_project(&fx.project, &data).expect("list the second page");
    fx
}

#[test]
fn a_global_label_names_the_page_its_net_continues_on() {
    let (_guard, root) = temp_root();
    let fx = two_pages(&root);
    let project = fx.project.to_str().unwrap();
    let before = without_journals(snapshot(&root));
    let out = run_cli(
        &["run", project],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            json!({"id": "query_cross_references", "args": {"document": "main.snxsch"}}),
        ]),
    );
    let answer = &out.lines[1];
    assert_eq!(answer["status"], "answered", "{}", out.stdout);
    let labels = answer["data"]["labels"].as_array().expect("labels");
    assert_eq!(labels.len(), 1, "{}", out.stdout);
    assert_eq!(labels[0]["text"], "GND", "{}", out.stdout);
    let placement = &labels[0]["placements"][0];
    assert_eq!(placement["targets"][0]["page"], 2, "{}", out.stdout);
    assert_eq!(
        placement["targets"][0]["position"],
        json!([100.0, 80.0]),
        "{}",
        out.stdout
    );
    let text = placement["cross_reference"].as_str().unwrap_or_default();
    assert!(text.starts_with("[2"), "{}", out.stdout);
    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}

#[test]
fn a_sheet_that_is_not_open_is_refused_with_what_to_do() {
    let (_guard, root) = temp_root();
    let fx = two_pages(&root);
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project, "--keep-going"],
        jsonl(&[json!({
            "id": "query_cross_references",
            "args": {"document": "second.snxsch"}
        })]),
    );
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    let reason = out.lines[0]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("open_document"), "{reason}");
}
