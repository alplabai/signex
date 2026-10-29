//! #1246 — with the electrical-grid toggle (Shift+E) on, a wire click within
//! the hotspot radius of an off-grid pin end or wire end lands exactly on it;
//! with the toggle off the click snaps to the grid as before.

use signex_app::app::{Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::schematic_runtime::SchematicRenderCache;
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol,
    SymbolTransform, Wire,
};

/// The pin tip, 0.3 mm off the 1.27 mm grid in x and 0.1 mm in y.
const PIN_TIP: (f64, f64) = (12.7 + 0.3, 6.35 + 0.1);

fn one_pin_part(pin_at: Point) -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: pin_at,
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn app_with_off_grid_pin_and_wire(hotspots: bool) -> (Signex, std::path::PathBuf) {
    let symbol = Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "R".to_string(),
        position: Point::new(PIN_TIP.0, PIN_TIP.1),
        ..Default::default()
    };
    app_with_symbol(hotspots, Point::new(0.0, 0.0), symbol)
}

fn app_with_symbol(hotspots: bool, pin_at: Point, symbol: Symbol) -> (Signex, std::path::PathBuf) {
    app_with_symbol_and_wire(hotspots, pin_at, symbol, None)
}

fn app_with_symbol_and_wire(
    hotspots: bool,
    pin_at: Point,
    symbol: Symbol,
    wire: Option<Wire>,
) -> (Signex, std::path::PathBuf) {
    let mut sheet = SchematicSheet::default();
    sheet.wires.extend(wire);
    sheet
        .lib_symbols
        .insert("R".to_string(), one_pin_part(pin_at));
    sheet.symbols.push(symbol);
    let path = std::path::PathBuf::from("hotspot-snap.snxsch");
    let engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.snap_hotspots = hotspots;
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "hotspot-snap".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(SchematicRenderCache::from_sheet(&sheet)));
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    (app, path)
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

/// Draw one wire from a grid point to a click near the pin and return it.
fn draw_wire_to_pin(app: &mut Signex, path: &std::path::PathBuf) -> Wire {
    draw_wire_to(app, path, PIN_TIP.0 + 0.4, PIN_TIP.1 + 0.3)
}

/// Draw one wire from a grid point to the click at `(x, y)` and return the
/// last segment placed.
fn draw_wire_to(app: &mut Signex, path: &std::path::PathBuf, x: f64, y: f64) -> Wire {
    click(app, 0.0, 6.35);
    click(app, x, y);
    app.document_state.engines[path]
        .document()
        .wires
        .last()
        .expect("a wire was placed")
        .clone()
}

#[test]
fn wire_click_near_an_off_grid_pin_lands_exactly_on_the_pin_end() {
    let (mut app, path) = app_with_off_grid_pin_and_wire(true);
    let wire = draw_wire_to_pin(&mut app, &path);
    assert_eq!((wire.end.x, wire.end.y), PIN_TIP);
}

#[test]
fn toggle_off_snaps_the_same_click_to_the_grid() {
    let (mut app, path) = app_with_off_grid_pin_and_wire(false);
    let wire = draw_wire_to_pin(&mut app, &path);
    assert_ne!((wire.end.x, wire.end.y), PIN_TIP);
    let g = 1.27;
    assert!(((wire.end.x / g).round() * g - wire.end.x).abs() < 1e-9);
}

/// A part rotated 90 degrees, mirrored, with its pin at a non-zero local
/// position: the snap target must be the pin tip as `SymbolTransform` places
/// it, not the symbol origin plus the local offset.
fn transformed_symbol() -> (Symbol, Point) {
    let local = Point::new(3.0, 1.0);
    let symbol = Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "R".to_string(),
        position: Point::new(25.4, 12.7),
        rotation: 90.0,
        mirror_x: true,
        ..Default::default()
    };
    (symbol, local)
}

fn off_grid(v: f64) -> bool {
    let g = 1.27;
    ((v / g).round() * g - v).abs() > 0.05
}

#[test]
fn wire_click_lands_on_the_pin_tip_of_a_rotated_mirrored_symbol() {
    let (symbol, local) = transformed_symbol();
    let tip = SymbolTransform::from_symbol(&symbol).apply(local);
    assert!(off_grid(tip.x) || off_grid(tip.y), "tip must be off-grid");
    assert_ne!(
        (tip.x, tip.y),
        (symbol.position.x + local.x, symbol.position.y + local.y),
        "the transform must move the tip"
    );
    let (mut app, path) = app_with_symbol(true, local, symbol);
    let wire = draw_wire_to(&mut app, &path, tip.x + 0.4, tip.y + 0.3);
    assert_eq!((wire.end.x, wire.end.y), (tip.x, tip.y));
}

#[test]
fn wire_click_outside_the_radius_falls_back_to_the_grid() {
    let (symbol, local) = transformed_symbol();
    let tip = SymbolTransform::from_symbol(&symbol).apply(local);
    let (mut app, path) = app_with_symbol(true, local, symbol);
    let wire = draw_wire_to(&mut app, &path, tip.x + 3.3, tip.y + 3.1);
    let g = 1.27;
    assert!(((wire.end.x / g).round() * g - wire.end.x).abs() < 1e-9);
    assert!(((wire.end.y / g).round() * g - wire.end.y).abs() < 1e-9);
    assert_ne!((wire.end.x, wire.end.y), (tip.x, tip.y));
}

#[test]
fn click_on_the_grid_point_next_to_a_wire_end_stays_on_the_grid() {
    let (mut app, path) = app_with_off_grid_pin_and_wire(true);
    click(&mut app, 0.0, 25.4);
    click(&mut app, 5.08, 25.4);
    // One grid step from the end of the wire just placed.
    click(&mut app, 6.35, 25.4);
    let wire = app.document_state.engines[&path]
        .document()
        .wires
        .last()
        .expect("a wire was placed")
        .clone();
    assert_eq!((wire.start.x, wire.end.x), (5.08, 6.35));
    assert_eq!(wire.end.y, 25.4);
}

/// Moving a symbol whose pin tip would land 0.3 / 0.1 mm beside an off-grid
/// wire end: with the toggle on the tip lands exactly on the wire end, with
/// it off the tip lands on the grid.
fn move_pin_tip_near_wire_end(hotspots: bool) -> (f64, f64) {
    let symbol = Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "R".to_string(),
        position: Point::new(12.7, 6.35),
        ..Default::default()
    };
    let id = symbol.uuid;
    let wire = Wire {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(50.0, 50.0),
        end: Point::new(25.4 + 0.3, 12.7 + 0.1),
        stroke_width: 0.0,
    };
    let (mut app, path) =
        app_with_symbol_and_wire(hotspots, Point::new(0.0, 0.0), symbol, Some(wire));
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    app.interaction_state.active_canvas_mut().selected =
        vec![signex_types::schematic::SelectedItem::new(
            id,
            signex_types::schematic::SelectedKind::Symbol,
        )];
    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 12.8,
        dy: 6.4,
        connections: Default::default(),
    }));
    let moved = &app.document_state.engines[&path].document().symbols[0];
    (moved.position.x, moved.position.y)
}

#[test]
fn moving_a_pin_tip_near_a_wire_end_lands_exactly_on_it() {
    let (x, y) = move_pin_tip_near_wire_end(true);
    assert!(
        (x - 25.7).abs() < 1e-9 && (y - 12.8).abs() < 1e-9,
        "({x}, {y})"
    );
}

#[test]
fn moving_with_the_toggle_off_lands_on_the_grid() {
    let (x, y) = move_pin_tip_near_wire_end(false);
    assert!(
        (x - 25.4).abs() < 1e-9 && (y - 12.7).abs() < 1e-9,
        "({x}, {y})"
    );
}
