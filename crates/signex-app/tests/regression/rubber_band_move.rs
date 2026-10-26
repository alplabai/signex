//! #963 — moving a schematic symbol drags the wires attached to its pins
//! ("Keep wires connected when moving", default on) through every move
//! entry point: mouse drag and click-click Move (both commit through
//! `CanvasEvent::MoveSelected`), the Move By dialog and Align. With the
//! preference off the move is the plain translate it always was.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{Message, MoveSelectionMsg, Signex};
use signex_app::canvas::CanvasEvent;
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, Wire,
};
use std::path::PathBuf;
use uuid::Uuid;

type Segment = ((f64, f64), (f64, f64));

fn lib_pin(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// `R1` at (50, 50), pins at (45, 50) and (55, 50), one wire into each.
fn app_with_resistor() -> (Signex, PathBuf, Uuid) {
    let mut sheet = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.lib_symbols.insert(
        "R".to_string(),
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
            pins: vec![lib_pin("1", -5.0), lib_pin("2", 5.0)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
    let symbol = Uuid::new_v4();
    sheet.symbols.push(Symbol {
        uuid: symbol,
        lib_id: "R".to_string(),
        reference: "R1".to_string(),
        position: Point::new(50.0, 50.0),
        ..Default::default()
    });
    for (a, b) in [((30.0, 50.0), (45.0, 50.0)), ((55.0, 50.0), (75.0, 50.0))] {
        sheet.wires.push(Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
            ..Default::default()
        });
    }

    let path = PathBuf::from("rubber-band-move.snxsch");
    let engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = false;
    app.ui_state.move_keeps_wires = true;
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "rubber-band-move".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(symbol, SelectedKind::Symbol)];
    (app, path, symbol)
}

fn segments(app: &Signex, path: &PathBuf) -> Vec<Segment> {
    app.document_state.engines[path]
        .document()
        .wires
        .iter()
        .map(|w| ((w.start.x, w.start.y), (w.end.x, w.end.y)))
        .collect()
}

fn symbol_y(app: &Signex, path: &PathBuf) -> f64 {
    app.document_state.engines[path].document().symbols[0]
        .position
        .y
}

/// Both wires still reach the (moved) pins and every wire is orthogonal.
fn assert_connected_after_dy(segs: &[Segment], dy: f64) {
    assert_eq!(segs.len(), 4, "one corner wire per attached wire: {segs:?}");
    assert!(
        segs.iter().all(|s| s.0.0 == s.1.0 || s.0.1 == s.1.1),
        "diagonal wire: {segs:?}"
    );
    let touches = |p: (f64, f64)| segs.iter().any(|s| s.0 == p || s.1 == p);
    assert!(touches((45.0, 50.0 + dy)), "pin 1 tip unwired: {segs:?}");
    assert!(touches((55.0, 50.0 + dy)), "pin 2 tip unwired: {segs:?}");
}

fn drop_by(app: &mut Signex, dx: f64, dy: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected { dx, dy }));
}

#[test]
fn mouse_drag_drags_the_attached_wires() {
    let (mut app, path, _) = app_with_resistor();
    drop_by(&mut app, 0.0, 10.0);
    assert_eq!(symbol_y(&app, &path), 60.0);
    assert_connected_after_dy(&segments(&app, &path), 10.0);
}

#[test]
fn click_click_move_selection_drags_the_attached_wires() {
    let (mut app, path, _) = app_with_resistor();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::MoveSelection,
    )));
    assert!(app.ui_state.move_pick.is_some(), "the lift must arm");
    drop_by(&mut app, 0.0, 10.0);
    assert_connected_after_dy(&segments(&app, &path), 10.0);
}

#[test]
fn move_by_dialog_drags_the_attached_wires() {
    let (mut app, path, _) = app_with_resistor();
    app.ui_state.move_selection.dx = "0".to_string();
    app.ui_state.move_selection.dy = "10".to_string();
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));
    assert_eq!(symbol_y(&app, &path), 60.0);
    assert_connected_after_dy(&segments(&app, &path), 10.0);
}

#[test]
fn the_whole_drag_is_one_undo_step() {
    let (mut app, path, _) = app_with_resistor();
    let before = segments(&app, &path);
    drop_by(&mut app, 0.0, 10.0);
    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    assert_eq!(segments(&app, &path), before);
    assert_eq!(symbol_y(&app, &path), 50.0);
}

#[test]
fn with_the_preference_off_wires_stay_behind_as_before() {
    let (mut app, path, _) = app_with_resistor();
    let before = segments(&app, &path);
    app.ui_state.move_keeps_wires = false;

    drop_by(&mut app, 0.0, 10.0);
    assert_eq!(symbol_y(&app, &path), 60.0);
    assert_eq!(segments(&app, &path), before, "plain translate");

    app.ui_state.move_selection.dx = "0".to_string();
    app.ui_state.move_selection.dy = "5".to_string();
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));
    assert_eq!(symbol_y(&app, &path), 65.0);
    assert_eq!(segments(&app, &path), before, "Move By is plain too");
}

#[test]
fn align_is_one_command_and_one_undo_step() {
    let (mut app, path, symbol) = app_with_resistor();
    // A second, stationary-row symbol so Align Top has something to align to.
    {
        let engine = app.document_state.engines.get_mut(&path).unwrap();
        let mut sheet = engine.document().clone();
        let id = Uuid::new_v4();
        let mut second = sheet.symbols[0].clone();
        second.uuid = id;
        second.reference = "R2".to_string();
        second.position = Point::new(90.0, 70.0);
        sheet.symbols.push(second);
        *engine = signex_engine::Engine::new(sheet).expect("engine");
    }
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(symbol, SelectedKind::Symbol),
        SelectedItem::new(
            app.document_state.engines[&path].document().symbols[1].uuid,
            SelectedKind::Symbol,
        ),
    ];
    let before = segments(&app, &path);
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::AlignTop,
    )));
    let doc = app.document_state.engines[&path].document();
    assert_eq!(doc.symbols[0].position.y, doc.symbols[1].position.y);
    let after = segments(&app, &path);
    assert!(
        after.iter().all(|s| s.0.0 == s.1.0 || s.0.1 == s.1.1),
        "{after:?}"
    );

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    assert_eq!(
        segments(&app, &path),
        before,
        "one undo reverts the whole align"
    );
    assert_eq!(symbol_y(&app, &path), 50.0);
}

#[test]
fn a_pin_that_cannot_be_kept_connected_reaches_the_messages_panel() {
    let (mut app, path, _) = app_with_resistor();
    {
        // Foreign wires on both L routes of pin 1's wire.
        let engine = app.document_state.engines.get_mut(&path).unwrap();
        let mut sheet = engine.document().clone();
        for (a, b) in [((45.0, 55.0), (45.0, 58.0)), ((30.0, 55.0), (30.0, 58.0))] {
            sheet.wires.push(Wire {
                uuid: Uuid::new_v4(),
                start: Point::new(a.0, a.1),
                end: Point::new(b.0, b.1),
                ..Default::default()
            });
        }
        *engine = signex_engine::Engine::new(sheet).expect("engine");
    }
    let _ = signex_app::diagnostics::init_logging();
    drop_by(&mut app, 0.0, 10.0);
    let reported = signex_app::diagnostics::recent_entries()
        .into_iter()
        .any(|entry| entry.message.contains("could not keep R1 pin 1 connected"));
    assert!(
        reported,
        "the engine warning must reach the diagnostics ring"
    );
}
