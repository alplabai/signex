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

/// The messages `task` emits, undelivered.
fn outputs(task: iced::Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = futures::StreamExt::next(&mut stream).await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Fails every write: the disk-full case.
struct FailingSink(&'static str);

impl DocumentSink for FailingSink {
    fn write(&self, _path: &Path, _bytes: &[u8]) -> io::Result<()> {
        Err(io::Error::other(self.0))
    }
}

#[test]
fn a_second_save_while_one_runs_waits_and_writes_the_newer_bytes_after_it() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "a");
    let first = f.app.update(Message::File(FileMsg::Save));
    edit_description(&mut f.app, "ab");
    // One writer per file: this queues a follow-up instead of a second job.
    let second = f.app.update(Message::File(FileMsg::Save));
    settle_task(&mut f.app, second);
    assert!(
        f.app.document_state.document_saves.is_saving(&f.prj),
        "the first write is still the only one running"
    );

    // The first completion finds memory ahead of its bytes: still dirty, and
    // the queued follow-up (run by `settle_task`) writes the newer ones.
    settle_task(&mut f.app, first);

    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("ab"));
    assert!(!is_dirty(&f), "the follow-up matches memory");
}

#[test]
fn an_older_write_finishing_cannot_clear_the_marker_of_edits_made_after_it_began() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "a");
    let first = f.app.update(Message::File(FileMsg::Save));
    edit_description(&mut f.app, "ab");

    // Completion delivered without the follow-up, to look at the state alone.
    for message in outputs(first) {
        drop(f.app.update(message));
    }

    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("a"));
    assert!(is_dirty(&f), "disk holds \"a\" but memory is \"ab\"");
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
fn a_failed_write_is_recorded_and_the_queued_save_that_follows_clears_it() {
    let mut f = fixture();
    open_tab(&mut f.app);
    f.app.document_state.document_saves =
        DocumentSaves::new(Arc::new(FailingWhenContains("doomed")));
    edit_description(&mut f.app, "doomed");
    let older = f.app.update(Message::File(FileMsg::Save));
    edit_description(&mut f.app, "kept");
    let queued = f.app.update(Message::File(FileMsg::Save));
    settle_task(&mut f.app, queued);

    // The first write fails; the follow-up carries the newer bytes.
    settle_task(&mut f.app, older);

    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("kept"));
    assert!(!is_dirty(&f), "the follow-up decides");
    assert!(
        f.app
            .document_state
            .document_saves
            .failure(&f.prj)
            .is_none(),
        "a later success forgets the failure"
    );
    // Kept on purpose: the first write really failed, so its Save Failed
    // card stays until the user dismisses it.
    assert!(
        f.app.document_state.error_notice.is_some(),
        "the card of the failed write is still shown"
    );
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
