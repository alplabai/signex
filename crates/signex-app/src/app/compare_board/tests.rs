//! #2462 — the Compare Board with Schematic dialog's own state: only the
//! newest job lands, a read error reaches Messages and the dialog, a
//! project with nothing to compare refuses with a notice, Close and Esc.
//! The flows over a project on disk are in `tests/regression/compare_board.rs`.

use std::path::PathBuf;

use super::{CompareBoardMsg, CompareBoardState};
use crate::app::update_pcb::Phase;
use crate::app::{Message, Signex};

fn app_with_state(job: u64) -> Signex {
    let (mut app, _t) = Signex::new();
    let project = app.document_state.mint_project_id();
    let mut state = CompareBoardState::new(
        project,
        "p.snxsch".to_string(),
        PathBuf::from("/w/p.snxpcb"),
    );
    state.job = job;
    state.phase = Phase::Computing;
    app.ui_state.compare_board = Some(Box::new(state));
    app
}

fn read_failed(job: u64, why: &str) -> Message {
    Message::from(CompareBoardMsg::Read {
        job,
        result: Err(why.to_string()),
    })
}

#[test]
fn a_read_error_reaches_messages_and_the_dialog() {
    let needle = "compare-board-test: the root sheet could not be read";
    let before = crate::diagnostics::logged_count(needle);
    let mut app = app_with_state(2);
    let _ = app.update(read_failed(1, "an older job"));
    let state = app.ui_state.compare_board.as_ref().expect("still open");
    assert!(
        matches!(state.phase, Phase::Computing),
        "an older job is dropped"
    );
    let _ = app.update(read_failed(2, needle));
    let state = app.ui_state.compare_board.as_ref().expect("still open");
    assert_eq!(state.failure(), Some(needle));
    assert!(state.report().is_none());
    assert_eq!(
        crate::diagnostics::logged_count(needle),
        before + 1,
        "Messages says why"
    );
}

#[test]
fn a_command_with_no_project_in_view_refuses_with_a_notice() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::from(CompareBoardMsg::Requested));
    assert!(app.ui_state.compare_board.is_none(), "no dialog");
    assert!(
        app.document_state.error_notice.is_some(),
        "the notice card says why"
    );
}

#[test]
fn close_drops_the_dialog() {
    let mut app = app_with_state(1);
    let _ = app.update(Message::from(CompareBoardMsg::CloseRequested));
    assert!(app.ui_state.compare_board.is_none());
}

#[test]
fn apply_and_export_wait_for_a_report() {
    let mut app = app_with_state(1);
    let _ = app.update(Message::from(CompareBoardMsg::ApplyRequested));
    let _ = app.update(Message::from(CompareBoardMsg::ExportRequested));
    let state = app.ui_state.compare_board.as_ref().expect("still open");
    assert!(!state.is_confirming());
    assert!(state.error().is_none());
}

#[test]
fn cancel_closes_only_the_confirmation() {
    let mut app = app_with_state(1);
    if let Some(state) = app.ui_state.compare_board.as_mut() {
        state.confirming = true;
    }
    let _ = app.update(Message::from(CompareBoardMsg::ApplyCancelled));
    let state = app
        .ui_state
        .compare_board
        .as_ref()
        .expect("the report stays");
    assert!(!state.is_confirming());
}

#[test]
fn a_kind_chip_filters_and_clears_the_shown_rows_choice() {
    let mut app = app_with_state(1);
    if let Some(state) = app.ui_state.compare_board.as_mut() {
        state.shown_choice = Some(signex_engine::pcb::eco::Direction::Skip);
    }
    let kind = signex_engine::pcb::eco::CompareKind::NetDifference;
    let _ = app.update(Message::from(CompareBoardMsg::KindFilterPicked(Some(kind))));
    let state = app.ui_state.compare_board.as_ref().unwrap();
    assert_eq!(state.filter(), Some(kind));
    assert_eq!(state.shown_choice, None);
}
