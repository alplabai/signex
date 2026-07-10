use signex_app::app::{Message, Signex, WindowKind, WindowMsg};
use signex_app::menu_bar::MenuMessage;
use signex_passive_calculator::{CalculatorMessage, ComponentKind};

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
fn tools_menu_message_routes_to_the_calculator_open_flow() {
    let (mut app, _startup) = Signex::new();
    let _task = app.update(Message::Menu(MenuMessage::OpenPassiveCalculator));
    assert!(
        app.ui_state
            .windows
            .values()
            .all(|kind| !matches!(kind, WindowKind::PassiveCalculator))
    );
}

#[test]
fn opened_calculator_window_gets_its_role_and_title() {
    let (mut app, _startup) = Signex::new();
    let id = iced::window::Id::unique();
    let _task = app.update(Message::PassiveCalculatorOpened(id));
    assert!(matches!(
        app.ui_state.windows.get(&id),
        Some(WindowKind::PassiveCalculator)
    ));
    assert_eq!(app.title(id), "Signex — Passive Network Calculator");
}

#[test]
fn closing_calculator_window_removes_its_window_role() {
    let (mut app, _startup) = Signex::new();
    let id = iced::window::Id::unique();
    app.ui_state
        .windows
        .insert(id, WindowKind::PassiveCalculator);
    let _task = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));
    assert!(!app.ui_state.windows.contains_key(&id));
}
