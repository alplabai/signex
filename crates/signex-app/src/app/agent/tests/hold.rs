//! #1924 PR B: a project is never open in two copies at once. A window holds a
//! shared lock on the project's lock file from the moment it opens the project
//! until it closes it; a headless `signex-cli` run holds it exclusively. So the
//! window refuses a project a run holds (and changes nothing), a run cannot
//! take a project any window has open, and many windows can have one project
//! open. Unix only: process 1 stands for a reused process id.

use std::path::PathBuf;

use super::super::client::{self, Hold, Lookup, hold};
use super::super::files;
use super::super::registry::entry_file_name;
use crate::app::{Message, Signex};
use crate::task_driver::TaskDriver;

struct Fixture {
    _root: tempfile::TempDir,
    dir: PathBuf,
    project: PathBuf,
    app: Signex,
}

/// A window (it lists its projects, as the boot of the real one does) whose
/// agent runtime folder is a scratch one, and a project file named `name`
/// (unique, so its Messages line can be told from other tests').
fn fixture(name: &str) -> Fixture {
    let root = tempfile::Builder::new()
        .prefix("snx-1924h")
        .tempdir()
        .expect("temp dir");
    let dir = root.path().join("run");
    files::prepare_dir(&dir).expect("runtime dir");
    let project = root.path().join(name);
    std::fs::write(&project, b"").expect("project file");
    let app = window_on(&dir);
    Fixture {
        _root: root,
        dir,
        project,
        app,
    }
}

/// Another window of this person, on the same runtime folder.
fn window_on(dir: &std::path::Path) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.agent.runtime_dir = Ok(dir.to_path_buf());
    app.agent.lists_projects = true;
    app
}

impl Fixture {
    /// A live headless run: its entry and its lock, as `signex-cli` makes them.
    fn held_by_a_run(&self) -> Hold {
        hold(&self.dir, &self.project).expect("a hold")
    }

    fn open(&mut self) {
        let _task = self
            .app
            .handle_document_file_opened(Some(self.project.clone()));
    }

    fn opened(&self) -> bool {
        opened_in(&self.app, &self.project)
    }
}

fn opened_in(app: &Signex, project: &std::path::Path) -> bool {
    app.document_state
        .projects
        .iter()
        .any(|open| open.path == project)
}

#[test]
fn a_project_held_by_a_live_headless_run_is_refused_with_a_message_and_a_notice() {
    let mut fx = fixture("HeldByRun1924.snxprj");
    let _run = fx.held_by_a_run();
    let before = fx.app.document_state.projects.len();
    // Starts the log, which keeps only what is logged after it.
    crate::diagnostics::logged_count("");

    fx.open();

    assert!(!fx.opened(), "the project was opened");
    assert_eq!(fx.app.document_state.projects.len(), before);
    assert!(fx.app.document_state.active_project.is_none());
    assert!(fx.app.document_state.tabs.is_empty());
    assert!(
        fx.app.agent.project_locks.is_empty(),
        "a refused open holds a lock"
    );
    let notice = fx.app.document_state.error_notice.take().expect("a notice");
    let pid = std::process::id();
    let want = format!("is being edited by a headless signex-cli run (process {pid})");
    assert!(notice.detail.contains(&want), "{}", notice.detail);
    // It names the entry to remove if no run is alive.
    let canonical = std::fs::canonicalize(&fx.project).unwrap();
    let entry = fx.dir.join("registry").join(entry_file_name(&canonical));
    assert!(
        notice.detail.contains(&entry.display().to_string()),
        "{}",
        notice.detail
    );
    assert!(
        crate::diagnostics::logged_count(&format!("HeldByRun1924.snxprj {want}")) > 0,
        "the refusal is in Messages"
    );
}

#[test]
fn the_project_opens_again_once_the_run_ends_or_dies() {
    let mut fx = fixture("ReleasedRun1924.snxprj");
    let run = fx.held_by_a_run();
    fx.open();
    assert!(!fx.opened());

    // The run ends: its entry is removed and its lock is released.
    drop(run);
    fx.open();
    assert!(fx.opened(), "an ended run still holds the project");

    // A run killed by a signal leaves its entry, but the kernel dropped its
    // lock; its process id is reused by a living process (1).
    let mut other = fixture("DiedRun1924.snxprj");
    let run = other.held_by_a_run();
    let entry = other.dir.join("registry").join(entry_file_name(
        &std::fs::canonicalize(&other.project).unwrap(),
    ));
    let text = std::fs::read_to_string(&entry).unwrap();
    let reused = text.replace(&std::process::id().to_string(), "1");
    drop(run);
    std::fs::write(&entry, reused).unwrap();
    other.open();
    assert!(other.opened(), "a dead run's entry still holds the project");
}

/// Two windows can have one project open, and a headless run is refused while
/// either does.
#[test]
fn two_windows_open_one_project_and_a_run_is_refused_while_either_has_it() {
    let mut a = fixture("TwoWindows1924.snxprj");
    let mut b_app = window_on(&a.dir);
    a.open();
    let _task = b_app.handle_document_file_opened(Some(a.project.clone()));
    assert!(
        a.opened() && opened_in(&b_app, &a.project),
        "both windows open it"
    );
    assert!(a.app.document_state.error_notice.is_none());
    assert!(b_app.document_state.error_notice.is_none());
    let refuse = |why: &str| {
        let refused = client::hold(&a.dir, &a.project).err();
        assert!(refused.is_some(), "{why}: a run took an open project");
    };
    refuse("two windows");

    // Window A closes it, and settles (its lock goes at the end of the update).
    let _kept = std::mem::take(&mut a.app.document_state.projects);
    let mut driver = TaskDriver::new();
    let task = a.app.update(Message::noop());
    driver.settle(&mut a.app, task).expect("the update ran");
    // No gap: B still has it open, so the run is still refused.
    refuse("one window left");
    assert!(
        matches!(
            client::lookup(&a.dir, &a.project),
            Ok(Lookup::GuiNotListening { .. })
        ),
        "the project looked free while a window had it"
    );

    // B closes it too: now a run may take it.
    let _kept = std::mem::take(&mut b_app.document_state.projects);
    let task = b_app.update(Message::noop());
    driver.settle(&mut b_app, task).expect("the update ran");
    assert!(
        client::hold(&a.dir, &a.project).is_ok(),
        "no window has it open"
    );
}

/// The lock is taken in the update that opens the project, not later on a
/// worker: a run cannot slip in between the open and the registry write.
#[test]
fn a_run_cannot_slip_in_between_the_open_and_the_registry_write() {
    let mut fx = fixture("NoGap1924.snxprj");
    fx.open();
    assert!(fx.opened());
    // No task has run: no entry was written yet.
    let canonical = std::fs::canonicalize(&fx.project).unwrap();
    assert!(
        !fx.dir
            .join("registry")
            .join(entry_file_name(&canonical))
            .exists()
    );
    assert!(
        client::hold(&fx.dir, &fx.project).is_err(),
        "a run took the open project"
    );
}

#[test]
fn an_app_that_does_not_list_never_refuses_it_is_the_run_that_holds_the_project() {
    // A headless run boots its own app on the project it holds; so does an app
    // made for a test.
    let mut fx = fixture("PlainApp1924.snxprj");
    fx.app.agent.lists_projects = false;
    let _run = fx.held_by_a_run();
    fx.open();
    assert!(fx.opened());
}
