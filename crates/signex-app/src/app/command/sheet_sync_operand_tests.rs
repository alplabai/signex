//! #1276 — Create Sheet Symbol From Sheet and Synchronize Sheet Entries and
//! Ports from operands: one undo step each, the file read before the edit
//! runs, the `place_sheet_symbol` checks (a loop at any depth included).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::canvas::placement_tests::grid;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind};
use crate::keymap::AppCommandId;

/// A grid-snapping app with an empty sheet active as `main.snxsch` in `dir`.
fn app_in(dir: &Path) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    add_sheet_with(
        &mut app,
        &dir.join("main.snxsch"),
        SchematicSheet::default(),
    );
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

fn undo(app: &mut Signex) {
    let undo = AppCommandId::new("undo").expect("id");
    let _ = app.dispatch_command_reporting(&undo, CommandArgs::none(), None);
}

/// Write the sheet `file` in `dir` with hierarchical labels `names`.
fn write_sheet(dir: &Path, file: &str, names: &[&str]) -> PathBuf {
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
        .expect("write sheet");
    path
}

fn create(app: &mut Signex, path: &Path, extra: &[String]) -> CommandOutcome {
    let mut operands = vec![format!("path={}", path.display())];
    operands.extend_from_slice(extra);
    run(app, "create_sheet_symbol_from_sheet", &operands)
}

#[test]
fn a_symbol_from_a_sheet_file_has_every_port_and_is_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let charger = write_sheet(dir.path(), "charger.snxsch", &["SDA", "SCL"]);
    let mut app = app_in(dir.path());
    let at = grid(20.0, 60.0);
    let answer = answered(create(&mut app, &charger, &[format!("at={}", pt(at))]));
    assert_eq!(answer["name"], "charger");
    assert_eq!(answer["file"], "charger.snxsch");
    assert_eq!(answer["at"], json!([at.x, at.y]));
    assert_eq!(answer["entries"], json!(["SCL", "SDA"]));
    let placed = &doc(&app).child_sheets[0];
    assert_eq!(placed.uuid.to_string(), answer["id"]);
    assert_eq!(placed.pins.len(), 2);
    undo(&mut app);
    assert!(doc(&app).child_sheets.is_empty(), "one undo step");
}

#[test]
fn a_file_already_placed_or_missing_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let charger = write_sheet(dir.path(), "charger.snxsch", &["SDA"]);
    let mut app = app_in(dir.path());
    answered(create(&mut app, &charger, &[]));
    let again = refusal(create(&mut app, &charger, &[]));
    assert!(
        again.contains("already refers to charger.snxsch"),
        "{again}"
    );
    // A file that is not there has no ports to make entries for.
    refusal(create(&mut app, &dir.path().join("nope.snxsch"), &[]));
    assert_eq!(doc(&app).child_sheets.len(), 1);
}

fn node(dir: &Path, file: &str, children: Vec<SheetNode>) -> SheetNode {
    SheetNode {
        name: file.into(),
        filename: file.into(),
        placed_as: Some(Uuid::now_v7()),
        path: dir.join(file),
        kind: SheetNodeKind::Present,
        detail: None,
        children,
    }
}

/// Write the sheet `file` in `dir` with one sheet symbol placing `places`.
fn write_parent(dir: &Path, file: &str, places: &str) -> PathBuf {
    let path = dir.join(file);
    let symbol = signex_types::schematic::ChildSheet {
        uuid: Uuid::now_v7(),
        name: places.trim_end_matches(".snxsch").to_string(),
        filename: places.to_string(),
        position: Point::new(0.0, 0.0),
        size: (30.0, 30.0),
        stroke_width: 0.0,
        fill: signex_types::schematic::FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    };
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        child_sheets: vec![symbol],
        ..Default::default()
    };
    signex_engine::Engine::new_with_path(sheet, Some(path.clone()))
        .expect("engine")
        .save()
        .expect("write sheet");
    path
}

/// main (A, edited) is open; b places c and c places main, both closed and in
/// no hierarchy a window holds: a symbol for b would close A→B→C→A. The GUI
/// refuses it the same way (`sheet_sync::tests::a_loop_through_two_closed_sheets_is_refused`).
#[test]
fn a_loop_two_levels_down_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let b = write_parent(d, "b.snxsch", "c.snxsch");
    write_parent(d, "c.snxsch", "main.snxsch");
    let mut app = app_in(d);
    let reason = refusal(create(&mut app, &b, &[]));
    assert!(reason.contains("places this sheet below it"), "{reason}");
    let merged = refusal(create(&mut app, &b, &["allow_merge=true".to_string()]));
    assert!(merged.contains("places this sheet below it"), "{merged}");
    assert!(doc(&app).child_sheets.is_empty());
}

/// top places mid, mid places main (the edited sheet): top is main's
/// grandparent, two levels up.
#[test]
fn a_grandparent_is_refused_as_a_loop() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let top = write_sheet(d, "top.snxsch", &["VIN"]);
    let mut app = app_in(d);
    let mid = node(d, "mid.snxsch", vec![node(d, "main.snxsch", Vec::new())]);
    app.document_state.panel_ctx.navigator.hierarchy = Some(SheetHierarchy {
        root: Some(node(d, "top.snxsch", vec![mid])),
        ..Default::default()
    });
    let reason = refusal(create(&mut app, &top, &[]));
    assert!(reason.contains("is an ancestor of this sheet"), "{reason}");
    assert!(doc(&app).child_sheets.is_empty());
}

/// `Charger` from grid (20, 20) to (40, 40), referring to `charger.snxsch`.
fn charger_symbol(app: &mut Signex) -> Value {
    answered(run(
        app,
        "place_sheet_symbol",
        &[
            format!("start={}", pt(grid(20.0, 20.0))),
            format!("end={}", pt(grid(40.0, 40.0))),
            "name=Charger".into(),
            "file=charger.snxsch".into(),
        ],
    ))
}

fn sync(app: &mut Signex, symbol: &str, direction: &str) -> CommandOutcome {
    run(
        app,
        "sync_sheet_entries",
        &[format!("symbol={symbol}"), format!("direction={direction}")],
    )
}

fn entry_names(app: &Signex) -> Vec<String> {
    let mut names: Vec<String> = doc(app).child_sheets[0]
        .pins
        .iter()
        .map(|p| p.name.clone())
        .collect();
    names.sort_unstable();
    names
}

#[test]
fn to_symbol_by_name_or_id_adds_the_ports_and_answers_the_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_sheet(dir.path(), "charger.snxsch", &["SDA", "SCL"]);
    let mut app = app_in(dir.path());
    let placed = charger_symbol(&mut app);
    let first = answered(sync(&mut app, "Charger", "to_symbol"));
    assert_eq!(first["direction"], "to_symbol");
    assert_eq!(first["added"], json!(["SCL", "SDA"]));
    assert_eq!(first["removed"], json!([]));
    assert_eq!(first["renamed"], json!([]));
    assert_eq!(entry_names(&app), ["SCL", "SDA"]);
    // The sides agree now: nothing to change, nothing changed.
    let id = placed["id"].as_str().expect("id").to_string();
    let second = answered(sync(&mut app, &id, "to_symbol"));
    assert_eq!(second["added"], json!([]));
    undo(&mut app);
    assert!(entry_names(&app).is_empty(), "one undo step");
}

#[test]
fn to_sheet_changes_the_closed_child_in_memory_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let child = write_sheet(dir.path(), "charger.snxsch", &[]);
    let on_disk = std::fs::read_to_string(&child).expect("child file");
    let mut app = app_in(dir.path());
    charger_symbol(&mut app);
    answered(run(
        &mut app,
        "place_sheet_entry",
        &[
            "sheet=Charger".into(),
            format!("at={}", pt(grid(20.0, 24.0))),
            "name=VIN".into(),
        ],
    ));
    let answer = answered(sync(&mut app, "Charger", "to_sheet"));
    assert_eq!(answer["added"], json!(["VIN"]));
    let engine = app
        .document_state
        .engines
        .get(&child)
        .expect("the closed child is held in memory");
    let ports: Vec<String> = signex_engine::exposed_sheet_ports(engine.document())
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(ports, ["VIN"]);
    assert_eq!(
        std::fs::read_to_string(&child).expect("child file"),
        on_disk,
        "nothing is written until Save"
    );
}

#[test]
fn an_unknown_direction_or_symbol_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_sheet(dir.path(), "charger.snxsch", &["SDA"]);
    let mut app = app_in(dir.path());
    charger_symbol(&mut app);
    let direction = refusal(sync(&mut app, "Charger", "both"));
    assert!(direction.contains("to_symbol or to_sheet"), "{direction}");
    let unknown = refusal(sync(&mut app, "Nope", "to_symbol"));
    assert!(unknown.contains("no sheet symbol `Nope`"), "{unknown}");
    assert!(entry_names(&app).is_empty());
}
