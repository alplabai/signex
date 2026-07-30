// This integration-test binary discards fallible test-setup calls with
// `let _ = ...` routinely (not a production `Task` getting dropped — see
// GH #99 part 1). `[lints]` in Cargo.toml is package-scoped, not
// target-scoped, so each integration-test crate root needs its own allow.
#![allow(clippy::let_underscore_must_use)]

use signex_app::app::{Message, OverlayMsg, Signex};
use signex_app::menu_bar::MenuMessage;
use signex_widgets::passive_calculator::{CalculatorMessage, ComponentKind};

#[test]
fn calculator_messages_update_the_dedicated_control_state() {
    let (mut app, _startup) = Signex::new();
    let _task = app.update(Message::PassiveCalculator(CalculatorMessage::KindChanged(
        ComponentKind::Capacitor,
    )));
    assert_eq!(
        app.ui_state.passive_calculator.kind,
        ComponentKind::Capacitor
    );
}

#[test]
fn tools_menu_message_opens_the_modal_without_a_window_task() {
    let (mut app, _startup) = Signex::new();
    assert!(!app.ui_state.passive_calculator_open);

    let task = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));

    assert!(
        app.ui_state.passive_calculator_open,
        "the Tools menu entry should open the in-app modal"
    );
    assert_eq!(
        task.units(),
        0,
        "the modal is an overlay, so no OS window task should be emitted"
    );
}

#[test]
fn reopening_while_open_is_a_no_op() {
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));
    let _ = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));
    assert!(app.ui_state.passive_calculator_open);
}

/// Open and close are both `OverlayMsg` leaves, so the pair can be driven
/// without going through the menu at all — the route a keybinding or the
/// command registry would take.
#[test]
fn overlay_open_and_close_round_trip_without_the_menu() {
    let (mut app, _startup) = Signex::new();

    let task = app.update(Message::Overlay(OverlayMsg::OpenPassiveCalculator));
    assert!(app.ui_state.passive_calculator_open);
    assert_eq!(
        task.units(),
        0,
        "the modal is an overlay, so no OS window task should be emitted"
    );

    let _ = app.update(Message::Overlay(OverlayMsg::ClosePassiveCalculator));
    assert!(!app.ui_state.passive_calculator_open);
}

#[test]
fn close_message_dismisses_the_modal() {
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));

    let _task = app.update(Message::Overlay(OverlayMsg::ClosePassiveCalculator));

    assert!(!app.ui_state.passive_calculator_open);
}

#[test]
fn closing_the_modal_keeps_the_entered_state() {
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));
    let _ = app.update(Message::PassiveCalculator(CalculatorMessage::KindChanged(
        ComponentKind::Inductor,
    )));

    let _ = app.update(Message::Overlay(OverlayMsg::ClosePassiveCalculator));
    let _ = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));

    assert_eq!(
        app.ui_state.passive_calculator.kind,
        ComponentKind::Inductor,
        "reopening should show the state the user left behind"
    );
}
