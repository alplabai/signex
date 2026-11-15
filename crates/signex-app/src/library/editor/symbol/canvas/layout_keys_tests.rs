//! #2095: the symbol canvas matches Ctrl+A / Ctrl+Z / Ctrl+Y on any layout
//! through `keymap::shortcut_char`; a Russian `я` on the Z key undoes.

use super::*;
use iced::event::Event;
use iced::keyboard::key::{Code, Physical};
use iced::{Rectangle, keyboard, mouse};
use signex_library::Symbol;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 900.0,
    height: 600.0,
};

/// What the canvas publishes for `logical` typed on physical `code`.
fn published(logical: &str, code: Code, modifiers: keyboard::Modifiers) -> Option<CanvasAction> {
    let key = keyboard::Key::Character(logical.into());
    let event = Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    });
    let symbol = Symbol::empty("T");
    let camera = crate::canvas::Camera::default();
    let canvas = SymbolCanvas::new(
        &symbol,
        None,
        SymbolTool::Select,
        &[],
        state::PartView::normal(1),
        false,
        &camera,
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
    let mut cstate = CanvasState::default();
    let action = canvas::Program::update(
        &canvas,
        &mut cstate,
        &event,
        BOUNDS,
        mouse::Cursor::Unavailable,
    )?;
    action.into_inner().0
}

#[test]
fn russian_ctrl_ya_undoes_and_ctrl_ef_selects_all() {
    let ctrl = keyboard::Modifiers::COMMAND;
    assert!(matches!(
        published("\u{44f}", Code::KeyZ, ctrl),
        Some(CanvasAction::Undo)
    ));
    assert!(matches!(
        published("\u{43d}", Code::KeyY, ctrl),
        Some(CanvasAction::Redo)
    ));
    assert!(matches!(
        published("\u{444}", Code::KeyA, ctrl),
        Some(CanvasAction::Select(SymbolSelection::All))
    ));
}

#[test]
fn greek_ctrl_zeta_undoes() {
    assert!(matches!(
        published("\u{3b6}", Code::KeyZ, keyboard::Modifiers::COMMAND),
        Some(CanvasAction::Undo)
    ));
}

#[test]
fn a_dvorak_letter_keeps_its_logical_meaning() {
    // Dvorak types `z` on the physical slash key: it still undoes, and the
    // `;` typed on the physical Z key does not.
    let ctrl = keyboard::Modifiers::COMMAND;
    assert!(matches!(
        published("z", Code::Slash, ctrl),
        Some(CanvasAction::Undo)
    ));
    assert!(published(";", Code::KeyZ, ctrl).is_none());
}
