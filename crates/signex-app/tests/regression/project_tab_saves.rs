//! #1202 — overlapping and failing `.snxprj` writes from the Project tab.
//! The newest write begun for the file alone decides whether the project is
//! clean; a failed write keeps it dirty and reaches the Messages panel.

use std::io;
use std::path::Path;
use std::sync::Arc;

use signex_app::app::{FileMsg, Message, ProjectCloseChoice, ProjectMsg};
use signex_app::persist::{AtomicFileSink, DocumentSaves, DocumentSink};

use crate::project_tab_support::{edit_description, fixture, is_dirty, on_disk, open_tab};
use crate::settle_support::{settle, settle_task};

/// Fails every write: the disk-full case.
struct FailingSink(&'static str);

impl DocumentSink for FailingSink {
    fn write(&self, _path: &Path, _bytes: &[u8]) -> io::Result<()> {
        Err(io::Error::other(self.0))
    }
}

/// Ctrl+S twice with an edit between, then the edit reverted so memory equals
/// the FIRST snapshot again. Returns both tasks.
fn two_overlapping_saves(
    f: &mut crate::project_tab_support::Fixture,
) -> (iced::Task<Message>, iced::Task<Message>) {
    open_tab(&mut f.app);
    edit_description(&mut f.app, "a");
    let first = f.app.update(Message::File(FileMsg::Save));
    edit_description(&mut f.app, "ab");
    let second = f.app.update(Message::File(FileMsg::Save));
    edit_description(&mut f.app, "a");
    (first, second)
}

#[test]
fn an_older_write_finishing_first_cannot_clear_the_marker_of_a_newer_one() {
    let mut f = fixture();
    let (first, second) = two_overlapping_saves(&mut f);

    // The first completion arrives while memory equals its bytes again.
    settle_task(&mut f.app, first);
    assert!(
        is_dirty(&f),
        "a newer write is still owed; nothing is clean"
    );

    settle_task(&mut f.app, second);
    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("ab"));
    assert!(
        is_dirty(&f),
        "disk holds \"ab\" but memory is \"a\": the marker must stay"
    );
}

#[test]
fn a_superseded_older_write_leaves_the_newest_to_decide() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "a");
    let first = f.app.update(Message::File(FileMsg::Save));
    edit_description(&mut f.app, "ab");
    let second = f.app.update(Message::File(FileMsg::Save));

    // The newer job reaches the sink first, so the older one is skipped.
    settle_task(&mut f.app, second);
    assert!(!is_dirty(&f), "the newest write matches memory");
    settle_task(&mut f.app, first);

    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("ab"));
    assert!(!is_dirty(&f), "the skipped write changes nothing");
}

#[test]
fn a_failed_write_keeps_the_project_dirty_and_is_reported() {
    let _ = signex_app::diagnostics::init_logging();
    let reason = "project_tab_saves disk-full marker 7b3e";
    let mut f = fixture();
    open_tab(&mut f.app);
    f.app.document_state.document_saves = DocumentSaves::new(Arc::new(FailingSink(reason)));
    edit_description(&mut f.app, "lost");

    let task = f.app.update(Message::File(FileMsg::Save));
    settle_task(&mut f.app, task);

    assert!(is_dirty(&f));
    assert_eq!(on_disk(&f.prj).description, None, "nothing was written");
    assert!(
        f.app
            .document_state
            .document_saves
            .failure(&f.prj)
            .is_some_and(|recorded| recorded.contains(reason)),
        "Save All reads the recorded reason"
    );
    assert!(
        f.app
            .document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(reason)),
        "the failure must be in the Messages panel snapshot"
    );
}

/// Fails only the writes whose bytes contain `marker`; the rest land.
struct FailingWhenContains(&'static str);

impl DocumentSink for FailingWhenContains {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if String::from_utf8_lossy(bytes).contains(self.0) {
            return Err(io::Error::other("older write failed"));
        }
        AtomicFileSink.write(path, bytes)
    }
}

#[test]
fn a_late_failure_of_an_older_write_raises_no_save_failure() {
    let mut f = fixture();
    open_tab(&mut f.app);
    f.app.document_state.document_saves =
        DocumentSaves::new(Arc::new(FailingWhenContains("doomed")));
    edit_description(&mut f.app, "doomed");
    let older = f.app.update(Message::File(FileMsg::Save));
    edit_description(&mut f.app, "kept");
    let newer = f.app.update(Message::File(FileMsg::Save));

    // The older write fails after the newer one began.
    settle_task(&mut f.app, older);
    assert!(
        f.app
            .document_state
            .document_saves
            .failure(&f.prj)
            .is_none(),
        "a stale failure must not be recorded for Save All"
    );
    assert!(
        f.app.document_state.error_notice.is_none(),
        "a stale failure must not raise the Save Failed card"
    );

    settle_task(&mut f.app, newer);
    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("kept"));
    assert!(!is_dirty(&f), "the newest write decides");
}

#[test]
fn reopening_a_project_whose_write_is_in_flight_is_refused() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "saved");
    let held = f.app.update(Message::File(FileMsg::Save));

    crate::close_support::close_first_project(&mut f.app);
    let _ = f.app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(f.app.document_state.projects.is_empty());

    // The old bytes are still on disk; loading them now would be clean.
    settle(
        &mut f.app,
        Message::File(FileMsg::Opened(Some(f.prj.clone()))),
    );
    assert!(
        f.app.document_state.projects.is_empty(),
        "the project must not load while its write is in flight"
    );

    settle_task(&mut f.app, held);
    settle(
        &mut f.app,
        Message::File(FileMsg::Opened(Some(f.prj.clone()))),
    );

    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("saved"));
    assert_eq!(f.app.document_state.projects.len(), 1);
    assert_eq!(
        f.app.document_state.projects[0].data.description.as_deref(),
        Some("saved")
    );
    assert!(!is_dirty(&f));
}
