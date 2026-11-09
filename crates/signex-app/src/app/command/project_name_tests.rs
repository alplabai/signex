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
    let reason = refusal(run(
        &mut app,
        "place_global_label",
        &["at=5.08,0", "text=GND", "allow_merge=true"],
    ));
    assert!(reason.contains("cannot be read completely"), "{reason}");
    assert!(reason.contains("child.snxsch is missing"), "{reason}");
    // A global label reaches other sheets by its own name: refused too. A
    // net label on a net nothing links outward is allowed.
    let reason = refusal(run(
        &mut app,
        "place_global_label",
        &["at=5.08,40.64", "text=FREE"],
    ));
    assert!(reason.contains("cannot be read completely"), "{reason}");
    let outcome = run(&mut app, "place_net_label", &["at=5.08,40.64", "text=FREE"]);
    assert_eq!(outcome, CommandOutcome::Ran);
}
