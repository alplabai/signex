//! #2418 — a placed object's Position, Rotation, Size and Parameters, edited
//! in Properties (through `Signex::update`) and from operands (the CLI and
//! agent path): one engine edit and one undo step each, the panel's unit and
//! page origin on the rows, a lock refusing with its reason.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;
use signex_types::coord::Unit;
use signex_types::schematic::{
    BusEntry, ChildSheet, FillType, HAlign, Junction, Label, LabelType, NoConnect, Point,
    SchematicSheet, SelectedItem, SelectedKind, SheetPin, Symbol, TextNote, VAlign, Wire,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;
use crate::panels::{PanelMsg, PlacedEditMsg, PlacedField};

const PART: u128 = 1;
const SHEET: u128 = 2;
const LABEL: u128 = 3;
const JUNCTION: u128 = 4;
const NO_CONNECT: u128 = 5;
const NOTE: u128 = 6;
const WIRE: u128 = 7;
const BUS_ENTRY: u128 = 8;
const NO_ERC: u128 = 9;
const ENTRY: u128 = 10;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// An A4 sheet (Lower Left origin, 210 mm tall) with one object of each
/// placed kind, none touching another.
fn sheet() -> SchematicSheet {
    let mut label = Label::new("NET", Point::new(100.0, 10.0), LabelType::Net);
    label.uuid = id(LABEL);
    SchematicSheet {
        paper_size: "A4".into(),
        symbols: vec![Symbol {
            uuid: id(PART),
            lib_id: "Device:R".into(),
            reference: "R1".into(),
            position: Point::new(10.0, 10.0),
            fields: HashMap::from([("Manufacturer".to_string(), "Acme".to_string())]),
            ..Default::default()
        }],
        child_sheets: vec![ChildSheet {
            uuid: id(SHEET),
            name: "Power".into(),
            filename: "power.snxsch".into(),
            position: Point::new(50.0, 30.0),
            size: (20.0, 20.0),
            stroke_width: 0.15,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: id(ENTRY),
                name: "VIN".into(),
                direction: "input".into(),
                position: Point::new(50.0, 35.0),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        }],
        labels: vec![label],
        junctions: vec![Junction {
            uuid: id(JUNCTION),
            position: Point::new(120.0, 10.0),
            diameter: 0.0,
            minted: false,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }],
        no_connects: vec![NoConnect {
            uuid: id(NO_CONNECT),
            position: Point::new(130.0, 10.0),
        }],
        no_erc_directives: vec![NoConnect {
            uuid: id(NO_ERC),
            position: Point::new(140.0, 10.0),
        }],
        text_notes: vec![TextNote {
            uuid: id(NOTE),
            text: "note".into(),
            position: Point::new(150.0, 10.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked: false,
        }],
        wires: vec![Wire {
            color: None,
            uuid: id(WIRE),
            start: Point::new(160.0, 10.0),
            end: Point::new(170.0, 10.0),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
            locked: false,
        }],
        bus_entries: vec![BusEntry {
            uuid: id(BUS_ENTRY),
            position: Point::new(180.0, 10.0),
            size: (2.54, 2.54),
        }],
        ..Default::default()
    }
}

fn app_with(dir: &Path, sheet: SchematicSheet) -> Signex {
    let (mut app, _task) = Signex::new();
    add_sheet_with(&mut app, &dir.join("main.snxsch"), sheet);
    app
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

fn part(app: &Signex) -> &Symbol {
    &doc(app).symbols[0]
}

fn select(app: &mut Signex, item: SelectedItem) {
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app.update_selection_info();
}

fn panel(app: &mut Signex, msg: PlacedEditMsg) {
    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::PlacedEdit(msg),
    )));
}

/// Type `text` into `field` of `item` and press Enter, as a person does.
fn type_in(app: &mut Signex, item: SelectedItem, field: PlacedField, text: &str) {
    panel(
        app,
        PlacedEditMsg::Typed {
            item,
            field: field.clone(),
            text: text.into(),
        },
    );
    panel(app, PlacedEditMsg::Submitted { item, field });
}

fn undo(app: &mut Signex) {
    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
}

fn item(n: u128, kind: SelectedKind) -> SelectedItem {
    SelectedItem::new(id(n), kind)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn run(app: &mut Signex, command: &str, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new(command).expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands");
    app.run_operands_settled(&command, &args).0
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

/// X then Y typed into a part's Location rows: each Enter is one edit and
/// one undo step; Y is read from the page origin (A4, Lower Left: 160
/// shown is 50 stored).
#[test]
fn a_parts_position_is_typed_in_one_undo_step_per_box() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let r1 = item(PART, SelectedKind::Symbol);
    select(&mut app, r1);
    assert_eq!(app.document_state.panel_ctx.placed.item, Some(r1));

    type_in(&mut app, r1, PlacedField::X, "20");
    assert_eq!(part(&app).position, Point::new(20.0, 10.0));
    type_in(&mut app, r1, PlacedField::Y, "160");
    assert!(
        close(part(&app).position.y, 50.0),
        "{:?}",
        part(&app).position
    );
    assert_eq!(app.document_state.panel_ctx.placed.draft, None, "committed");

    undo(&mut app);
    assert_eq!(
        part(&app).position,
        Point::new(20.0, 10.0),
        "Y undone alone"
    );
    undo(&mut app);
    assert_eq!(part(&app).position, Point::new(10.0, 10.0));
}

/// The rows read the panel's unit: 1000 mil typed is 25.4 mm stored.
#[test]
fn a_typed_position_is_read_in_the_panels_unit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    app.ui_state.unit = Unit::Mil;
    let r1 = item(PART, SelectedKind::Symbol);
    select(&mut app, r1);
    type_in(&mut app, r1, PlacedField::X, "1000");
    assert!(
        close(part(&app).position.x, 25.4),
        "{:?}",
        part(&app).position
    );
}

/// Text that is not a number changes nothing and keeps the draft to fix.
#[test]
fn a_box_that_is_not_a_number_keeps_its_draft_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let r1 = item(PART, SelectedKind::Symbol);
    select(&mut app, r1);
    type_in(&mut app, r1, PlacedField::X, "ten");
    assert_eq!(part(&app).position, Point::new(10.0, 10.0));
    let draft = app.document_state.panel_ctx.placed.draft.clone();
    assert_eq!(draft.map(|d| d.text).as_deref(), Some("ten"));
}

/// The Rotation pick turns the part in place, one undo step.
#[test]
fn a_rotation_pick_turns_the_part_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let r1 = item(PART, SelectedKind::Symbol);
    select(&mut app, r1);
    panel(
        &mut app,
        PlacedEditMsg::RotationPicked {
            item: r1,
            degrees: 90.0,
        },
    );
    assert!(close(part(&app).rotation, 90.0), "{}", part(&app).rotation);
    assert_eq!(
        part(&app).position,
        Point::new(10.0, 10.0),
        "turns in place"
    );
    undo(&mut app);
    assert!(close(part(&app).rotation, 0.0));
}

/// A sheet symbol's Width, Height and X are editable, each one undo step.
#[test]
fn a_sheet_symbols_size_and_position_are_typed_in() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let power = item(SHEET, SelectedKind::ChildSheet);
    select(&mut app, power);
    let child = |app: &Signex| doc(app).child_sheets[0].clone();

    type_in(&mut app, power, PlacedField::Width, "30");
    type_in(&mut app, power, PlacedField::Height, "25");
    assert_eq!(child(&app).size, (30.0, 25.0));
    type_in(&mut app, power, PlacedField::X, "60");
    assert_eq!(child(&app).position, Point::new(60.0, 30.0));
    assert_eq!(
        child(&app).pins[0].position,
        Point::new(60.0, 35.0),
        "the entry moves with its symbol"
    );

    undo(&mut app);
    undo(&mut app);
    assert_eq!(child(&app).size, (30.0, 20.0), "one step per box");
    undo(&mut app);
    assert_eq!(child(&app).size, (20.0, 20.0));
}

/// A part's parameter is edited in its Parameters row, one undo step.
#[test]
fn a_parameter_is_edited_in_place_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let r1 = item(PART, SelectedKind::Symbol);
    select(&mut app, r1);
    let field = PlacedField::Parameter("Manufacturer".into());
    type_in(&mut app, r1, field, "Other");
    let value = |app: &Signex| part(app).fields.get("Manufacturer").cloned();
    assert_eq!(value(&app).as_deref(), Some("Other"));
    undo(&mut app);
    assert_eq!(value(&app).as_deref(), Some("Acme"));
}

/// Every kind that had only the read-only fallback view now has an editable
/// Position: typing X moves it.
#[test]
fn every_placed_kind_takes_a_typed_position() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    for (n, kind) in [
        (LABEL, SelectedKind::Label),
        (JUNCTION, SelectedKind::Junction),
        (NO_CONNECT, SelectedKind::NoConnect),
        (NO_ERC, SelectedKind::NoErcDirective),
        (NOTE, SelectedKind::TextNote),
        (WIRE, SelectedKind::Wire),
        (BUS_ENTRY, SelectedKind::BusEntry),
    ] {
        let it = item(n, kind);
        select(&mut app, it);
        assert_eq!(
            app.document_state.panel_ctx.placed.item,
            Some(it),
            "{kind:?}"
        );
        let before = signex_engine::placed_geometry(doc(&app), &it).expect("geometry");
        let to = before.position.x + 2.54;
        type_in(&mut app, it, PlacedField::X, &to.to_string());
        let after = signex_engine::placed_geometry(doc(&app), &it).expect("geometry");
        assert!(
            close(after.position.x, to),
            "{kind:?}: {:?}",
            after.position
        );
    }
    // A sheet entry stays on its symbol's border: Y moves it along it.
    let entry = item(ENTRY, SelectedKind::SheetPin);
    select(&mut app, entry);
    type_in(&mut app, entry, PlacedField::Y, "172.5");
    let pin = doc(&app).child_sheets[0].pins[0].position;
    assert!(close(pin.y, 37.5), "{pin:?}");
}

/// A locked part's rows are disabled with the lock's reason, and an edit
/// that reaches the handler anyway changes nothing.
#[test]
fn a_locked_part_shows_the_reason_and_refuses_the_edit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut locked = sheet();
    locked.symbols[0].locked = true;
    let mut app = app_with(dir.path(), locked);
    let r1 = item(PART, SelectedKind::Symbol);
    select(&mut app, r1);
    let reason = app.document_state.panel_ctx.placed.locked.clone();
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("R1 is locked")),
        "{reason:?}"
    );
    type_in(&mut app, r1, PlacedField::X, "20");
    panel(
        &mut app,
        PlacedEditMsg::RotationPicked {
            item: r1,
            degrees: 90.0,
        },
    );
    type_in(
        &mut app,
        r1,
        PlacedField::Parameter("Manufacturer".into()),
        "Other",
    );
    assert_eq!(part(&app).position, Point::new(10.0, 10.0));
    assert!(close(part(&app).rotation, 0.0));
    assert_eq!(part(&app).fields["Manufacturer"], "Acme");
}

/// The four commands from operands make the same edits as the rows, one undo
/// step each, and answer with the object's geometry.
#[test]
fn the_commands_from_operands_edit_as_the_rows_do() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let r1 = format!("id={}", id(PART));
    let power = format!("id={}", id(SHEET));

    let moved = answered(run(
        &mut app,
        "set_object_position",
        &[r1.clone(), "at=20,30".into()],
    ));
    assert_eq!(moved["kind"], "symbol");
    assert_eq!(moved["position"], serde_json::json!([20.0, 30.0]));
    let turned = answered(run(
        &mut app,
        "set_object_rotation",
        &[r1.clone(), "rotation=180".into()],
    ));
    assert!(close(turned["rotation"].as_f64().expect("rotation"), 180.0));
    let sized = answered(run(
        &mut app,
        "set_sheet_symbol_size",
        &[power, "size=30,25".into()],
    ));
    assert_eq!(sized["size"], serde_json::json!([30.0, 25.0]));
    let param = answered(run(
        &mut app,
        "set_symbol_parameter",
        &[r1, "name=Manufacturer".into(), "value=Other".into()],
    ));
    assert_eq!(param["parameter"]["value"], "Other");

    assert_eq!(part(&app).position, Point::new(20.0, 30.0));
    assert_eq!(part(&app).fields["Manufacturer"], "Other");
    for _ in 0..4 {
        undo(&mut app);
    }
    assert_eq!(part(&app).position, Point::new(10.0, 10.0));
    assert!(close(part(&app).rotation, 0.0));
    assert_eq!(doc(&app).child_sheets[0].size, (20.0, 20.0));
    assert_eq!(part(&app).fields["Manufacturer"], "Acme");
}

#[test]
fn the_commands_refuse_with_the_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut locked = sheet();
    locked.symbols[0].locked = true;
    let mut app = app_with(dir.path(), locked);
    let r1 = format!("id={}", id(PART));
    for (command, operands, needle) in [
        (
            "set_object_position",
            vec![r1.clone(), "at=20,30".into()],
            "R1 is locked",
        ),
        (
            "set_object_position",
            vec![format!("id={}", Uuid::from_u128(99)), "at=1,1".into()],
            "no placed object",
        ),
        (
            "set_object_position",
            vec![r1.clone(), "at=R1.1".into()],
            "not a pin reference",
        ),
        (
            "set_object_rotation",
            vec![format!("id={}", id(JUNCTION)), "rotation=90".into()],
            "only a part or a label",
        ),
        (
            "set_sheet_symbol_size",
            vec![r1.clone(), "size=10,10".into()],
            "only a sheet symbol",
        ),
        (
            "set_sheet_symbol_size",
            vec![format!("id={}", id(SHEET)), "size=1,10".into()],
            "at least",
        ),
        (
            "set_symbol_parameter",
            vec![r1.clone(), "name=value".into()],
            "its own row",
        ),
    ] {
        let reason = refusal(run(&mut app, command, &operands));
        assert!(reason.contains(needle), "{command}: {reason}");
    }
    assert_eq!(part(&app).position, Point::new(10.0, 10.0));
    assert_eq!(doc(&app).child_sheets[0].size, (20.0, 20.0));
}

/// Without operands (palette, key) a command says which row to use.
#[test]
fn without_operands_the_command_names_its_properties_row() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    for (command, row) in [
        ("set_object_position", "Position row"),
        ("set_object_rotation", "Rotation row"),
        ("set_sheet_symbol_size", "Size row"),
        ("set_symbol_parameter", "Parameters rows"),
    ] {
        let command = AppCommandId::new(command).expect("id");
        let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
        let reason = refusal(outcome);
        assert!(reason.contains(row), "{reason}");
    }
}
