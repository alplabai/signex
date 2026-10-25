//! #962 — under the Altium profile Space rotates the selected symbol 90
//! degrees counter-clockwise and Shift+Space 90 degrees clockwise. Drives
//! the real keymap path (`UiMsg::KeymapStroke` -> `active_keymap` ->
//! command bridge -> engine) and Undo.
//!
//! Directions are asserted ON SCREEN, through `SymbolTransform::apply`, not
//! on the raw `rotation` field: the render path rotates and then mirrors,
//! so the stored angle turns the other way when exactly one of
//! `mirror_x` / `mirror_y` is set, and the engine compensates per symbol.

use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, UiMsg};
use signex_app::keymap::{KeyStroke, KeyToken, Modifiers};
use signex_types::schematic::{
    Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, SymbolTransform,
};
use std::path::PathBuf;

fn symbol(reference: &str, mirror_x: bool, mirror_y: bool) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        mirror_x,
        mirror_y,
        ..Default::default()
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    }
}

fn app_with_selected_symbol(mirror_x: bool, mirror_y: bool) -> Signex {
    let symbol = symbol("R1", mirror_x, mirror_y);
    let item = SelectedItem::new(symbol.uuid, SelectedKind::Symbol);
    let sheet = sheet_with(vec![symbol]);
    let path = PathBuf::from("rotate-direction.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _t) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "rotate-direction".to_string(),
        path: path.clone(),
        cached_document: None,
        pcb_revision: 0,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app
}

fn press_space(app: &mut Signex, shift: bool) {
    let stroke = KeyStroke {
        modifiers: Modifiers {
            shift,
            ..Modifiers::default()
        },
        key: KeyToken::Space,
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke,
    }));
}

/// Where a pin pointing east in library space ends up on screen
/// (y-down), relative to the symbol origin.
fn pin_on_screen(symbol: &Symbol) -> (f64, f64) {
    let p = SymbolTransform::from_symbol(symbol).apply(Point::new(1.0, 0.0));
    (p.x - symbol.position.x, p.y - symbol.position.y)
}

fn assert_close(actual: (f64, f64), expected: (f64, f64), what: &str) {
    assert!(
        (actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9,
        "{what}: got {actual:?}, expected {expected:?}"
    );
}

/// `v` turned 90 degrees counter-clockwise on a y-down screen.
fn ccw(v: (f64, f64)) -> (f64, f64) {
    (v.1, -v.0)
}

/// `v` turned 90 degrees clockwise on a y-down screen.
fn cw(v: (f64, f64)) -> (f64, f64) {
    (-v.1, v.0)
}

fn first_symbol(app: &Signex) -> Symbol {
    let engine = app.document_state.active_engine().expect("engine");
    engine.document().symbols[0].clone()
}

#[test]
fn space_turns_counterclockwise_and_shift_space_clockwise_on_screen() {
    for (mx, my) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut app = app_with_selected_symbol(mx, my);
        let before = pin_on_screen(&first_symbol(&app));
        press_space(&mut app, false);
        assert_close(
            pin_on_screen(&first_symbol(&app)),
            ccw(before),
            &format!("Space, mirror_x={mx} mirror_y={my}"),
        );

        let mut app = app_with_selected_symbol(mx, my);
        press_space(&mut app, true);
        assert_close(
            pin_on_screen(&first_symbol(&app)),
            cw(before),
            &format!("Shift+Space, mirror_x={mx} mirror_y={my}"),
        );
    }
}

#[test]
fn mixed_mirrored_and_plain_selection_turns_the_same_way_on_screen() {
    let plain = symbol("R1", false, false);
    let mirrored = symbol("R2", false, true);
    let items = vec![
        SelectedItem::new(plain.uuid, SelectedKind::Symbol),
        SelectedItem::new(mirrored.uuid, SelectedKind::Symbol),
    ];
    let mut engine = signex_engine::Engine::new(sheet_with(vec![plain, mirrored])).expect("engine");
    let before: Vec<_> = engine
        .document()
        .symbols
        .iter()
        .map(pin_on_screen)
        .collect();
    engine
        .execute(signex_engine::Command::RotateSelection {
            items,
            angle_degrees: 90.0,
        })
        .expect("rotate");
    for (symbol, before) in engine.document().symbols.iter().zip(before) {
        assert_close(
            pin_on_screen(symbol),
            ccw(before),
            &format!("mixed selection, {}", symbol.reference),
        );
    }
}

#[test]
fn undo_and_redo_step_through_ccw_then_cw_on_a_mirrored_symbol() {
    let mut app = app_with_selected_symbol(false, true);
    let start = pin_on_screen(&first_symbol(&app));
    press_space(&mut app, false);
    let after_ccw = pin_on_screen(&first_symbol(&app));
    assert_close(after_ccw, ccw(start), "Space on a mirrored symbol");
    press_space(&mut app, true);
    assert_close(pin_on_screen(&first_symbol(&app)), start, "CCW then CW");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_close(pin_on_screen(&first_symbol(&app)), after_ccw, "undo CW");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_close(pin_on_screen(&first_symbol(&app)), start, "undo CCW");

    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_close(pin_on_screen(&first_symbol(&app)), after_ccw, "redo CCW");
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_close(pin_on_screen(&first_symbol(&app)), start, "redo CW");
}
