//! #2199 review round 4: a sheet symbol from operands judged against the
//! project it joins: a loop through its file's subtree, and the names its
//! file brings in (a short or a rename needs `allow_merge`); a child-to-be is
//! no page of its own while an earlier edit of the list is judged.

use std::path::Path;

use serde_json::{Value, json};
use signex_types::schematic::{ChildSheet, Label, LabelType, Point, SchematicSheet, Wire};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

fn save(dir: &Path, name: &str, sheet: SchematicSheet) {
    signex_engine::Engine::new_with_path(sheet, Some(dir.join(name)))
        .expect("engine")
        .save()
        .expect("write sheet");
}

fn sheet(wires: Vec<Wire>, labels: Vec<Label>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        wires,
        labels,
        ..Default::default()
    }
}

fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    }
}

fn global(text: &str, at: (f64, f64)) -> Label {
    Label::new(text, Point::new(at.0, at.1), LabelType::Global)
}

/// The project `top` (root, open, the target) and `other` (closed).
fn project(dir: &Path) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch", "other.snxsch"]);
    app.ui_state.snap_enabled = false;
    app.open_or_focus_sheet_path(dir.join("top.snxsch"));
    app
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let call = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    app.run_operands_settled(&id, &call).0
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_sheet_symbol_whose_file_places_this_sheet_is_a_loop() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    save(d, "top.snxsch", sheet(Vec::new(), Vec::new()));
    save(d, "other.snxsch", sheet(Vec::new(), Vec::new()));
    let up =
        signex_engine::child_sheet_from_corners(Point::new(0.0, 0.0), Point::new(20.0, 20.0), "Up")
            .unwrap();
    save(
        d,
        "loop.snxsch",
        SchematicSheet {
            uuid: Uuid::now_v7(),
            child_sheets: vec![ChildSheet {
                filename: "top.snxsch".into(),
                ..up
            }],
            ..Default::default()
        },
    );
    let mut app = project(d);
    let operands = ["start=50,50", "end=70,70", "name=L", "file=loop.snxsch"];
    let reason = refusal(run(&mut app, "place_sheet_symbol", &operands));
    assert!(reason.contains("contain itself"), "{reason}");
}

/// `other` has separate nets named by the globals VBUS and VBAT; the closed
/// `charger` has one wire carrying both.
fn shorting_child(dir: &Path) {
    save(dir, "top.snxsch", sheet(Vec::new(), Vec::new()));
    save(
        dir,
        "other.snxsch",
        sheet(
            vec![
                wire((0.0, 0.0), (10.0, 0.0)),
                wire((0.0, 20.0), (10.0, 20.0)),
            ],
            vec![global("VBUS", (0.0, 0.0)), global("VBAT", (0.0, 20.0))],
        ),
    );
    save(
        dir,
        "charger.snxsch",
        sheet(
            vec![wire((0.0, 0.0), (10.0, 0.0))],
            vec![global("VBUS", (0.0, 0.0)), global("VBAT", (10.0, 0.0))],
        ),
    );
}

#[test]
fn a_sheet_symbol_whose_file_shorts_two_project_nets_needs_allow_merge() {
    let dir = tempfile::tempdir().unwrap();
    shorting_child(dir.path());
    let mut app = project(dir.path());
    let operands = ["start=50,50", "end=70,70", "name=C", "file=charger.snxsch"];
    let reason = refusal(run(&mut app, "place_sheet_symbol", &operands));
    assert!(reason.contains("allow_merge"), "{reason}");
    let top = dir.path().join("top.snxsch");
    assert!(
        app.document_state.engines[&top]
            .document()
            .child_sheets
            .is_empty()
    );
}

#[test]
fn a_sheet_symbol_whose_file_shorts_two_project_nets_is_placed_with_allow_merge() {
    let dir = tempfile::tempdir().unwrap();
    shorting_child(dir.path());
    let mut app = project(dir.path());
    let operands = [
        "start=50,50",
        "end=70,70",
        "name=C",
        "file=charger.snxsch",
        "allow_merge=true",
    ];
    let outcome = run(&mut app, "place_sheet_symbol", &operands);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

/// The earlier edit of a list is judged on the project without the file a
/// later `place_sheet_symbol` brings in: that file is no top-level page.
#[test]
fn a_child_to_be_of_the_list_does_not_join_nets_for_an_earlier_edit() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    save(
        d,
        "top.snxsch",
        sheet(
            vec![
                wire((0.0, 0.0), (10.16, 0.0)),
                wire((20.32, 0.0), (30.48, 0.0)),
            ],
            vec![global("A", (0.0, 0.0)), global("B", (30.48, 0.0))],
        ),
    );
    save(d, "other.snxsch", sheet(Vec::new(), Vec::new()));
    save(
        d,
        "c.snxsch",
        sheet(
            vec![wire((0.0, 0.0), (10.0, 0.0))],
            vec![global("A", (0.0, 0.0)), global("B", (10.0, 0.0))],
        ),
    );
    let mut app = project(d);
    let path = d.join("top.snxsch");
    let revision = app.document_state.engines[&path].revision();
    let edits: Value = json!([
        {"id": "place_wire", "args": {"start": "10.16,0", "end": "20.32,0"}},
        {"id": "place_sheet_symbol", "args": {
            "start": "50,50", "end": "70,70", "name": "C", "file": "c.snxsch", "allow_merge": true}},
    ]);
    let document = format!("document={}", path.display());
    let expect = format!("expect_revision={revision}");
    let list = format!("edits={edits}");
    let reason = refusal(run(&mut app, "apply_edits", &[&document, &expect, &list]));
    assert!(reason.starts_with("edit 1"), "{reason}");
    assert!(reason.contains("allow_merge"), "{reason}");
}

/// The file is open too (an agent opened it to look): the project is read
/// with it all the same.
#[test]
fn a_sheet_symbol_whose_open_file_shorts_two_project_nets_needs_allow_merge() {
    let dir = tempfile::tempdir().unwrap();
    shorting_child(dir.path());
    let mut app = project(dir.path());
    app.open_or_focus_sheet_path(dir.path().join("other.snxsch"));
    app.open_or_focus_sheet_path(dir.path().join("charger.snxsch"));
    let top = dir.path().join("top.snxsch");
    let doc = format!("document={}", top.display());
    let operands = [
        "start=50,50",
        "end=70,70",
        "name=C",
        "file=charger.snxsch",
        &doc,
    ];
    let reason = refusal(run(&mut app, "place_sheet_symbol", &operands));
    assert!(reason.contains("allow_merge"), "{reason}");
    assert!(
        app.document_state.engines[&top]
            .document()
            .child_sheets
            .is_empty()
    );
}

#[test]
fn a_loose_sheet_symbol_whose_closed_file_places_this_sheet_is_a_loop() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    save(d, "a.snxsch", sheet(Vec::new(), Vec::new()));
    let up =
        signex_engine::child_sheet_from_corners(Point::new(0.0, 0.0), Point::new(20.0, 20.0), "Up")
            .unwrap();
    save(
        d,
        "b.snxsch",
        SchematicSheet {
            uuid: Uuid::now_v7(),
            child_sheets: vec![ChildSheet {
                filename: "a.snxsch".into(),
                ..up
            }],
            ..Default::default()
        },
    );
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = false;
    app.open_or_focus_sheet_path(d.join("a.snxsch"));
    let operands = ["start=50,50", "end=70,70", "name=L", "file=b.snxsch"];
    let reason = refusal(run(&mut app, "place_sheet_symbol", &operands));
    assert!(reason.contains("contain itself"), "{reason}");
}

/// `top` places `new.snxsch`, which is not there yet: the project is
/// incomplete.
fn incomplete_project(dir: &Path) {
    let pending = signex_engine::child_sheet_from_corners(
        Point::new(0.0, 0.0),
        Point::new(20.0, 20.0),
        "New",
    )
    .unwrap();
    save(
        dir,
        "top.snxsch",
        SchematicSheet {
            uuid: Uuid::now_v7(),
            child_sheets: vec![ChildSheet {
                filename: "new.snxsch".into(),
                ..pending
            }],
            ..Default::default()
        },
    );
    save(dir, "other.snxsch", sheet(Vec::new(), Vec::new()));
}

#[test]
fn a_sheet_symbol_whose_file_brings_no_names_is_placed_in_a_project_with_a_missing_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    incomplete_project(d);
    save(
        d,
        "plain.snxsch",
        sheet(vec![wire((0.0, 0.0), (10.0, 0.0))], Vec::new()),
    );
    let mut app = project(d);
    let operands = ["start=50,50", "end=70,70", "name=P", "file=plain.snxsch"];
    let outcome = run(&mut app, "place_sheet_symbol", &operands);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_sheet_symbol_whose_file_has_a_global_label_is_refused_in_a_project_with_a_missing_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    incomplete_project(d);
    save(
        d,
        "named.snxsch",
        sheet(
            vec![wire((0.0, 0.0), (10.0, 0.0))],
            vec![global("VBUS", (0.0, 0.0))],
        ),
    );
    let mut app = project(d);
    let operands = ["start=50,50", "end=70,70", "name=N", "file=named.snxsch"];
    let reason = refusal(run(&mut app, "place_sheet_symbol", &operands));
    assert!(reason.contains("new.snxsch is missing"), "{reason}");
    assert!(reason.contains("allow_merge"), "{reason}");
    assert!(!reason.contains("this net"), "{reason}");
}
