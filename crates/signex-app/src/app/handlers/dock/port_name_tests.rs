//! Mockup row 45 (DECISIONS 2026-11-22): a power port's one Name field goes
//! through the engine as one undo step, however many keys were typed, and
//! a pin focus on the port leaves its Location open.

use crate::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::{PanelMsg, PlacedEditMsg};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::path::PathBuf;

const PORT: uuid::Uuid = uuid::Uuid::from_u128(1);
const OTHER: uuid::Uuid = uuid::Uuid::from_u128(2);
const PART: uuid::Uuid = uuid::Uuid::from_u128(3);

fn power_port(uuid: uuid::Uuid, x: f64) -> Symbol {
    Symbol {
        uuid,
        lib_id: "power:bar".into(),
        value: "+3V3".into(),
        position: Point::new(x, 10.0),
        is_power: true,
        ..Default::default()
    }
}

/// A saved sheet with two power ports and a part, the first port selected.
fn app() -> (Signex, PathBuf) {
    let part = Symbol {
        uuid: PART,
        lib_id: "Device:R".into(),
        reference: "R1".into(),
        position: Point::new(50.0, 10.0),
        ..Default::default()
    };
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![power_port(PORT, 10.0), power_port(OTHER, 30.0), part],
        ..Default::default()
    };
    let path = PathBuf::from("port-name.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.mark_saved();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "port".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    select(&mut app, PORT);
    (app, path)
}

fn item(id: uuid::Uuid) -> SelectedItem {
    SelectedItem::new(id, SelectedKind::Symbol)
}

fn select(app: &mut Signex, id: uuid::Uuid) {
    app.interaction_state.active_canvas_mut().selected = vec![item(id)];
    app.update_selection_info();
}

fn panel(app: &mut Signex, msg: PlacedEditMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PlacedEdit(msg))));
}

fn typing(app: &mut Signex, text: &str) {
    panel(
        app,
        PlacedEditMsg::PortNameTyped {
            item: item(PORT),
            text: text.into(),
        },
    );
}

fn name(app: &Signex, id: uuid::Uuid) -> String {
    let engine = app.document_state.active_engine().expect("engine");
    let symbols = &engine.document().symbols;
    let symbol = symbols.iter().find(|s| s.uuid == id).expect("symbol");
    symbol.value.clone()
}

#[test]
fn typing_a_name_is_no_edit_until_enter() {
    let (mut app, path) = app();
    typing(&mut app, "+5");
    assert_eq!(name(&app, PORT), "+3V3");
    assert!(!app.document_state.engines[&path].can_undo());
}

#[test]
fn a_rename_is_one_undo_step_however_many_keys_were_typed() {
    let (mut app, path) = app();
    for text in ["+", "+5", "+5V"] {
        typing(&mut app, text);
    }
    panel(
        &mut app,
        PlacedEditMsg::PortNameSubmitted { item: item(PORT) },
    );
    assert_eq!(name(&app, PORT), "+5V");
    assert_eq!(app.document_state.panel_ctx.placed.port_name, None);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(name(&app, PORT), "+3V3");
    assert!(!app.document_state.engines[&path].can_undo());
}

#[test]
fn leaving_the_port_applies_the_typed_name_to_it() {
    let (mut app, path) = app();
    typing(&mut app, "+5V");
    select(&mut app, OTHER);
    // The update loop settles what the selection change left owed.
    panel(
        &mut app,
        PlacedEditMsg::PortNameSubmitted { item: item(OTHER) },
    );
    assert_eq!(name(&app, PORT), "+5V");
    assert_eq!(name(&app, OTHER), "+5V", "the whole rail renames");
    assert!(app.document_state.engines[&path].can_undo());
}

#[test]
fn a_save_commit_lands_the_typed_name() {
    let (mut app, path) = app();
    typing(&mut app, "+5V");
    app.commit_note_drafts_where(&|sheet| sheet == path);
    assert_eq!(name(&app, PORT), "+5V");
}

#[test]
fn a_discard_drops_the_typed_name() {
    let (mut app, path) = app();
    typing(&mut app, "+5V");
    app.drop_note_drafts_where(&|sheet| sheet == path);
    assert_eq!(app.document_state.panel_ctx.placed.port_name, None);
    assert_eq!(name(&app, PORT), "+3V3");
    assert!(!app.document_state.engines[&path].can_undo());
}

#[test]
fn a_pin_focus_folds_a_part_but_not_a_power_port() {
    let (app, _path) = app();
    assert!(!app.pin_focus_folds(Some(PORT)));
    assert!(app.pin_focus_folds(Some(PART)));
    assert!(!app.pin_focus_folds(None));
}

/// Select `symbol` the way a click on its pin does: the pin focused.
fn click_pin(app: &mut Signex, path: &std::path::Path, symbol: uuid::Uuid) {
    let placement = app.shown_placement(path);
    app.interaction_state.clicked_pin = Some(crate::app::state::ClickedPin {
        sheet: path.to_path_buf(),
        placement,
        symbol,
        pin: "1".into(),
    });
    select(app, symbol);
}

/// Mockup row 45: a part's pin focus folds Location, and a click on the
/// power port's own pin next opens its Location with Position and Rotation.
#[test]
fn a_power_port_pin_click_shows_its_location_open() {
    let (mut app, path) = app();
    click_pin(&mut app, &path, PART);
    let collapsed =
        |app: &Signex| (app.document_state.panel_ctx.collapsed_sections).contains("sel_location");
    assert!(collapsed(&app), "a part's pin focus folds Location");
    click_pin(&mut app, &path, PORT);
    assert!(
        app.interaction_state.clicked_pin.is_some(),
        "the port's pin stays focused"
    );
    assert!(!collapsed(&app), "the power port's Location is open");
    assert_eq!(app.document_state.panel_ctx.placed.item, Some(item(PORT)));
}
