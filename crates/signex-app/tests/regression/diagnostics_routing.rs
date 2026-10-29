//! Records emitted while handling a message have to be on screen in the
//! Messages panel by the time that message finishes.
//!
//! The panel does not read the diagnostics ring buffer directly — it
//! renders `document_state.panel_ctx.diagnostics`, which is a snapshot
//! republished by `sync_diagnostics_panel_ctx`. `finish_update` calls
//! that, but not every dispatcher calls `finish_update`:
//! `dispatch_preferences_message` deliberately does not, because
//! reloading the History panel and draining git commits on every
//! keystroke in a modal is not what that dispatcher is for. Before this
//! was wired, a Preferences failure sat in the ring buffer until some
//! unrelated later message happened to republish it — so the log window
//! showed the failure attached to the wrong action, or not at all.
//!
//! These drive the real dispatcher through `Signex::update`, so they fail
//! if the republish call is removed.

use signex_app::app::Signex;
use signex_app::app::contracts::{Message, PreferencesMsg};
use signex_app::diagnostics;

/// Install the process logger so `tracing::*` records actually reach the
/// ring buffer. `log::set_boxed_logger` accepts exactly one call per
/// process, so a second one from another test in this binary is the
/// already-installed error and is nothing to act on.
fn ensure_logger() {
    let _ = diagnostics::init_logging();
}

/// The record is emitted at `Error`, which every level except `off`
/// passes, so this does not depend on the developer's `RUST_LOG`.
#[test]
fn a_record_emitted_during_a_preferences_message_reaches_the_panel_snapshot() {
    // Arrange
    ensure_logger();
    let (mut app, _boot) = Signex::new();
    let marker = "diagnostics_routing marker 4f21ac";
    assert!(
        !app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(marker)),
        "the marker must not already be in the panel snapshot"
    );
    diagnostics::log_error("diagnostics_routing", &anyhow::anyhow!("{marker}"));

    // Act — any Preferences message; the point is the dispatcher, not
    // which arm ran.
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    // Assert
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(marker)),
        "a record emitted before a Preferences message must be in the panel snapshot \
         once that message has been handled — `dispatch_preferences_message` is not \
         republishing the diagnostics buffer"
    );
}

/// The snapshot has to be the *current* buffer, not a stale copy taken
/// once at boot. Pins that the republish happens per message.
#[test]
fn the_panel_snapshot_is_refreshed_on_every_preferences_message() {
    // Arrange
    ensure_logger();
    let (mut app, _boot) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let marker = "diagnostics_routing second marker 9b03de";
    assert!(
        !app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(marker)),
        "the marker must not already be in the panel snapshot"
    );
    diagnostics::log_error("diagnostics_routing", &anyhow::anyhow!("{marker}"));

    // Act
    let _ = app.update(Message::Preferences(PreferencesMsg::Close));

    // Assert
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(marker)),
        "the panel snapshot must be rebuilt on each Preferences message, not captured once"
    );
}
