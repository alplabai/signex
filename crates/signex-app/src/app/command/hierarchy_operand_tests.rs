//! #2199 PR C — sheet symbols, sheet entries and Sync Sheet Pins from
//! operands: one undoable edit each, no file written, the connection rule and
//! the refusals.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Wire};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::canvas::placement_tests::grid;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;

/// A grid-snapping app with `sheet` active as `main.snxsch` in `dir`.
fn app_in(dir: &Path, sheet: SchematicSheet) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    add_sheet_with(&mut app, &dir.join("main.snxsch"), sheet);
    app
}

fn pt(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

fn run(app: &mut Signex, id: &str, operands: &[String]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&id, &refs).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

/// `Charger` from grid (20, 20) to (40, 40): its left border is x = 25.4.
fn sheet_symbol(app: &mut Signex, name: &str, file: &str) -> CommandOutcome {
    run(
        app,
        "place_sheet_symbol",
        &[
            format!("start={}", pt(grid(20.0, 20.0))),
            format!("end={}", pt(grid(40.0, 40.0))),
            format!("name={name}"),
            format!("file={file}"),
        ],
    )
}

fn entry(app: &mut Signex, sheet: &str, at: Point, name: &str) -> CommandOutcome {
    run(
        app,
        "place_sheet_entry",
        &[
            format!("sheet={sheet}"),
            format!("at={}", pt(at)),
            format!("name={name}"),
        ],
    )
}

/// A point on Charger's left border, `row` grid steps down.
fn left(row: f64) -> Point {
    grid(20.0, 20.0 + row)
}

#[test]
fn a_sheet_symbol_is_placed_from_corners_and_refers_to_its_file_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path(), SchematicSheet::default());
    let answer = answered(sheet_symbol(&mut app, "Charger", "charger.snxsch"));
    assert_eq!(answer["name"], "Charger");
    assert_eq!(answer["file"], "charger.snxsch");
    assert_eq!(answer["at"], json!([25.4, 25.4]));
    assert_eq!(answer["size"], json!([25.4, 25.4]));
    let placed = &doc(&app).child_sheets[0];
    assert_eq!(
        (placed.name.as_str(), placed.filename.as_str()),
        ("Charger", "charger.snxsch")
    );
    assert_eq!(placed.uuid.to_string(), answer["id"]);
    assert!(
        !dir.path().join("charger.snxsch").exists(),
        "no file is written"
    );
    // One undo takes it back.
    let undo = AppCommandId::new("undo").expect("id");
    let _ = app.dispatch_command_reporting(&undo, CommandArgs::none(), None);
    assert!(doc(&app).child_sheets.is_empty());
}

#[test]
fn a_sheet_symbol_file_outside_the_folder_or_off_grid_corners_are_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path(), SchematicSheet::default());
    for (file, needle) in [
        ("../outside.snxsch", "inside the project folder"),
        ("charger.txt", "must end in .snxsch"),
    ] {
        let reason = refusal(sheet_symbol(&mut app, "Charger", file));
        assert!(reason.contains(needle), "{file}: {reason}");
    }
    let off = run(
        &mut app,
        "place_sheet_symbol",
        &[
            "start=1,1".into(),
            format!("end={}", pt(grid(40.0, 40.0))),
            "name=Charger".into(),
            "file=charger.snxsch".into(),
        ],
    );
    assert!(refusal(off).contains("is not where a click lands"));
    answered(sheet_symbol(&mut app, "Charger", "charger.snxsch"));
    let twice = sheet_symbol(&mut app, "Other", "charger.snxsch");
    assert!(refusal(twice).contains("one file is placed once"));
    let same_name = sheet_symbol(&mut app, "Charger", "other.snxsch");
    assert!(refusal(same_name).contains("already on this sheet"));
    assert_eq!(doc(&app).child_sheets.len(), 1);
}

#[test]
fn an_entry_goes_on_the_border_and_a_pin_reference_finds_it_next() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path(), SchematicSheet::default());
    answered(sheet_symbol(&mut app, "Charger", "charger.snxsch"));
    let answer = answered(entry(&mut app, "Charger", left(4.0), "SDA"));
    assert_eq!(answer["name"], "SDA");
    assert_eq!(answer["direction"], "input", "the left edge's default");
    assert_eq!(answer["at"], json!([left(4.0).x, left(4.0).y]));
    let wire = run(
        &mut app,
        "place_wire",
        &[
            "start=Charger.SDA".into(),
            format!("end={}", pt(grid(10.0, 24.0))),
            "allow_open=true".into(),
        ],
    );
    assert!(matches!(wire, CommandOutcome::Answered(_)), "{wire:?}");
    let placed = &doc(&app).wires[0];
    assert!(placed.start == left(4.0) || placed.end == left(4.0));
}

#[test]
fn an_entry_off_the_border_twice_named_or_on_an_unknown_sheet_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path(), SchematicSheet::default());
    answered(sheet_symbol(&mut app, "Charger", "charger.snxsch"));
    let inside = refusal(entry(&mut app, "Charger", grid(30.0, 30.0), "SDA"));
    assert!(inside.contains("is not on the border"), "{inside}");
    let corner = refusal(entry(&mut app, "Charger", grid(20.0, 20.0), "SDA"));
    assert!(corner.contains("is not on the border"), "{corner}");
    answered(entry(&mut app, "Charger", left(4.0), "SDA"));
    let twice = refusal(entry(&mut app, "Charger", left(8.0), "SDA"));
    assert!(twice.contains("already has an entry `SDA`"));
    let unknown = refusal(entry(&mut app, "Nope", left(8.0), "SCL"));
    assert!(unknown.contains("no sheet symbol `Nope`"));
    let bad_direction = run(
        &mut app,
        "place_sheet_entry",
        &[
            "sheet=Charger".into(),
            format!("at={}", pt(left(8.0))),
            "name=SCL".into(),
            "direction=sideways".into(),
        ],
    );
    assert!(refusal(bad_direction).contains("`direction` must be"));
    assert_eq!(doc(&app).child_sheets[0].pins.len(), 1);
}

#[test]
fn an_entry_on_an_ambiguous_sheet_name_or_a_wire_end_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let symbol = |x: f64| {
        signex_engine::child_sheet_from_corners(grid(x, 20.0), grid(x + 20.0, 40.0), "Twin")
            .expect("big enough")
    };
    let sheet = SchematicSheet {
        child_sheets: vec![symbol(20.0), symbol(60.0)],
        ..Default::default()
    };
    let mut app = app_in(dir.path(), sheet);
    let twins = refusal(entry(&mut app, "Twin", left(4.0), "SDA"));
    assert!(twins.contains("2 sheet symbols"), "{twins}");

    let wired = SchematicSheet {
        child_sheets: vec![symbol(20.0)],
        wires: vec![Wire {
            uuid: Uuid::now_v7(),
            start: grid(10.0, 24.0),
            end: left(4.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut app = app_in(dir.path(), wired);
    let reason = refusal(entry(&mut app, "Twin", left(4.0), "SDA"));
    assert!(reason.contains("would land on another wire"), "{reason}");
}

/// Write the child sheet `file` in `dir` with hierarchical labels `names`.
fn write_child(dir: &Path, file: &str, names: &[&str]) -> PathBuf {
    let path = dir.join(file);
    let labels = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            Label::new(
                *name,
                Point::new(20.0, 20.0 + 5.08 * i as f64),
                LabelType::Hierarchical,
            )
        })
        .collect();
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        labels,
        ..Default::default()
    };
    signex_engine::Engine::new_with_path(sheet, Some(path.clone()))
        .expect("engine")
        .save()
        .expect("write child");
    path
}

fn sync(app: &mut Signex) -> CommandOutcome {
    run(app, "sync_sheet_pins", &["sheet=Charger".into()])
}

#[test]
fn sync_sheet_pins_adds_removes_and_keeps_entries_by_the_child_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_child(dir.path(), "charger.snxsch", &["SDA", "SCL"]);
    let mut app = app_in(dir.path(), SchematicSheet::default());
    answered(sheet_symbol(&mut app, "Charger", "charger.snxsch"));
    // No selection: the operand names the sheet symbol.
    let first = answered(sync(&mut app));
    assert_eq!(first["added"], json!(["SCL", "SDA"]));
    assert_eq!(first["removed"], json!([]));

    write_child(dir.path(), "charger.snxsch", &["SDA", "VIN"]);
    let second = answered(sync(&mut app));
    assert_eq!(second["added"], json!(["VIN"]));
    assert_eq!(second["removed"], json!(["SCL"]));
    assert_eq!(second["kept"], json!(["SDA"]));
    // One undo takes back one sync.
    let undo = AppCommandId::new("undo").expect("id");
    let _ = app.dispatch_command_reporting(&undo, CommandArgs::none(), None);
    let mut names: Vec<&str> = doc(&app).child_sheets[0]
        .pins
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["SCL", "SDA"]);
}

#[test]
fn sync_sheet_pins_of_a_missing_child_file_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path(), SchematicSheet::default());
    answered(sheet_symbol(&mut app, "Charger", "charger.snxsch"));
    let reason = refusal(sync(&mut app));
    assert!(reason.contains("child schematic not found"), "{reason}");
    let unknown = refusal(run(&mut app, "sync_sheet_pins", &["sheet=Nope".into()]));
    assert!(unknown.contains("no sheet symbol `Nope`"));
}
