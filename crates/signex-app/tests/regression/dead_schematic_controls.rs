//! #791 — four schematic controls that used to do nothing: View > Cycle
//! Grid Size, View > Zoom In / Zoom Out, and the Annotate dialog's
//! "Update Changes List" (removed: the preview is live) and "All On" /
//! "All Off" (were placeholders that closed the dialog).

use iced::{Point as ScreenPoint, Rectangle};
use signex_app::app::{AnnotateMsg, Message, Signex, UiMsg};
use signex_app::menu_bar::MenuMessage;
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use std::collections::HashMap;
use std::path::PathBuf;

fn symbol(uuid: uuid::Uuid, reference: &str, x: f64) -> Symbol {
    Symbol {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid,
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: String::new(),
        datasheet: String::new(),
        position: Point::new(x, 0.0),
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols,
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    }
}

fn app_with(symbols: Vec<Symbol>) -> (Signex, PathBuf) {
    let path = PathBuf::from("dead-controls.snxsch");
    let engine = signex_engine::Engine::new(sheet_with(symbols)).expect("engine");

    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "dead-controls".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());

    (app, path)
}

const VIEWPORT: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

#[test]
fn cycle_grid_steps_snap_and_linked_visible_grid_through_the_presets() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.visible_grid_mm = 1.27;

    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 2.54);
    assert_eq!(app.ui_state.visible_grid_mm, 2.54);

    // Wraps from the last preset back to the first.
    app.ui_state.grid_size_mm = 5.08;
    app.ui_state.visible_grid_mm = 5.08;
    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 0.635);
    assert_eq!(app.ui_state.visible_grid_mm, 0.635);
}

#[test]
fn ui_grid_cycle_message_steps_the_grid_like_the_menu_item() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 2.54;
    app.ui_state.visible_grid_mm = 2.54;

    let _ = app.update(Message::Ui(UiMsg::GridCycle));
    assert_eq!(app.ui_state.grid_size_mm, 5.08);
    assert_eq!(app.ui_state.visible_grid_mm, 5.08);
}

#[test]
fn cycle_grid_from_a_non_preset_size_lands_on_the_first_preset() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 0.5;
    app.ui_state.visible_grid_mm = 0.5;

    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 0.635);
}

#[test]
fn cycle_grid_leaves_a_deliberately_different_visible_grid_alone() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.visible_grid_mm = 5.08;

    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 2.54);
    assert_eq!(app.ui_state.visible_grid_mm, 5.08);
}

#[test]
fn zoom_in_and_out_change_scale_and_keep_the_view_centre_fixed() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]);
    app.interaction_state.active_canvas().set_viewport(VIEWPORT);
    let centre = ScreenPoint::new(400.0, 300.0);
    let world_at_centre = |app: &Signex| {
        let (ox, oy, s) = app.interaction_state.active_canvas().live_camera();
        ((centre.x - ox) / s, (centre.y - oy) / s)
    };
    let (_, _, s0) = app.interaction_state.active_canvas().live_camera();
    let w0 = world_at_centre(&app);

    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let (_, _, s1) = app.interaction_state.active_canvas().live_camera();
    assert!(
        (s1 / s0 - 1.1).abs() < 1e-4,
        "one notch in = x1.1, got {}",
        s1 / s0
    );
    let w1 = world_at_centre(&app);
    assert!((w1.0 - w0.0).abs() < 1e-3 && (w1.1 - w0.1).abs() < 1e-3);

    let _ = app.update(Message::Menu(MenuMessage::ZoomOut));
    let (_, _, s2) = app.interaction_state.active_canvas().live_camera();
    assert!((s2 - s0).abs() < 1e-3, "in then out returns to the start");
}

#[test]
fn zoom_stops_at_the_camera_limits() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]);
    app.interaction_state.active_canvas().set_viewport(VIEWPORT);
    for _ in 0..200 {
        let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    }
    let (_, _, max) = app.interaction_state.active_canvas().live_camera();
    for _ in 0..200 {
        let _ = app.update(Message::Menu(MenuMessage::ZoomOut));
    }
    let (_, _, min) = app.interaction_state.active_canvas().live_camera();
    assert!(
        (max / min - 64.0).abs() < 0.5,
        "25%..1600% = 64x range, got {}",
        max / min
    );
}

#[test]
fn pcb_canvas_zooms_about_its_centre() {
    let pcb = signex_app::pcb_canvas::PcbCanvas::new();
    pcb.set_viewport(VIEWPORT);
    let (ox0, oy0, s0) = pcb.live_camera();
    let w0 = ((400.0 - ox0) / s0, (300.0 - oy0) / s0);
    assert!(pcb.zoom_about_centre(true));
    let (ox1, oy1, s1) = pcb.live_camera();
    assert!(s1 > s0);
    let w1 = ((400.0 - ox1) / s1, (300.0 - oy1) / s1);
    assert!((w1.0 - w0.0).abs() < 1e-3 && (w1.1 - w0.1).abs() < 1e-3);
    assert!(pcb.zoom_about_centre(false));
}

#[test]
fn annotate_all_off_locks_every_listed_symbol_and_all_on_unlocks_them() {
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let (mut app, _path) = app_with(vec![symbol(a, "R?", 0.0), symbol(b, "R?", 10.0)]);

    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(true)));
    assert!(app.ui_state.annotate_locked.contains(&a));
    assert!(app.ui_state.annotate_locked.contains(&b));

    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(false)));
    assert!(app.ui_state.annotate_locked.is_empty());
}

#[test]
fn annotate_all_on_off_keep_the_dialog_open() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R?", 0.0)]);
    let _ = app.update(Message::Annotate(AnnotateMsg::OpenDialog));
    assert!(app.ui_state.annotate_dialog_open);
    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(true)));
    assert!(app.ui_state.annotate_dialog_open);
    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(false)));
    assert!(app.ui_state.annotate_dialog_open);
}
