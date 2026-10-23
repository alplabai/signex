// This integration-test binary discards fallible test-setup calls with
// `let _ = ...` routinely (not a production `Task` getting dropped — see
// GH #99 part 1). `[lints]` in Cargo.toml is package-scoped, not
// target-scoped, so each integration-test crate root needs its own allow.
#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

use signex_app::app::{Message, OverlayMsg, Signex, Tool};
use signex_app::diagnostics;
use signex_app::menu_bar::MenuMessage;
use signex_widgets::trace_calculator::{LengthUnit, TraceCalculatorMessage};

fn edit(app: &mut Signex, message: TraceCalculatorMessage) {
    let _ = app.update(Message::Overlay(OverlayMsg::PcbTraceCalculatorEdited(
        message,
    )));
}

fn panel_count(app: &Signex, needle: &str) -> usize {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .filter(|entry| entry.message.contains(needle))
        .count()
}

#[test]
fn tools_menu_message_opens_the_modal_without_a_window_task() {
    let (mut app, _startup) = Signex::new();
    assert!(!app.ui_state.pcb_trace_calculator_open);

    let task = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));

    assert!(
        app.ui_state.pcb_trace_calculator_open,
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
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));
    assert!(app.ui_state.pcb_trace_calculator_open);
}

#[test]
fn the_overlay_open_message_opens_the_modal_without_the_menu() {
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPcbTraceCalculator));
    assert!(app.ui_state.pcb_trace_calculator_open);
}

#[test]
fn edit_messages_update_the_dedicated_control_state() {
    let (mut app, _startup) = Signex::new();
    edit(
        &mut app,
        TraceCalculatorMessage::LengthUnitChanged(LengthUnit::Inch),
    );
    edit(
        &mut app,
        TraceCalculatorMessage::CurrentChanged("12".into()),
    );

    assert_eq!(
        app.ui_state.pcb_trace_calculator.length_unit,
        LengthUnit::Inch
    );
    assert_eq!(app.ui_state.pcb_trace_calculator.current, "12");
}

#[test]
fn close_message_dismisses_the_modal() {
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));

    let _ = app.update(Message::Overlay(OverlayMsg::ClosePcbTraceCalculator));

    assert!(!app.ui_state.pcb_trace_calculator_open);
}

#[test]
fn closing_the_modal_keeps_the_entered_state() {
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));
    edit(&mut app, TraceCalculatorMessage::CurrentChanged("7".into()));

    let _ = app.update(Message::Overlay(OverlayMsg::ClosePcbTraceCalculator));
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));

    assert_eq!(
        app.ui_state.pcb_trace_calculator.current, "7",
        "reopening should show the state the user left behind"
    );
}

#[test]
fn escape_closes_the_modal_and_keeps_the_active_tool() {
    let (mut app, _startup) = Signex::new();
    // Keep the first-run tour out of the ladder so the calculator is the
    // only rung, and arm a tool that the Select reset would visibly undo.
    app.ui_state.first_run_tour_open = false;
    app.interaction_state.current_tool = Tool::Wire;
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));

    let _ = app.update(Message::EscapePressed { window: None });

    assert!(
        !app.ui_state.pcb_trace_calculator_open,
        "Esc should dismiss the calculator"
    );
    assert_eq!(
        app.interaction_state.current_tool,
        Tool::Wire,
        "Esc must not fall through to the tool reset behind the modal"
    );
}

/// A calculation failure shows in the modal AND is a log record on the
/// Messages panel: the dialog is not a substitute for the log. The same
/// failure repeated by the next keystroke is logged once, not per edit.
///
/// Relies on the default log level: `SIGNEX_LOG=error` filters the `warn`
/// record out and fails this test.
#[test]
fn a_calculation_failure_reaches_the_messages_panel() {
    let _ = diagnostics::init_logging();
    let (mut app, _startup) = Signex::new();
    let marker = "Enter a numeric value for current.";
    assert_eq!(
        panel_count(&app, marker),
        0,
        "precondition: nothing logged yet"
    );

    edit(
        &mut app,
        TraceCalculatorMessage::CurrentChanged("five".into()),
    );

    assert!(
        app.ui_state.pcb_trace_calculator.calculation().is_err(),
        "the modal itself shows the failure"
    );
    assert_eq!(
        panel_count(&app, marker),
        1,
        "the failure must be on the Messages panel snapshot as well"
    );

    edit(
        &mut app,
        TraceCalculatorMessage::CurrentChanged("five".into()),
    );

    assert_eq!(
        panel_count(&app, marker),
        1,
        "an identical failing edit must not add a second entry"
    );
}

/// The live log showed one line seven times: a field typed valid, emptied,
/// valid, emptied. The same failure is logged once per modal session.
#[test]
fn replaying_valid_and_empty_input_logs_exactly_one_entry() {
    let _ = diagnostics::init_logging();
    let (mut app, _startup) = Signex::new();
    let marker = "Enter a numeric value for copper thickness.";
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));

    for text in ["1", "", "1", "", "1", ""] {
        edit(
            &mut app,
            TraceCalculatorMessage::ThicknessChanged(text.into()),
        );
    }

    assert_eq!(panel_count(&app, marker), 1);
}

/// Closing and reopening starts a new session: the same failure is
/// reported again.
#[test]
fn reopening_the_modal_reports_the_same_failure_again() {
    let _ = diagnostics::init_logging();
    let (mut app, _startup) = Signex::new();
    let marker = "Enter a numeric value for trace length.";
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));
    edit(&mut app, TraceCalculatorMessage::LengthChanged("".into()));
    let before = panel_count(&app, marker);
    assert_eq!(before, 1);

    let _ = app.update(Message::Overlay(OverlayMsg::ClosePcbTraceCalculator));
    let _ = app.update(Message::Menu(MenuMessage::OpenPcbTraceCalculator));
    edit(&mut app, TraceCalculatorMessage::LengthChanged(" ".into()));

    assert_eq!(panel_count(&app, marker), 2);
}
