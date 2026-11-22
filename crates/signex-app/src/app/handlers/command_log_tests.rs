//! Replay and the current selection (decision 2026-11-19, mockup row 161).

use std::time::Duration;

use serde_json::Map;

use crate::app::Signex;
use crate::panels::command_log::{CommandLogMsg, LogOutcome, LogSource};

/// `cycle_selection_mode` needs no document, so a replay that ran it would
/// show in the mode.
#[test]
fn a_row_with_no_arguments_is_refused_and_runs_nothing() {
    let (mut app, _boot) = Signex::new();
    let mode = app.ui_state.selection_mode;
    let seq = app
        .record_command(
            "cycle_selection_mode",
            Map::new(),
            LogOutcome::Ok(None),
            Duration::ZERO,
            LogSource::Gui { window: None },
            None,
        )
        .expect("logged");

    let _ = app.handle_command_log_message(CommandLogMsg::ReplayRequested(seq));

    assert_eq!(app.ui_state.selection_mode, mode, "the command did not run");
    let log = &app.ui_state.command_log.log;
    assert_eq!(log.rows().count(), 2, "the refusal is a new row");
    let refused = log.rows().next_back().expect("the new row");
    assert_eq!(refused.command, "cycle_selection_mode");
    assert_eq!(refused.source, LogSource::Gui { window: None });
    match &refused.outcome {
        LogOutcome::Failed(reason) => {
            assert!(reason.contains("no recorded arguments"), "{reason}");
        }
        other => panic!("the refused replay was logged as {other:?}"),
    }
}
