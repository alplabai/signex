//! #599 — a git history walk that failed is not "no commits yet".
//!
//! `Message::HistoryLoaded` carries `Result<Vec<HistoryEntry>, String>`
//! and used to be `unwrap_or_default()`-ed straight into a `Ready` mode
//! with an empty entry list — which the History panel renders as the
//! "No history yet." card. A corrupt object or an unreadable `.git/`
//! was therefore presented as "this file has no commits", and "Restore
//! this version" silently disappeared. The panel already keeps `NoRepo`
//! separate from "no commits yet" for the same reason.

use std::path::PathBuf;

use signex_app::app::{Message, Signex};
use signex_app::panels::history::HistoryRenderMode;

const WALK_ERROR: &str = "backend: git revwalk: corrupt object 4f21ac";

/// The completion message the async loader would deliver, tagged with
/// the generation the app is currently waiting on (a mismatched
/// generation is dropped as stale before any of this runs).
fn history_loaded(
    app: &Signex,
    result: Result<Vec<signex_widgets::HistoryEntry>, String>,
) -> Message {
    Message::HistoryLoaded {
        generation: app.document_state.history.generation,
        path: PathBuf::from("/projects/board/main.snxsch"),
        result,
    }
}

#[test]
fn a_failed_history_walk_renders_as_an_error_not_an_empty_list() {
    // Arrange
    let (mut app, _boot) = Signex::new();
    let message = history_loaded(&app, Err(WALK_ERROR.to_string()));

    // Act
    let _ = app.update(message);

    // Assert — the panel gets a distinct mode carrying the diagnosis,
    // not the `Ready` + empty combination that reads as "no commits".
    assert_eq!(
        app.document_state.panel_ctx.history.mode,
        HistoryRenderMode::Error(WALK_ERROR.to_string()),
        "a failed walk must not be reported as a successful empty load"
    );
    assert!(
        app.document_state.panel_ctx.history.entries.is_empty(),
        "a failed walk has no entries to show"
    );
}

#[test]
fn a_successful_history_walk_is_still_ready() {
    // Arrange
    let (mut app, _boot) = Signex::new();
    let message = history_loaded(&app, Ok(Vec::new()));

    // Act
    let _ = app.update(message);

    // Assert — the "fresh repo, no commits yet" path is untouched.
    assert_eq!(
        app.document_state.panel_ctx.history.mode,
        HistoryRenderMode::Ready
    );
}

#[test]
fn a_failed_history_walk_reaches_the_messages_panel() {
    // Arrange
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    let message = history_loaded(&app, Err(WALK_ERROR.to_string()));

    // Act
    let _ = app.update(message);

    // Assert — on screen in the same frame, with the error text. The
    // panel compacts records to 160 characters, so the diagnosis has
    // to survive that budget, not just be emitted.
    let panel = &app.document_state.panel_ctx.diagnostics;
    assert!(
        panel.iter().any(
            |entry| entry.message.contains("git history could not be read")
                && entry.message.contains("corrupt object 4f21ac")
        ),
        "the failure must be in the Messages panel snapshot; got {:?}",
        panel.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}
