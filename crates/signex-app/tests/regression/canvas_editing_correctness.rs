//! #102 — canvas editing correctness leftovers from the 2026-07 audit.
//!
//! The undo-marker desync (#533), the partial-undo repaint and the
//! Find/Replace whole-field overwrite (#125) were already fixed; these pin
//! the three that were not:
//!
//! * switching placement tools leaked wire / arc / polyline / shape-anchor
//!   buffers into the new tool;
//! * the selection survived a switch between two schematic tabs;
//! * a double-click commit of a shape tool skipped the grid snap the
//!   single-click path applies.

use signex_app::app::{Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::menu_bar::MenuMessage;
use signex_app::tab_bar::TabMessage;
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
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
        net_colors: HashMap::new(),
        font_family: None,
    }
}

fn add_tab(app: &mut Signex, path: &Path) {
    let engine = signex_engine::Engine::new(empty_sheet()).expect("engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.to_string_lossy().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
}

fn activate(app: &mut Signex, idx: usize) {
    app.document_state.active_tab = idx;
    app.document_state.active_path = Some(app.document_state.tabs[idx].path.clone());
}

#[test]
fn switching_tools_drops_the_previous_tools_buffers() {
    let (mut app, _task) = Signex::new();
    let i = &mut app.interaction_state;
    i.current_tool = Tool::Wire;
    i.wire_drawing = true;
    i.wire_points = vec![Point::new(1.0, 1.0)];
    i.arc_points = vec![Point::new(2.0, 2.0)];
    i.polyline_points = vec![Point::new(3.0, 3.0)];
    i.shape_anchor = Some(Point::new(4.0, 4.0));
    i.active_canvas_mut().drawing_mode = true;
    i.active_canvas_mut().arc_points = vec![Point::new(2.0, 2.0)];

    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)));

    let i = &app.interaction_state;
    assert_eq!(i.current_tool, Tool::Rectangle);
    assert!(!i.wire_drawing);
    assert!(i.wire_points.is_empty());
    assert!(i.arc_points.is_empty());
    assert!(i.polyline_points.is_empty());
    assert!(i.shape_anchor.is_none());
    assert!(!i.active_canvas().drawing_mode);
    assert!(i.active_canvas().arc_points.is_empty());
}

#[test]
fn selection_does_not_carry_to_another_schematic_tab() {
    let (mut app, _task) = Signex::new();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    add_tab(&mut app, &PathBuf::from("a.snxsch"));
    add_tab(&mut app, &PathBuf::from("b.snxsch"));
    activate(&mut app, 0);

    app.interaction_state.active_canvas_mut().selected = vec![SelectedItem::new(
        uuid::Uuid::new_v4(),
        SelectedKind::Symbol,
    )];
    // The panel mirror of that selection (what a Properties edit reads).
    app.document_state.panel_ctx.selection_count = 1;
    app.document_state.panel_ctx.selected_uuid = Some(uuid::Uuid::new_v4());
    app.document_state.panel_ctx.selected_kind = Some(SelectedKind::Symbol);

    let _ = app.update(Message::Tab {
        window_id: window,
        msg: TabMessage::Select(1),
    });

    assert_eq!(app.document_state.active_tab, 1);
    assert!(
        app.interaction_state.active_canvas().selected.is_empty(),
        "a selection picked in tab A must not survive into tab B"
    );
    let ctx = &app.document_state.panel_ctx;
    assert_eq!(ctx.selection_count, 0);
    assert!(ctx.selected_uuid.is_none() && ctx.selected_kind.is_none());
}

#[test]
fn menu_place_wire_drops_a_half_placed_shape() {
    let (mut app, _task) = Signex::new();
    app.interaction_state.current_tool = Tool::Line;
    app.interaction_state.shape_anchor = Some(Point::new(1.0, 1.0));

    let _ = app.update(Message::Menu(MenuMessage::PlaceWire));

    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
    assert!(app.interaction_state.shape_anchor.is_none());
}

#[test]
fn reselecting_the_active_tool_keeps_the_gesture() {
    let (mut app, _task) = Signex::new();
    app.interaction_state.current_tool = Tool::Line;
    app.interaction_state.shape_anchor = Some(Point::new(1.0, 1.0));

    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Line)));

    assert!(app.interaction_state.shape_anchor.is_some());
}

#[test]
fn double_click_shape_commit_snaps_to_the_grid() {
    let (mut app, _task) = Signex::new();
    add_tab(&mut app, &PathBuf::from("snap.snxsch"));
    activate(&mut app, 0);
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 2.54;
    app.interaction_state.current_tool = Tool::Rectangle;
    app.interaction_state.shape_anchor = Some(Point::new(0.0, 0.0));

    let _ = app.update(Message::CanvasEvent(CanvasEvent::DoubleClicked {
        world_x: 5.0,
        world_y: 5.0,
        screen_x: 0.0,
        screen_y: 0.0,
    }));

    let drawings = &app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .drawings;
    let [SchDrawing::Rect { end, .. }] = drawings.as_slice() else {
        panic!("expected exactly one rectangle, got {drawings:?}");
    };
    assert!(
        (end.x - 5.08).abs() < 1e-4 && (end.y - 5.08).abs() < 1e-4,
        "end = {end:?}"
    );
}

#[test]
fn tab_switch_drops_in_flight_drawing_buffers() {
    let (mut app, _task) = Signex::new();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    add_tab(&mut app, &PathBuf::from("a.snxsch"));
    add_tab(&mut app, &PathBuf::from("b.snxsch"));
    activate(&mut app, 0);

    // Half-placed wire in tab A.
    app.interaction_state.current_tool = Tool::Wire;
    app.interaction_state.wire_drawing = true;
    app.interaction_state.wire_points = vec![Point::new(1.0, 1.0)];
    let _ = app.update(Message::Tab {
        window_id: window,
        msg: TabMessage::Select(1),
    });
    assert!(!app.interaction_state.wire_drawing);
    assert!(app.interaction_state.wire_points.is_empty());

    // Half-placed line in tab A must not complete into tab B.
    activate(&mut app, 0);
    app.interaction_state.current_tool = Tool::Line;
    app.interaction_state.shape_anchor = Some(Point::new(100.0, 100.0));
    let _ = app.update(Message::Tab {
        window_id: window,
        msg: TabMessage::Select(1),
    });
    assert!(app.interaction_state.shape_anchor.is_none());
    let _ = app.update(Message::CanvasEvent(CanvasEvent::DoubleClicked {
        world_x: 5.08,
        world_y: 5.08,
        screen_x: 0.0,
        screen_y: 0.0,
    }));
    let drawings = &app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .drawings;
    assert!(drawings.is_empty(), "tab B got {drawings:?}");
}
