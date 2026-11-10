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

fn settle(app: &mut Signex) {
    let mut driver = TaskDriver::new();
    let task = app.update(Message::noop());
    driver.settle(app, task).expect("the update ran");
}

/// A run that took the lock while the person's last save was still being
/// written would read the old file and write over that save.
#[test]
fn a_closed_projects_lock_is_kept_until_its_save_is_reported_done() {
    let mut fx = fixture("SaveThenClose1924.snxprj");
    fx.open();
    assert!(fx.opened());
    let _job = fx
        .app
        .document_state
        .document_saves
        .begin(fx.project.clone(), Vec::new());

    // Close Project while the write is out.
    let _kept = std::mem::take(&mut fx.app.document_state.projects);
    settle(&mut fx.app);
    assert!(fx.app.document_state.document_saves.is_saving(&fx.project));
    assert!(
        client::hold(&fx.dir, &fx.project).is_err(),
        "a run took a project whose save is still being written"
    );

    // The save is reported done: the lock goes with that update.
    fx.app.document_state.document_saves.finish(&fx.project);
    settle(&mut fx.app);
    assert!(
        client::hold(&fx.dir, &fx.project).is_ok(),
        "the lock outlived the save"
    );
}

/// Quit: the locks go after the document, autosave and library writes, with
/// the cleanup task, not when the quit starts.
#[test]
fn the_quit_keeps_the_locks_until_its_writes_are_done() {
    let mut fx = fixture("SaveThenQuit1924.snxprj");
    fx.open();
    assert!(fx.opened());
    let _job = fx
        .app
        .document_state
        .document_saves
        .begin(fx.project.clone(), Vec::new());

    let task = fx.app.flush_quit_writes_then(iced::Task::none());
    assert!(fx.app.document_state.document_saves.is_saving(&fx.project));
    assert!(
        client::hold(&fx.dir, &fx.project).is_err(),
        "a run took a project at the start of the quit, before its save landed"
    );
    // Once the quit's task is done with, the lock is let go.
    drop(task);
    assert!(client::hold(&fx.dir, &fx.project).is_ok());
}

fn locked_warnings(name: &str) -> usize {
    crate::diagnostics::logged_count(&format!("{name} could not be locked"))
}

/// A lock that cannot be made (here the runtime folder sits under a file) is
/// said once and tried again at the registry retry tick, not at every update.
#[test]
fn a_lock_that_cannot_be_made_is_said_once_and_retried_on_the_tick() {
    let root = tempfile::Builder::new()
        .prefix("snx-1924h")
        .tempdir()
        .expect("temp dir");
    let blocker = root.path().join("notadir");
    std::fs::write(&blocker, b"").unwrap();
    let dir = blocker.join("run");
    let name = "NoLock1924.snxprj";
    let project = root.path().join(name);
    std::fs::write(&project, b"").unwrap();
    let mut app = window_on(&dir);
    crate::diagnostics::logged_count("");

    let _task = app.handle_document_file_opened(Some(project.clone()));
    assert!(opened_in(&app, &project), "the project opens anyway");
    for _ in 0..5 {
        settle(&mut app);
    }
    assert_eq!(locked_warnings(name), 1, "said at every update");
    assert!(app.agent.project_locks.is_empty());
    // A tick that fails again does not say it again.
    let _task = app.update(Message::Agent(super::super::AgentMsg::RegistryRetryTick));
    assert_eq!(locked_warnings(name), 1, "said again at the tick");

    // The folder is usable now; nothing is tried before the tick.
    std::fs::remove_file(&blocker).unwrap();
    files::prepare_dir(&blocker).expect("the folder");
    files::prepare_dir(&dir).expect("the runtime folder");
    for _ in 0..3 {
        settle(&mut app);
    }
    assert!(
        app.agent.project_locks.is_empty(),
        "a failed lock was tried again before the tick"
    );
    let _task = app.update(Message::Agent(super::super::AgentMsg::RegistryRetryTick));
    assert!(
        app.agent.project_locks.contains_key(&project),
        "the tick did not retry the lock"
    );
    assert_eq!(locked_warnings(name), 1);
}

/// A run took the project after the window's claim had failed: not dropped
/// silently, said once in Messages with the run's process, and not tried at
/// every update.
#[test]
fn a_run_that_took_an_unlocked_open_project_is_said_once() {
    let mut fx = fixture("RunTook1924.snxprj");
    fx.app.agent.lists_projects = false;
    let _run = fx.held_by_a_run();
    fx.open();
    assert!(fx.opened());
    fx.app.agent.lists_projects = true;
    crate::diagnostics::logged_count("");

    for _ in 0..4 {
        settle(&mut fx.app);
    }
    let pid = std::process::id();
    let said = format!(
        "a signex-cli run (pid {pid}) now holds {}",
        fx.project.display()
    );
    assert_eq!(
        crate::diagnostics::logged_count(&said),
        1,
        "not said exactly once"
    );
    assert!(fx.app.agent.project_locks.is_empty());
}
