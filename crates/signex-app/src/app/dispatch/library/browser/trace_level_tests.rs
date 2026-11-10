//! The open-library trace is a developer log line, not a Messages row
//! (round 9 of #2262): at the default Info level it never reaches the ring
//! the Messages panel reads.

use crate::app::Signex;
use crate::diagnostics::logged_count;

#[test]
fn opening_a_library_does_not_put_its_trace_in_messages() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let needle = "open_library_browser: enter";
    let before = logged_count(needle);
    let _task = app.handle_open_library_browser(dir.path().join("trace-level.snxlib"));
    assert_eq!(logged_count(needle), before);
}
