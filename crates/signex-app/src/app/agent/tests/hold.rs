//! #1924 PR B: the app does not open a project a live headless `signex-cli`
//! run holds, and changes nothing when it refuses. A project another app
//! holds keeps PR A's behaviour (it opens; the registry write logs). Unix
//! only: process 1 stands for a reused process id.

use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use super::super::client::{Hold, hold};
use super::super::files;
use super::super::registry::{Owner, entry_file_name, sync};
use super::super::transport::endpoint_name;
use crate::app::Signex;

const HOLDER_BOOT: &str = "cccccccccccccccc";

struct Fixture {
    _root: tempfile::TempDir,
    dir: PathBuf,
    project: PathBuf,
    app: Signex,
}

/// An app whose agent runtime folder is a scratch one, and a project file
/// named `name` (unique, so its Messages line can be told from other tests').
fn fixture(name: &str) -> Fixture {
    let root = tempfile::Builder::new()
        .prefix("snx-1924h")
        .tempdir()
        .expect("temp dir");
    let dir = root.path().join("run");
    files::prepare_dir(&dir).expect("runtime dir");
    let project = root.path().join(name);
    std::fs::write(&project, b"").expect("project file");
    let (mut app, _boot) = Signex::new();
    app.agent.runtime_dir = Ok(dir.clone());
    Fixture {
        _root: root,
        dir,
        project,
        app,
    }
}

impl Fixture {
    /// A live headless run: its entry and its lock, as `signex-cli` makes them.
    fn held_by_a_run(&self) -> Hold {
        hold(&self.dir, &self.project).expect("a hold")
    }

    /// The entry of an app (process 1, listening) for the project.
    fn held_by_an_app(&self) {
        let owner = Owner {
            pid: 1,
            boot_id: HOLDER_BOOT.into(),
            endpoint: endpoint_name(&self.dir, HOLDER_BOOT),
        };
        sync(&self.dir, &owner, std::slice::from_ref(&self.project)).expect("an entry");
    }

    fn open(&mut self) {
        let _task = self
            .app
            .handle_document_file_opened(Some(self.project.clone()));
    }

    fn opened(&self) -> bool {
        self.app
            .document_state
            .projects
            .iter()
            .any(|project| project.path == self.project)
    }
}

fn is_listening(dir: &Path) -> UnixListener {
    UnixListener::bind(endpoint_name(dir, HOLDER_BOOT)).expect("a listening app")
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

    // The run ends: its entry is removed.
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

#[test]
fn a_project_held_by_another_app_still_opens() {
    let mut fx = fixture("HeldByApp1924.snxprj");
    let _listening = is_listening(&fx.dir);
    fx.held_by_an_app();
    fx.open();
    assert!(
        fx.opened(),
        "PR A's behaviour changed: another app holds it"
    );
    assert!(fx.app.document_state.error_notice.is_none());
}

#[test]
fn a_headless_app_never_refuses_it_is_the_run_that_holds_the_project() {
    // A headless run boots its own app on the project it holds.
    let mut fx = fixture("HeadlessApp1924.snxprj");
    fx.app.ui_state.io = crate::io_ports::IoPorts::headless(Default::default());
    let _run = fx.held_by_a_run();
    fx.open();
    assert!(fx.opened());
}
