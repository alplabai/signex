//! #980 — Space rotates the selection counter-clockwise ON SCREEN and
//! Shift+Space clockwise, like the schematic. Drives the real chain:
//! iced key event -> `SymbolCanvas` `Program::update` -> `CanvasAction`
//! -> `SymbolEditorMsg` -> `apply_symbol_primitive_edit`, then undo.
//!
//! World coordinates are y-up, so a counter-clockwise turn on screen is
//! `(x, y) -> (-y, x)`; a pin pointing Right ends up pointing Up.

use super::*;
use crate::app::SymbolEditorState;
use crate::library::editor::standalone::symbol::symbol_action_to_primitive_msg;
use crate::library::editor::symbol::updates::apply_symbol_primitive_edit;
use iced::event::Event;
use iced::{Rectangle, keyboard, mouse};
use signex_library::{PinOrientation, Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind};
use std::path::PathBuf;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 900.0,
    height: 600.0,
};

fn editor_with(symbol: Symbol, selected: SymbolSelection) -> SymbolEditorState {
    let mut editor =
        SymbolEditorState::new(PathBuf::from("t.snxsym"), SymbolFile::from_symbol(symbol));
    editor.pending_view = None;
    editor.selected = Some(selected);
    editor
}

/// Press Space (optionally with Shift) over the canvas and apply what
/// it publishes, exactly as the dispatcher would.
fn press_space(editor: &mut SymbolEditorState, shift: bool) {
    let space = keyboard::Key::Named(keyboard::key::Named::Space);
    let modifiers = if shift {
        keyboard::Modifiers::SHIFT
    } else {
        keyboard::Modifiers::default()
    };
    let event = Event::Keyboard(keyboard::Event::KeyPressed {
        key: space.clone(),
        modified_key: space,
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Space),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    });
    let mut cstate = CanvasState::default();
    let canvas = SymbolCanvas::new(
        editor.primitive(),
        editor.selected.clone(),
        editor.tool,
        &editor.polygon_vertices,
        editor.active_part,
        false,
        &editor.camera,
        1.27,
        true,
        crate::render_config::GridStyle::default(),
        true,
        Color::WHITE,
        Color::from_rgb(0.2, 0.4, 0.9),
        Color::BLACK,
        Color::BLACK,
        Color::BLACK,
    );
    let action = canvas::Program::update(
        &canvas,
        &mut cstate,
        &event,
        BOUNDS,
        mouse::Cursor::Unavailable,
    );
    let (published, _, _) = action.expect("Space is claimed").into_inner();
    apply_symbol_primitive_edit(
        editor,
        symbol_action_to_primitive_msg(published.expect("Space publishes a rotate")),
    );
}

fn undo(editor: &mut SymbolEditorState) {
    apply_symbol_primitive_edit(editor, crate::library::messages::SymbolEditorMsg::Undo);
}

fn right_pin_symbol() -> Symbol {
    let mut s = Symbol::empty("T");
    let mut p = signex_library::SymbolPin::new("1", "P1");
    p.position = [2.0, 1.0];
    p.length = 2.54;
    p.orientation = PinOrientation::Right;
    s.pins.push(p);
    s
}

fn rect_symbol() -> Symbol {
    let mut s = Symbol::empty("T");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [1.0, 2.0],
            to: [3.0, 4.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    s
}

fn rect_corners(editor: &SymbolEditorState) -> ([f64; 2], [f64; 2]) {
    match &editor.primitive().graphics[0].kind {
        SymbolGraphicKind::Rectangle { from, to } => (*from, *to),
        _ => panic!("expected Rectangle"),
    }
}

#[test]
fn space_turns_a_selected_pin_counterclockwise_on_screen() {
    let mut e = editor_with(right_pin_symbol(), SymbolSelection::Pin(0));
    press_space(&mut e, false);
    assert_eq!(e.primitive().pins[0].orientation, PinOrientation::Up);
}

#[test]
fn shift_space_turns_a_selected_pin_clockwise_on_screen() {
    let mut e = editor_with(right_pin_symbol(), SymbolSelection::Pin(0));
    press_space(&mut e, true);
    assert_eq!(e.primitive().pins[0].orientation, PinOrientation::Down);
}

#[test]
fn space_turns_a_selected_graphic_counterclockwise_on_screen() {
    let mut e = editor_with(rect_symbol(), SymbolSelection::Graphic(0));
    press_space(&mut e, false);
    assert_eq!(rect_corners(&e), ([-2.0, 1.0], [-4.0, 3.0]));
}

#[test]
fn shift_space_turns_a_selected_graphic_clockwise_on_screen() {
    let mut e = editor_with(rect_symbol(), SymbolSelection::Graphic(0));
    press_space(&mut e, true);
    assert_eq!(rect_corners(&e), ([2.0, -1.0], [4.0, -3.0]));
}

#[test]
fn each_key_rotation_is_its_own_undo_step() {
    let mut e = editor_with(rect_symbol(), SymbolSelection::Graphic(0));
    press_space(&mut e, false);
    press_space(&mut e, true);
    assert_eq!(rect_corners(&e), ([1.0, 2.0], [3.0, 4.0]), "CCW then CW");

    undo(&mut e);
    assert_eq!(rect_corners(&e), ([-2.0, 1.0], [-4.0, 3.0]), "undo the CW");
    undo(&mut e);
    assert_eq!(rect_corners(&e), ([1.0, 2.0], [3.0, 4.0]), "undo the CCW");
}
