//! #1932: a write that fails while a headless session ends is reported, even
//! though the session exits before `update` sees the failure. Its own binary:
//! it corrupts the per-process prefs file.

use signex_app::app::{Message, Signex};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::task_driver::TaskDriver;

#[test]
fn a_dock_layout_write_failing_at_session_end_is_reported() {
    signex_app::diagnostics::init_logging().expect("logger");
    let (mut app, _boot) = Signex::new();
    let prefs = signex_app::fonts::prefs_file_path();
    std::fs::create_dir_all(prefs.parent().expect("prefs folder")).expect("prefs folder");
    std::fs::write(&prefs, b"{ not json").expect("corrupt prefs");
    let _debounce_not_run = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Bottom,
    )));
    let before = signex_app::diagnostics::newest_entry_id();

    let end = app.end_headless_session();
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, end).expect("session ended");

    assert!(driver.has_exited());
    let reported = signex_app::diagnostics::entries_since(before)
        .entries
        .iter()
        .any(|e| e.message.contains("Dock layout was not saved"));
    assert!(reported, "the failed write is reported");
}
