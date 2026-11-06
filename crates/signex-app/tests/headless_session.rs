//! #1932: a headless session reports what happened, keeps stdout clean and
//! ends with the quit path's flush.

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use std::path::PathBuf;

use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::panels::{PanelMsg, TitleBlockField};
use signex_app::task_driver::TaskDriver;
use signex_types::schematic::SchematicSheet;

const PROBE_ENV: &str = "SIGNEX_SESSION_PROBE";
const PROBE_LINE: &str = "zq-session-probe-line";

/// Logging goes to stderr and the ring only: stdout is left to a JSON
/// envelope.
#[test]
fn log_lines_never_reach_stdout() {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, "1")
        .output()
        .expect("run the probe");
    assert!(output.status.success(), "probe failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(PROBE_LINE), "stdout: {stdout}");
    assert!(stderr.contains(PROBE_LINE), "stderr: {stderr}");
}

#[test]
fn probe() {
    if std::env::var_os(PROBE_ENV).is_none() {
        return;
    }
    signex_app::diagnostics::init_logging().expect("logger");
    let before = signex_app::diagnostics::newest_entry_id();
    signex_app::diagnostics::log_warning(PROBE_LINE);
    let delta = signex_app::diagnostics::entries_since(before);
    assert_eq!(delta.missed, 0);
    assert!(delta.entries.iter().any(|e| e.message == PROBE_LINE));
}

/// A layout change whose debounce never fired is on disk once the session
/// has ended. A running write is covered by
/// `prefs_async_tests::ending_a_headless_session_waits_for_a_running_preference_write`.
#[test]
fn a_pref_change_followed_by_session_end_is_on_disk() {
    let (mut app, _boot) = Signex::new();
    let _debounce_not_run = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Bottom,
    )));
    let end = app.end_headless_session();
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, end).expect("session ended");
    assert!(driver.has_exited());

    let prefs = signex_app::fonts::load_prefs_json();
    let dock = signex_app::fonts::read_dock_layout_from(prefs.as_ref()).expect("layout saved");
    assert!(dock.is_collapsed(PanelPosition::Bottom), "layout flushed");
}

/// The session end runs the GUI quit's pre-steps: a title-block draft still
/// being typed is committed, and the edit it makes is reported as unsaved.
#[test]
fn a_pending_title_block_draft_is_committed_and_reported_at_session_end() {
    // Already installed by another test of this binary: fine.
    signex_app::diagnostics::init_logging().ok();
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("zq-headless-draft.snxsch");
    let engine = signex_engine::Engine::new(SchematicSheet::default()).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "draft".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    let _typing = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::TitleBlockTyping(TitleBlockField::DrawnBy, "Headless".into()),
    )));

    let end = app.end_headless_session();
    TaskDriver::new()
        .settle(&mut app, end)
        .expect("session ended");

    let stored = app.document_state.engines[&path]
        .document()
        .title_block
        .get("drawn_by")
        .cloned();
    assert_eq!(
        stored.as_deref(),
        Some("Headless"),
        "the draft was committed"
    );
    let reported = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message
            .contains("unsaved changes in zq-headless-draft.snxsch")
    });
    assert!(reported, "the committed edit is reported as unsaved");
}

/// While a rename or delete runs the session does not end, as the GUI quit
/// is held; the hold is reported.
#[test]
fn a_session_does_not_end_while_a_path_op_runs() {
    // Already installed by another test of this binary: fine.
    signex_app::diagnostics::init_logging().ok();
    let (mut app, _boot) = Signex::new();
    let _held = app.document_state.pending_path_ops.begin(
        signex_app::app::path_ops::PathOpKind::Rename,
        vec![PathBuf::from("/zq-held/sheet.snxsch")],
        signex_app::app::CaseFold::default(),
    );
    let before = signex_app::diagnostics::newest_entry_id();

    let end = app.end_headless_session();
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, end).expect("settled");

    assert!(!driver.has_exited(), "the session ended mid-rename");
    let reported = signex_app::diagnostics::entries_since(before)
        .entries
        .iter()
        .any(|e| e.message.contains("A rename or delete is still running"));
    assert!(reported, "the hold is reported");
}
