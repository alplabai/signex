//! Opening a sheet off the update loop (#859, #1756): the helpers the other
//! tests open sheets with, and what a sheet read in flight may and may not do.

use std::path::{Path, PathBuf};

use signex_types::format::SnxSchematic;
use signex_types::schematic::InstancePath;

use super::sheet_open::{AfterOpen, SheetOpen};
use crate::app::handlers::menu::export::tests::{open_with, sheet_with_net};
use crate::app::path_ops::settle_task;
use crate::app::*;

/// Run the sheet read `task` and hand its result to the open's completion
/// handler, as the runtime would, but without `update`'s own bookkeeping and
/// without running the tasks that returns (Navigator rebuilds, index builds,
/// recovery checks): the open's own effect, as the synchronous open had it.
/// A test that wants the rest runs the task through `settle_task`.
pub(crate) fn finish_read(app: &mut Signex, task: iced::Task<Message>) {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut messages = Vec::new();
    runtime.block_on(async {
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(next) = action {
                messages.push(next);
            }
        }
    });
    for message in messages {
        match message {
            Message::File(FileMsg::DocumentReadFinished { request, result }) => {
                let _ = app.handle_document_read_finished(request, result);
            }
            other => {
                let _ = app.update(other);
            }
        }
    }
}

impl Signex {
    /// Open `path` and finish the read. `true` when the sheet is the focused
    /// one afterwards.
    pub(crate) fn open_or_focus_sheet_path(&mut self, path: PathBuf) -> bool {
        let task = self.open_sheet(path.clone(), SheetOpen::plain());
        finish_read(self, task);
        let held = self.resolve_held_spelling(path);
        self.document_state.active_path.as_deref() == Some(held.as_path())
    }

    /// [`Self::open_sheet_with_placement`], read to the end.
    pub(crate) fn open_placed_settled(&mut self, path: PathBuf, placement: Option<InstancePath>) {
        let task = self.open_sheet_with_placement(path, placement);
        finish_read(self, task);
    }

    /// [`Self::ascend_to_parent_sheet`], read to the end.
    pub(crate) fn ascend_settled(&mut self) {
        let task = self.ascend_to_parent_sheet();
        finish_read(self, task);
    }
}

fn write_sheet(dir: &Path, name: &str, label: &str) -> PathBuf {
    let path = dir.join(name);
    let text = SnxSchematic::new(sheet_with_net("R1", label, &[]))
        .write_string()
        .expect("serialize");
    std::fs::write(&path, text).expect("write sheet");
    path
}

fn has_tab(app: &Signex, path: &Path) -> bool {
    app.document_state.tabs.iter().any(|t| t.path == path)
}

#[test]
fn an_unopened_sheet_is_read_on_a_worker_not_in_update() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_sheet(dir.path(), "child.snxsch", "NET_A");
    let (mut app, _task) = Signex::new();

    let task = app.open_sheet(path.clone(), SheetOpen::plain());

    assert!(!has_tab(&app, &path), "update opened no tab");
    assert!(app.ui_state.pending_descents.contains_key(&path));
    settle_task(&mut app, task);
    assert!(has_tab(&app, &path));
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(path.as_path())
    );
    assert!(app.ui_state.pending_descents.is_empty());
}

#[test]
fn a_sheet_read_in_flight_does_not_open_after_every_document_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_sheet(dir.path(), "child.snxsch", "NET_A");
    let (mut app, _task) = Signex::new();

    let task = app.open_sheet(path.clone(), SheetOpen::plain().then(AfterOpen::Nothing));
    app.cancel_in_flight_reads();
    settle_task(&mut app, task);

    assert!(!has_tab(&app, &path), "the stale read installed nothing");
    assert!(app.ui_state.pending_descents.is_empty());
}

#[test]
fn a_missing_sheet_opens_nothing_and_leaves_no_pending_open() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("gone.snxsch");
    let (mut app, _task) = Signex::new();
    let since = crate::diagnostics::newest_entry_id();

    let task = app.open_sheet(path.clone(), SheetOpen::plain());
    settle_task(&mut app, task);

    assert!(!has_tab(&app, &path));
    assert!(app.ui_state.pending_descents.is_empty());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| Some(e.id) > since && e.message.contains("Child-sheet file not found")),
        "said in Messages"
    );
}

/// The jump is made when the sheet is showing, on the sheet's own canvas.
#[test]
fn a_jump_asked_with_the_open_lands_after_the_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_sheet(dir.path(), "child.snxsch", "NET_A");
    let (mut app, _task) = Signex::new();
    let then = AfterOpen::FocusAt {
        x: 12.0,
        y: 34.0,
        item: None,
    };

    let task = app.open_sheet(path.clone(), SheetOpen::plain().then(then));
    assert!(app.interaction_state.canvas.pending_fit.get().is_none());
    settle_task(&mut app, task);

    let fit = app
        .interaction_state
        .canvas
        .pending_fit
        .get()
        .expect("staged after the read");
    assert!((fit.x - 4.0).abs() < 1e-3 && (fit.y - 26.0).abs() < 1e-3);
}

/// Back / forward: the sheet is read, and arriving must not make a visit that
/// drops the forward list.
#[test]
fn a_history_step_to_an_unopened_sheet_keeps_the_forward_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = write_sheet(dir.path(), "a.snxsch", "NET_A");
    let b = write_sheet(dir.path(), "b.snxsch", "NET_B");
    let (mut app, _task) = Signex::new();
    open_with(
        &mut app.document_state,
        &b,
        sheet_with_net("R2", "NET_B", &[]),
    );
    app.open_or_focus_sheet_path(b.clone());
    app.document_state.net_nav.history = Default::default();
    let history = &mut app.document_state.net_nav.history;
    history.visit(&a);
    history.visit(&b);
    assert_eq!(history.peek(false), Some(a.as_path()));

    let task = app.step_sheet_history(false);
    assert!(!has_tab(&app, &a), "update did not read it");
    settle_task(&mut app, task);

    assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
    let history = &app.document_state.net_nav.history;
    assert_eq!(history.current(), Some(a.as_path()));
    assert_eq!(history.peek(true), Some(b.as_path()), "forward kept");
    assert_eq!(history.peek(false), None);
}

#[test]
fn a_history_step_to_a_sheet_that_cannot_be_read_drops_the_entry() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let gone = dir.path().join("gone.snxsch");
    let b = write_sheet(dir.path(), "b.snxsch", "NET_B");
    let (mut app, _task) = Signex::new();
    assert!(app.open_or_focus_sheet_path(b.clone()));
    app.document_state.net_nav.history = Default::default();
    let history = &mut app.document_state.net_nav.history;
    history.visit(&gone);
    history.visit(&b);
    let since = crate::diagnostics::newest_entry_id();

    let task = app.step_sheet_history(false);
    settle_task(&mut app, task);

    assert_eq!(app.document_state.net_nav.history.peek(false), None);
    assert_eq!(app.document_state.active_path.as_deref(), Some(b.as_path()));
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| Some(e.id) > since && e.message.contains("history skipped")),
        "said in Messages"
    );
}
