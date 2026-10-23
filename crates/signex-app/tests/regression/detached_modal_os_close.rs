//! #622 — a detached modal keeps its native title bar, so the OS close
//! button must run the same close path as the dialog's own close message.

use signex_app::app::{AnnotateMsg, ErcMsg, Message, Signex, WindowMsg};

#[test]
fn os_close_of_a_detached_erc_dialog_runs_the_erc_close_path() {
    let (mut app, _t) = Signex::new();

    let _ = app.update(Message::Erc(ErcMsg::OpenDialog));
    let window_id = *app
        .ui_state
        .windows
        .keys()
        .next()
        .expect("opening the ERC dialog must register a detached window");
    assert!(app.ui_state.erc_dialog_open);

    let task = app.update(Message::Window(WindowMsg::WindowCloseRequested(window_id)));

    assert!(
        !app.ui_state.erc_dialog_open,
        "the OS close button must clear the dialog state like the in-body close"
    );
    assert!(
        !app.ui_state.windows.contains_key(&window_id),
        "the detached window entry must be dropped"
    );
    assert_eq!(task.units(), 1, "the window must be closed exactly once");
}

#[test]
fn os_close_of_a_detached_annotate_dialog_runs_the_annotate_close_path() {
    let (mut app, _t) = Signex::new();

    let _ = app.update(Message::Annotate(AnnotateMsg::OpenDialog));
    let window_id = *app
        .ui_state
        .windows
        .keys()
        .next()
        .expect("opening the Annotate dialog must register a detached window");
    assert!(app.ui_state.annotate_dialog_open);

    let task = app.update(Message::Window(WindowMsg::WindowCloseRequested(window_id)));

    assert!(
        !app.ui_state.annotate_dialog_open,
        "the OS close button must clear the dialog state like the in-body close"
    );
    assert!(
        !app.ui_state.windows.contains_key(&window_id),
        "the detached window entry must be dropped"
    );
    assert_eq!(task.units(), 1, "the window must be closed exactly once");
}
