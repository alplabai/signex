//! #2199 — the name side of the intent checks is judged on the whole project:
//! a name that reaches a net from another sheet counts.

use std::path::Path;

use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    ChildSheet, FillType, Label, LabelType, Point, SchematicSheet, SheetPin, Symbol, Wire,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

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

/// The parent: a wire from the sheet pin `SIG` at (0,0) to (5.08,0), and a
/// second wire (10.16,0)-(15.24,0) named `X`; and a free wire at y = 40.64.
fn parent() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "U".into(),
        filename: "child.snxsch".into(),
        position: Point::new(0.0, 60.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: Uuid::now_v7(),
            name: "SIG".into(),
            direction: "input".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    sheet.wires.push(wire((0.0, 0.0), (5.08, 0.0)));
    sheet.wires.push(wire((10.16, 0.0), (15.24, 0.0)));
    sheet.labels.push(global("X", (15.24, 0.0)));
    sheet.wires.push(wire((0.0, 40.64), (5.08, 40.64)));
    sheet
}

/// The child: `SIG` on a wire that also carries the `VCC` power port.
fn child() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(wire((0.0, 0.0), (10.16, 0.0)));
    sheet.labels.push(Label::new(
        "SIG",
        Point::new(0.0, 0.0),
        LabelType::Hierarchical,
    ));
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "power:VCC".into(),
        reference: "#PWR01".into(),
        value: "VCC".into(),
        is_power: true,
        position: Point::new(10.16, 0.0),
        unit: 1,
        ..Symbol::default()
    });
    sheet
}

fn write(path: &Path, sheet: SchematicSheet) {
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// An app with the project open and its parent sheet the edit target.
fn project_app(dir: &tempfile::TempDir) -> Signex {
    let _ = crate::diagnostics::init_logging();
    write(&dir.path().join("top.snxsch"), parent());
    write(&dir.path().join("child.snxsch"), child());
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.ui_state.snap_enabled = false;
    app.open_or_focus_sheet_path(dir.path().join("top.snxsch"));
    app.refresh_panel_ctx();
    app
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    app.run_operands(&id, &args).0
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_label_that_gives_a_net_a_name_from_another_sheet_a_third_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    // SIG (the sheet pin and the child's label) and VCC (the child's port).
    let reason = refusal(run(
        &mut app,
        "place_global_label",
        &["at=5.08,0", "text=GND"],
    ));
    assert!(reason.contains("reach it from other sheets"), "{reason}");
    let outcome = run(
        &mut app,
        "place_global_label",
        &["at=5.08,0", "text=GND", "allow_merge=true"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_wire_that_joins_a_net_with_a_name_from_another_sheet_to_a_named_net_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let ends = ["start=5.08,0", "end=10.16,0"];
    let reason = refusal(run(&mut app, "place_wire", &ends));
    assert!(reason.contains("reach it from other sheets"), "{reason}");
    let outcome = run(
        &mut app,
        "place_wire",
        &[ends[0], ends[1], "allow_merge=true"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_placement_on_a_net_with_no_names_from_elsewhere_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let outcome = run(
        &mut app,
        "place_global_label",
        &["at=5.08,40.64", "text=FREE"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn an_unreadable_project_refuses_what_reaches_other_sheets_and_allows_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    std::fs::remove_file(dir.path().join("child.snxsch")).unwrap();
    let label = ["at=5.08,0", "text=GND"];
    let reason = refusal(run(&mut app, "place_global_label", &label));
    assert!(reason.contains("cannot be read completely"), "{reason}");
    assert!(reason.contains("child.snxsch is missing"), "{reason}");
    // The maintainer's decision: allow_merge places it anyway.
    let outcome = run(
        &mut app,
        "place_global_label",
        &[label[0], label[1], "allow_merge=true"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    // A power port reaches other sheets by its name: refused, then allowed.
    let port = ["at=5.08,40.64", "net_name=GND", "lib_id=power:GND"];
    let reason = refusal(run(&mut app, "place_power_symbol", &port));
    assert!(reason.contains("cannot be read completely"), "{reason}");
    let outcome = run(
        &mut app,
        "place_power_symbol",
        &[port[0], port[1], port[2], "allow_merge=true"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    // A no-connect never joins nets or adds a name: it is not judged here.
    let outcome = run(&mut app, "place_no_connect", &["at=2.54,0"]);
    assert_eq!(outcome, CommandOutcome::Ran);
    // A net label on a net nothing links outward is allowed.
    let outcome = run(&mut app, "place_net_label", &["at=5.08,20.32", "text=FREE"]);
    assert_eq!(outcome, CommandOutcome::Ran);
}

#[test]
fn a_sheet_opened_under_another_spelling_than_the_parent_writes_is_still_checked() {
    let dir = tempfile::tempdir().unwrap();
    // The parent writes `Child.snxsch`; the file and its tab are `child.snxsch`.
    let mut top = SchematicSheet::default();
    top.child_sheets
        .push(sheet_symbol("U", "Child.snxsch", &[]));
    // The child: a wire on the entry S of its own sheet symbol G (so, across
    // the project, named S and Q by g.snxsch, though unnamed on this sheet),
    // and a wire named R.
    let mut inner = SchematicSheet::default();
    inner
        .child_sheets
        .push(sheet_symbol("G", "g.snxsch", &[("S", (0.0, 0.0))]));
    inner.wires.push(wire((0.0, 0.0), (10.16, 0.0)));
    inner.wires.push(wire((0.0, 20.32), (10.16, 20.32)));
    inner
        .labels
        .push(Label::new("R", Point::new(5.08, 20.32), LabelType::Net));
    let mut grand = child_with(&["S"]);
    grand
        .labels
        .push(Label::new("Q", Point::new(5.08, 0.0), LabelType::Net));
    write(&dir.path().join("top.snxsch"), top);
    write(&dir.path().join("child.snxsch"), inner);
    write(&dir.path().join("g.snxsch"), grand);
    if !dir.path().join("Child.snxsch").exists() {
        return; // a case-sensitive volume: the spelling is a different file
    }
    // The project lists the child as a page under its own spelling.
    let _ = crate::diagnostics::init_logging();
    let mut app = app_workspace(
        &dir.path().to_string_lossy(),
        &["top.snxsch", "child.snxsch"],
    );
    // This volume ignores case, so the project keys `Child` and `child` as one.
    app.document_state.projects[0].case_fold = crate::app::state::scope::CaseFold::FOLD;
    app.ui_state.snap_enabled = false;
    app.open_or_focus_sheet_path(dir.path().join("child.snxsch"));
    app.refresh_panel_ctx();
    let reason = refusal(run(
        &mut app,
        "place_wire",
        &["start=10.16,0", "end=10.16,20.32"],
    ));
    assert!(reason.contains("reach it from other sheets"), "{reason}");
}

fn bus(a: (f64, f64), b: (f64, f64)) -> signex_types::schematic::Bus {
    signex_types::schematic::Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
    }
}

#[test]
fn a_bus_run_is_judged_on_the_project_like_a_wire() {
    // A bus run is judged on the project like a wire: from the end of a bus
    // that a hierarchical label takes to another sheet, in a project that
    // cannot be read (a join of member nets across sheets needs the same
    // stitch; two member nets of one bus carry one name on a sheet, so the
    // unreadable case is the observable one).
    let dir = tempfile::tempdir().unwrap();
    let mut top = SchematicSheet::default();
    top.buses.push(bus((0.0, 0.0), (0.0, 20.32)));
    top.labels.push(Label::new(
        "D[0..3]",
        Point::new(0.0, 20.32),
        LabelType::Hierarchical,
    ));
    top.child_sheets.push(sheet_symbol("U", "gone.snxsch", &[]));
    write(&dir.path().join("top.snxsch"), top);
    let mut app = open_project(&dir);
    let run_from = ["start=0,20.32", "end=20.32,20.32", "allow_open=true"];
    let reason = refusal(run(&mut app, "place_bus", &run_from));
    assert!(reason.contains("cannot be read completely"), "{reason}");
    let outcome = run(
        &mut app,
        "place_bus",
        &[run_from[0], run_from[1], run_from[2], "allow_merge=true"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn an_unreadable_listed_page_and_a_refused_child_reference_count_as_incomplete() {
    for (file, text) in [("child.snxsch", "not a sheet"), ("child.snxsch", "")] {
        let dir = tempfile::tempdir().unwrap();
        let mut app = project_app(&dir);
        std::fs::write(dir.path().join(file), text).unwrap();
        let reason = refusal(run(
            &mut app,
            "place_global_label",
            &["at=5.08,0", "text=GND"],
        ));
        assert!(reason.contains("child.snxsch is unreadable"), "{reason}");
    }
    // A child reference that leaves the project folder.
    let dir = tempfile::tempdir().unwrap();
    let mut top = parent();
    top.child_sheets
        .push(sheet_symbol("Away", "../outside_2199.snxsch", &[]));
    write(&dir.path().join("top.snxsch"), top);
    write(&dir.path().join("child.snxsch"), child());
    let mut app = open_project(&dir);
    let reason = refusal(run(
        &mut app,
        "place_global_label",
        &["at=5.08,0", "text=GND"],
    ));
    assert!(reason.contains("leaves its folder"), "{reason}");
}

/// A sheet symbol named `name` on `file`, its entries `(name, at)`.
fn sheet_symbol(name: &str, file: &str, entries: &[(&str, (f64, f64))]) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: name.into(),
        filename: file.into(),
        position: Point::new(0.0, 80.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: entries
            .iter()
            .map(|(pin, at)| SheetPin {
                uuid: Uuid::now_v7(),
                name: (*pin).into(),
                direction: "input".into(),
                position: Point::new(at.0, at.1),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            })
            .collect(),
        instances: Vec::new(),
    }
}

/// The app with the project of `dir` open and its `top.snxsch` the target.
fn open_project(dir: &tempfile::TempDir) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.ui_state.snap_enabled = false;
    app.open_or_focus_sheet_path(dir.path().join("top.snxsch"));
    app.refresh_panel_ctx();
    app
}

fn hier(text: &str, at: (f64, f64)) -> Label {
    Label::new(text, Point::new(at.0, at.1), LabelType::Hierarchical)
}

/// A child with each of `names` as a hierarchical label on a wire of its own
/// at y = 20.32 * index.
fn child_with(names: &[&str]) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    for (i, name) in names.iter().enumerate() {
        let y = 20.32 * i as f64;
        sheet.wires.push(wire((0.0, y), (10.16, y)));
        sheet.labels.push(hier(name, (0.0, y)));
    }
    sheet
}

#[test]
fn two_entries_named_only_by_the_child_with_different_names_are_a_rename() {
    let dir = tempfile::tempdir().unwrap();
    let mut top = SchematicSheet::default();
    let entries = [("A", (0.0, 0.0)), ("B", (0.0, 20.32))];
    top.child_sheets
        .push(sheet_symbol("U", "c.snxsch", &entries));
    top.wires.push(wire((0.0, 0.0), (5.08, 0.0)));
    top.wires.push(wire((0.0, 20.32), (5.08, 20.32)));
    // Elsewhere one net carries the globals A and B, on purpose.
    top.wires.push(wire((0.0, 60.96), (10.16, 60.96)));
    top.labels.push(global("A", (0.0, 60.96)));
    top.labels.push(global("B", (10.16, 60.96)));
    write(&dir.path().join("top.snxsch"), top);
    write(&dir.path().join("c.snxsch"), child_with(&["A", "B"]));
    let mut app = open_project(&dir);
    // Alias-only nets A and B (the globals elsewhere change nothing): two
    // signals made one.
    let ends = ["start=5.08,0", "end=5.08,20.32"];
    let reason = refusal(run(&mut app, "place_wire", &ends));
    assert!(reason.contains("{A, B}"), "{reason}");
    let outcome = run(
        &mut app,
        "place_wire",
        &[ends[0], ends[1], "allow_merge=true"],
    );
    let CommandOutcome::Answered(report) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(report["aliases"], serde_json::json!(["A", "B"]));
    assert_eq!(report["merged"], serde_json::json!(["A", "B"]));
}

#[test]
fn two_nets_with_different_own_names_are_a_rename_across_the_project() {
    // The parent's nets are named SDA_A and SDA_B by net labels; the entries
    // carry the child's aliases. A wire between them is refused.
    let dir = tempfile::tempdir().unwrap();
    let mut top = SchematicSheet::default();
    let entries = [("A", (0.0, 0.0)), ("B", (0.0, 20.32))];
    top.child_sheets
        .push(sheet_symbol("U", "c.snxsch", &entries));
    top.wires.push(wire((0.0, 0.0), (5.08, 0.0)));
    top.wires.push(wire((0.0, 20.32), (5.08, 20.32)));
    for (text, y) in [("SDA_A", 0.0), ("SDA_B", 20.32)] {
        top.labels
            .push(Label::new(text, Point::new(5.08, y), LabelType::Net));
    }
    write(&dir.path().join("top.snxsch"), top);
    write(&dir.path().join("c.snxsch"), child_with(&["A", "B"]));
    let mut app = open_project(&dir);
    let ends = ["start=5.08,0", "end=5.08,20.32"];
    let reason = refusal(run(&mut app, "place_wire", &ends));
    assert!(reason.contains("allow_merge"), "{reason}");
    let outcome = run(
        &mut app,
        "place_wire",
        &[ends[0], ends[1], "allow_merge=true"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn two_placements_of_one_child_join_freely_on_the_same_entry_and_not_on_different_ones() {
    let ends = ["start=5.08,0", "end=5.08,20.32"];
    for (second, rename) in [("SIG", false), ("SCL", true)] {
        let dir = tempfile::tempdir().unwrap();
        let mut top = SchematicSheet::default();
        for (name, entry, y) in [("U1", "SIG", 0.0), ("U2", second, 20.32)] {
            top.child_sheets
                .push(sheet_symbol(name, "c.snxsch", &[(entry, (0.0, y))]));
            top.wires.push(wire((0.0, y), (5.08, y)));
        }
        write(&dir.path().join("top.snxsch"), top);
        write(&dir.path().join("c.snxsch"), child_with(&["SIG", "SCL"]));
        let mut app = open_project(&dir);
        let outcome = run(&mut app, "place_wire", &ends);
        if rename {
            let reason = refusal(outcome);
            assert!(reason.contains("{SCL, SIG}"), "{reason}");
        } else {
            // The same entry of both placements is a shared bus.
            assert!(
                matches!(outcome, CommandOutcome::Answered(_)),
                "{outcome:?}"
            );
        }
    }
}

#[test]
fn an_edit_list_judges_each_edit_on_the_project_it_reads_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let path = dir.path().join("top.snxsch");
    let revision = |app: &Signex| app.document_state.engines[&path].revision();
    let apply = |app: &mut Signex, short: serde_json::Value| {
        let edits = serde_json::json!([
            {"id": "place_global_label", "args": {"at": [5.08, 40.64], "text": "FREE"}},
            {"id": "place_wire", "args": short},
        ]);
        run(
            app,
            "apply_edits",
            &[
                &format!("document={}", path.display()),
                &format!("expect_revision={}", revision(app)),
                &format!("edits={edits}"),
            ],
        )
    };
    let before = revision(&app);
    let reason = refusal(apply(
        &mut app,
        serde_json::json!({"start": [5.08, 0], "end": [10.16, 0]}),
    ));
    assert!(reason.contains("edit 2"), "{reason}");
    assert!(reason.contains("reach it from other sheets"), "{reason}");
    assert_eq!(revision(&app), before, "a refused list changes nothing");
    let allowed = serde_json::json!({"start": [5.08, 0], "end": [10.16, 0], "allow_merge": true});
    assert!(matches!(
        apply(&mut app, allowed),
        CommandOutcome::Answered(_)
    ));
}

#[test]
fn a_dry_run_in_a_project_writes_nothing_to_messages() {
    let dir = tempfile::tempdir().unwrap();
    let mut top = parent();
    // A reference the read refuses, once per read; named so the test finds it.
    top.child_sheets
        .push(sheet_symbol("Away", "../zz_unique_2199_away.snxsch", &[]));
    write(&dir.path().join("top.snxsch"), top);
    write(&dir.path().join("child.snxsch"), child());
    let mut app = open_project(&dir);
    let mark = crate::diagnostics::newest_entry_id();
    let outcome = run(
        &mut app,
        "place_wire",
        &[
            "start=5.08,0",
            "end=10.16,0",
            "dry_run=true",
            "allow_merge=true",
        ],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .filter(|m| m.contains("zz_unique_2199_away") || m.contains("child-sheet reference"))
        .collect();
    assert!(said.is_empty(), "{said:?}");
}
