//! DECISIONS 2026-11-22, mockup row 45: a power port's rename takes the whole
//! rail, on open and closed sheets, as one undo step or not at all.

use std::path::{Path, PathBuf};

use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol,
};
use uuid::Uuid;

use super::rail_commands;
use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{PanelMsg, PlacedEditMsg};

const PORT: Uuid = Uuid::from_u128(0x71);
const CLOSED_PORT: Uuid = Uuid::from_u128(0x72);
const OTHER_RAIL: Uuid = Uuid::from_u128(0x73);

fn power_port(uuid: Uuid, name: &str) -> Symbol {
    Symbol {
        uuid,
        lib_id: "power:bar".into(),
        value: name.into(),
        position: Point::new(40.0, 40.0),
        is_power: true,
        ..Default::default()
    }
}

fn read_sheet(path: &Path) -> SchematicSheet {
    let text = std::fs::read_to_string(path).expect("read sheet");
    SnxSchematic::parse(&text).expect("parse sheet").sheet
}

fn write_sheet(path: &Path, sheet: SchematicSheet) {
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// `top` open with the port typed on and a port of another rail; `child`
/// closed on disk with `closed` added to it.
fn project(dir: &tempfile::TempDir, closed: Symbol) -> (Signex, PathBuf, PathBuf) {
    let (mut app, top, child) = placed_twice_on_disk(dir);
    let mut child_sheet = read_sheet(&child);
    child_sheet.symbols.push(closed);
    write_sheet(&child, child_sheet);
    let mut top_sheet = app.document_state.engines[&top].document().clone();
    top_sheet.symbols.push(power_port(PORT, "+3V3"));
    top_sheet.symbols.push(Symbol {
        position: Point::new(60.0, 40.0),
        ..power_port(OTHER_RAIL, "+5V")
    });
    let engine =
        signex_engine::Engine::new_with_path(top_sheet, Some(top.clone())).expect("engine");
    app.document_state.insert_engine(top.clone(), engine);
    select_port(&mut app);
    (app, top, child)
}

fn select_port(app: &mut Signex) {
    let item = SelectedItem::new(PORT, SelectedKind::Symbol);
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app.update_selection_info();
}

/// Type `name` into the port's Name box, press Enter and run the reads.
fn rename(app: &mut Signex, name: &str) {
    let item = SelectedItem::new(PORT, SelectedKind::Symbol);
    let panel = |msg| Message::Dock(DockMessage::Panel(PanelMsg::PlacedEdit(msg)));
    let mut driver = crate::task_driver::TaskDriver::new();
    let typed = PlacedEditMsg::PortNameTyped {
        item,
        text: name.into(),
    };
    driver.send(app, panel(typed)).expect("typed");
    let submitted = PlacedEditMsg::PortNameSubmitted { item };
    driver
        .send(app, panel(submitted))
        .expect("the rename settled");
}

fn value(app: &Signex, sheet: &Path, id: Uuid) -> String {
    let symbols = &app.document_state.engines[sheet].document().symbols;
    let symbol = symbols.iter().find(|s| s.uuid == id).expect("symbol");
    symbol.value.clone()
}

fn assert_nothing_changed(app: &Signex, top: &Path, child: &Path, on_disk: &[u8]) {
    assert_eq!(value(app, top, PORT), "+3V3");
    assert!(!app.document_state.engines[top].can_undo(), "no undo step");
    assert!(
        !app.document_state.engines.contains_key(child),
        "not loaded"
    );
    assert_eq!(std::fs::read(child).expect("child"), on_disk);
}

#[test]
fn a_rename_takes_the_rail_on_a_closed_sheet_as_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = project(&dir, power_port(CLOSED_PORT, "+3V3"));
    let on_disk = std::fs::read(&child).expect("child");

    rename(&mut app, "+3V3_MCU");

    assert_eq!(value(&app, &top, PORT), "+3V3_MCU");
    assert_eq!(value(&app, &child, CLOSED_PORT), "+3V3_MCU");
    assert_eq!(value(&app, &top, OTHER_RAIL), "+5V", "another rail stays");
    assert!(app.document_state.is_document_dirty(&child));
    assert_eq!(
        std::fs::read(&child).expect("child"),
        on_disk,
        "no disk write"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));

    assert_eq!(value(&app, &top, PORT), "+3V3");
    assert!(!app.document_state.engines[&top].can_undo(), "one step");
    assert!(
        !app.document_state.engines.contains_key(&child),
        "the closed sheet is back to its file"
    );
    assert_eq!(std::fs::read(&child).expect("child"), on_disk);
}

#[test]
fn a_rename_takes_the_rail_on_a_sheet_open_in_another_tab() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = project(&dir, power_port(CLOSED_PORT, "+3V3"));
    assert!(app.open_or_focus_sheet_path(child.clone()));
    assert!(app.open_or_focus_sheet_path(top.clone()));
    select_port(&mut app);

    rename(&mut app, "+3V3_MCU");

    assert_eq!(value(&app, &top, PORT), "+3V3_MCU");
    assert_eq!(value(&app, &child, CLOSED_PORT), "+3V3_MCU");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(value(&app, &child, CLOSED_PORT), "+3V3", "one undo step");
}

#[test]
fn an_unreadable_sheet_refuses_the_whole_rename() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = project(&dir, power_port(CLOSED_PORT, "+3V3"));
    std::fs::write(&child, "not a sheet").expect("break the child");

    rename(&mut app, "+3V3_MCU");

    assert_nothing_changed(&app, &top, &child, b"not a sheet");
}

#[test]
fn a_locked_port_of_the_rail_refuses_the_whole_rename() {
    let dir = tempfile::tempdir().expect("tempdir");
    let locked = Symbol {
        locked: true,
        ..power_port(CLOSED_PORT, "+3V3")
    };
    let (mut app, top, child) = project(&dir, locked);
    let on_disk = std::fs::read(&child).expect("child");

    rename(&mut app, "+3V3_MCU");

    assert_eq!(value(&app, &top, PORT), "+3V3");
    assert!(!app.document_state.engines.contains_key(&child), "put back");
    assert_eq!(std::fs::read(&child).expect("child"), on_disk);
}

#[test]
fn a_blank_name_would_unname_the_rail_and_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, top, child) = project(&dir, power_port(CLOSED_PORT, "+3V3"));
    let on_disk = std::fs::read(&child).expect("child");

    rename(&mut app, "  ");

    assert_nothing_changed(&app, &top, &child, &on_disk);
}

#[test]
fn only_a_named_power_port_starts_a_rail_rename() {
    let flag = Symbol {
        lib_id: "power:PWR_FLAG".into(),
        ..power_port(Uuid::from_u128(1), "+3V3")
    };
    let part = Symbol {
        is_power: false,
        ..power_port(Uuid::from_u128(2), "+3V3")
    };
    let unnamed = power_port(Uuid::from_u128(3), "");
    let sheet = SchematicSheet {
        symbols: vec![flag, part, unnamed, power_port(PORT, "+3V3")],
        ..Default::default()
    };
    let path = PathBuf::from("rail.snxsch");
    let (mut app, _task) = Signex::new();
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    for id in 1..=3 {
        assert!(
            app.rail_rename_on(&path, Uuid::from_u128(id), "X")
                .is_none()
        );
    }
    let rename = app.rail_rename_on(&path, PORT, "X").expect("a rail");
    assert_eq!((rename.old.as_str(), rename.new.as_str()), ("+3V3", "X"));
}

#[test]
fn the_rail_is_its_power_ports_and_power_labels_only() {
    let flag = Symbol {
        lib_id: "power:PWR_FLAG".into(),
        ..power_port(Uuid::from_u128(1), "+3V3")
    };
    let part = Symbol {
        is_power: false,
        lib_id: "Device:R".into(),
        ..power_port(Uuid::from_u128(2), "+3V3")
    };
    let origin = Point::new(0.0, 0.0);
    let sheet = SchematicSheet {
        symbols: vec![
            flag,
            part,
            power_port(PORT, "+3V3"),
            power_port(OTHER_RAIL, "+5V"),
        ],
        labels: vec![
            Label {
                uuid: Uuid::from_u128(3),
                ..Label::new("+3V3", origin, LabelType::Power)
            },
            Label {
                uuid: Uuid::from_u128(4),
                ..Label::new("+3V3", origin, LabelType::Net)
            },
        ],
        ..Default::default()
    };
    let targets: Vec<_> = rail_commands(&sheet, "+3V3", "+1V8")
        .into_iter()
        .map(|command| match command {
            signex_engine::Command::UpdateText { target, value } => {
                assert_eq!(value, "+1V8");
                target
            }
            other => panic!("a text edit, got {other:?}"),
        })
        .collect();
    assert_eq!(
        targets,
        [
            signex_engine::TextTarget::SymbolValue(PORT),
            signex_engine::TextTarget::Label(Uuid::from_u128(3)),
        ]
    );
}
