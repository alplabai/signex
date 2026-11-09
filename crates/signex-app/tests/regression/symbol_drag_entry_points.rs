//! Job 18 — a component drag runs the local net guard and the walked ghost
//! split on every way the UI moves a component, and what they show is what
//! the drop commits. The ghost is `MovePreview` (a preview with the base nets:
//! the local guard); the drop is `Engine::execute` with none (the guard over
//! every net). Each test drives real messages through `Signex::update`:
//! canvas pointer events for a mouse drag, a box select then a drag, the
//! Active Bar's click-click Move Selection, and the Move Selection dialog.
//!
//! There is no arrow-key nudge to cover: the keymap has no binding that moves
//! the selection by a step (`M` / `M M` arm the click-click move).

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{Message, MoveSelectionMsg, Signex, TabInfo, TabKind};
use signex_app::canvas::{CanvasEvent, CanvasState};
use signex_types::schematic::{
    FillType, Graphic, LibGraphic, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire,
};
use std::path::PathBuf;

use super::click_click_move::{canvas_event_placing, left_press, screen_at};
use iced::{Event, mouse};

fn resistor_lib() -> LibSymbol {
    let pin = |number: &str, x: f64| LibPin {
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
    };
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
        graphics: vec![LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(-3.0, -2.0),
                end: Point::new(3.0, 2.0),
                width: 0.25,
                fill: FillType::Background,
            },
        }],
        pins: vec![pin("1", -5.0), pin("2", 5.0)],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// Ten resistors in two rows, each wired on both pins to a stub, the stubs
/// of one column sharing a net label name.
fn rig() -> (Signex, Vec<SelectedItem>) {
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.lib_symbols.insert("R".to_string(), resistor_lib());
    let mut symbols = Vec::new();
    for n in 0..10 {
        let (x, y) = (30.0 + 25.0 * (n % 5) as f64, 40.0 + 30.0 * (n / 5) as f64);
        let id = uuid::Uuid::now_v7();
        sheet.symbols.push(Symbol {
            uuid: id,
            lib_id: "R".to_string(),
            reference: format!("R{}", n + 1),
            position: Point::new(x, y),
            unit: 1,
            ..Default::default()
        });
        symbols.push(SelectedItem::new(id, SelectedKind::Symbol));
        for (from, to) in [(x - 5.0, x - 10.0), (x + 5.0, x + 8.0)] {
            sheet.wires.push(Wire {
                uuid: uuid::Uuid::now_v7(),
                start: Point::new(from, y),
                end: Point::new(to, y),
                ..Default::default()
            });
        }
    }
    let path = PathBuf::from("symbol-drag-entry-points.snxsch");
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let cache = signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet);
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "entry-points".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.ui_state.snap_enabled = false;
    app.ui_state.move_keeps_wires = true;
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(cache));
    (app, symbols)
}

fn sheet(app: &Signex) -> SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .clone()
}

/// The sheet's geometry, ids left out: symbol positions, wire ends, dots.
fn geometry(sheet: &SchematicSheet) -> String {
    let key = |p: &Point| (p.x.to_bits(), p.y.to_bits());
    let mut symbols: Vec<_> = sheet
        .symbols
        .iter()
        .map(|s| (s.reference.clone(), key(&s.position)))
        .collect();
    let mut wires: Vec<_> = sheet
        .wires
        .iter()
        .map(|w| (key(&w.start), key(&w.end)))
        .collect();
    let mut dots: Vec<_> = sheet.junctions.iter().map(|j| key(&j.position)).collect();
    symbols.sort();
    wires.sort();
    dots.sort();
    format!("{symbols:?} {wires:?} {dots:?}")
}

/// What the preview (local guard, walked split inputs) shows for `moves`.
fn ghost(before: &SchematicSheet, moves: &[(SelectedItem, f64, f64)]) -> SchematicSheet {
    signex_engine::MovePreview::new(before)
        .moved_with(moves, true)
        .0
}

fn world_of(app: &Signex, symbol: SelectedItem) -> (f32, f32) {
    let s = sheet(app);
    let p = s
        .symbols
        .iter()
        .find(|s| s.uuid == symbol.uuid)
        .expect("symbol")
        .position;
    (p.x as f32, p.y as f32)
}

fn cursor(position: iced::Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position })
}

fn release() -> Event {
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
}

/// Press on `grab`, drag it by `(dx, dy)` world mm in steps, release; the
/// `MoveSelected` message the canvas publishes has gone through `update`.
fn drag(app: &mut Signex, grab: (f32, f32), dx: f32, dy: f32) -> (f64, f64) {
    let mut state = CanvasState::default();
    let start = screen_at(app, grab.0, grab.1);
    let _ = canvas_event_placing(app, &mut state, cursor(start), start);
    let _ = canvas_event_placing(app, &mut state, left_press(), start);
    for step in 1..=4 {
        let f = step as f32 / 4.0;
        let at = screen_at(app, grab.0 + dx * f, grab.1 + dy * f);
        let _ = canvas_event_placing(app, &mut state, cursor(at), at);
    }
    let end = screen_at(app, grab.0 + dx, grab.1 + dy);
    let dropped = canvas_event_placing(app, &mut state, release(), end);
    match dropped {
        Some(Message::CanvasEvent(CanvasEvent::MoveSelected { dx, dy, .. })) => (dx, dy),
        other => panic!("the release did not drop the move: {other:?}"),
    }
}

fn assert_drop_equals_ghost(
    app: &mut Signex,
    selection: &[SelectedItem],
    drop: impl FnOnce(&mut Signex) -> (f64, f64),
) {
    let before = sheet(app);
    let (dx, dy) = drop(app);
    let moves: Vec<_> = selection.iter().map(|item| (*item, dx, dy)).collect();
    let shown = ghost(&before, &moves);
    let committed = sheet(app);
    assert_ne!(geometry(&committed), geometry(&before), "nothing moved");
    assert_eq!(
        geometry(&committed),
        geometry(&shown),
        "the drop differs from the ghost"
    );
}

#[test]
fn a_mouse_drag_of_one_symbol_drops_what_the_ghost_shows() {
    let (mut app, symbols) = rig();
    let one = symbols[2];
    app.interaction_state.active_canvas_mut().selected = vec![one];
    let grab = world_of(&app, one);
    assert_drop_equals_ghost(&mut app, &[one], |app| drag(app, grab, 7.5, 12.5));
}

#[test]
fn a_box_select_then_a_drag_of_many_symbols_drops_what_the_ghost_shows() {
    let (mut app, symbols) = rig();
    // Box-select the first row: a real drag on empty canvas.
    let mut state = CanvasState::default();
    let from = screen_at(&app, 8.0, 25.0);
    let to = screen_at(&app, 150.0, 52.0);
    let _ = canvas_event_placing(&mut app, &mut state, cursor(from), from);
    let _ = canvas_event_placing(&mut app, &mut state, left_press(), from);
    let _ = canvas_event_placing(&mut app, &mut state, cursor(to), to);
    let _ = canvas_event_placing(&mut app, &mut state, release(), to);
    let selected = app.interaction_state.active_canvas().selected.clone();
    let symbols_selected: Vec<_> = selected
        .iter()
        .copied()
        .filter(|i| i.kind == SelectedKind::Symbol)
        .collect();
    assert_eq!(
        symbols_selected.len(),
        5,
        "the box selected the first row: {selected:?}"
    );
    assert!(symbols_selected.iter().all(|s| symbols[..5].contains(s)));
    let grab = world_of(&app, symbols[0]);
    // The box also took the wires; drag them all, as the user would.
    let all = selected.clone();
    assert_drop_equals_ghost(&mut app, &all, |app| drag(app, grab, -5.0, 10.0));
}

#[test]
fn the_click_click_move_selection_drops_what_the_ghost_shows() {
    let (mut app, symbols) = rig();
    let chosen = vec![symbols[1], symbols[6]];
    app.interaction_state.active_canvas_mut().selected = chosen.clone();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::MoveSelection,
    )));
    let reference = world_of(&app, symbols[1]);
    assert_drop_equals_ghost(&mut app, &chosen, |app| {
        let mut state = CanvasState::default();
        let pick = screen_at(app, reference.0, reference.1);
        let _ = canvas_event_placing(app, &mut state, left_press(), pick);
        let target = screen_at(app, reference.0 + 10.0, reference.1 + 5.0);
        let _ = canvas_event_placing(app, &mut state, cursor(target), target);
        match canvas_event_placing(app, &mut state, left_press(), target) {
            Some(Message::CanvasEvent(CanvasEvent::MoveSelected { dx, dy, .. })) => (dx, dy),
            other => panic!("the second press did not drop the move: {other:?}"),
        }
    });
}

#[test]
fn the_move_selection_dialog_commits_what_the_ghost_shows() {
    let (mut app, symbols) = rig();
    let chosen = vec![symbols[3], symbols[4], symbols[8]];
    app.interaction_state.active_canvas_mut().selected = chosen.clone();
    assert_drop_equals_ghost(&mut app, &chosen, |app| {
        let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Open));
        let _ = app.update(Message::MoveSelection(MoveSelectionMsg::DxChanged(
            "5".to_string(),
        )));
        let _ = app.update(Message::MoveSelection(MoveSelectionMsg::DyChanged(
            "-10".to_string(),
        )));
        let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));
        (5.0, -10.0)
    });
}
