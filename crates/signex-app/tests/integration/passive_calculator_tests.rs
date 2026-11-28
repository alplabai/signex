use signex_app::app::{Message, OverlayMsg, Signex};
use signex_app::menu_bar::MenuMessage;
use signex_widgets::passive_calculator::{
    CalculatorMessage, CalculatorTab, ComponentKind, RkmEncoderMessage,
};

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

/// #2992 (mockup row 17): the RKM Value field is a text input; its text
/// reaches the encoder through `Signex::update` and drives the code.
#[test]
fn a_typed_rkm_value_drives_the_code_through_the_app() {
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));
    let _ = app.update(Message::PassiveCalculator(CalculatorMessage::TabChanged(
        CalculatorTab::RkmEncoder,
    )));

    let task = app.update(Message::PassiveCalculator(CalculatorMessage::RkmEncoder(
        RkmEncoderMessage::ValueInputChanged("4.7".to_string()),
    )));
    assert_eq!(task.units(), 0, "a typed value is pure state, no task");
    let encoder = &app.ui_state.passive_calculator.rkm_encoder;
    assert_eq!(encoder.value_input, "4.7");
    assert_eq!(encoder.code().value_code(), "4K7");

    let _ = app.update(Message::PassiveCalculator(CalculatorMessage::RkmEncoder(
        RkmEncoderMessage::ValueInputChanged("4.".to_string()),
    )));
    let encoder = &app.ui_state.passive_calculator.rkm_encoder;
    assert_eq!(encoder.value_input, "4.", "the mid-edit text is kept");
    assert_eq!(
        encoder.code().value_code(),
        "4K7",
        "a partial value keeps the last readable one"
    );
}
