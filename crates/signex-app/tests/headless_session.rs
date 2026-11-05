//! #1932: a headless session reports what happened, keeps stdout clean and
//! ends with the quit path's flush.

use iced::Task;
use signex_app::app::{Message, Signex, UiMsg};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::task_driver::TaskDriver;

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

/// A layout change whose debounce never fired and a toggle still being
/// written are both on disk once the session has ended.
#[test]
fn a_pref_change_followed_by_session_end_is_on_disk() {
    let (mut app, _boot) = Signex::new();
    let _debounce_not_run = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Bottom,
    )));
    let grid_before = app.ui_state.grid_visible;
    let toggle = app.update(Message::Ui(UiMsg::GridToggle));
    let end = app.end_headless_session();
    let mut driver = TaskDriver::new();
    driver
        .settle(&mut app, Task::batch([toggle, end]))
        .expect("session ended");
    assert!(driver.has_exited());

    let prefs = signex_app::fonts::load_prefs_json();
    let dock = signex_app::fonts::read_dock_layout_from(prefs.as_ref()).expect("layout saved");
    assert!(dock.is_collapsed(PanelPosition::Bottom), "layout flushed");
    assert_eq!(
        signex_app::fonts::read_grid_visible_pref_from(prefs.as_ref()),
        !grid_before,
        "toggle written"
    );
}
