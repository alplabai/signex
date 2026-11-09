//! #1924 PR B: the app does not open a project a live headless `signex-cli`
//! run holds, and changes nothing when it refuses. A project another app
//! holds keeps PR A's behaviour (it opens; the registry write logs). Unix
//! only: process 1 is the always-live holder.

use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use super::super::files;
use super::super::registry::{Kind, Owner, sync_as};
use super::super::transport::endpoint_name;
use crate::app::Signex;

const HOLDER_BOOT: &str = "cccccccccccccccc";
/// Above every system's process id limit.
const DEAD_PID: u32 = 2_000_000_000;

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
    /// The registry entry of the project, as `kind` process `pid` writes it.
    fn held_by(&self, kind: Kind, pid: u32) {
        let endpoint = match kind {
            Kind::Gui => endpoint_name(&self.dir, HOLDER_BOOT),
            Kind::Headless => String::new(),
        };
        let owner = Owner {
            pid,
            boot_id: HOLDER_BOOT.into(),
            endpoint,
        };
        sync_as(kind, &self.dir, &owner, std::slice::from_ref(&self.project)).expect("an entry");
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
    fx.held_by(Kind::Headless, 1);
    let before = fx.app.document_state.projects.len();
    // Starts the log, which keeps only what is logged after it.
    crate::diagnostics::logged_count("");

    fx.open();

    assert!(!fx.opened(), "the project was opened");
    assert_eq!(fx.app.document_state.projects.len(), before);
    assert!(fx.app.document_state.active_project.is_none());
    assert!(fx.app.document_state.tabs.is_empty());
    let notice = fx.app.document_state.error_notice.take().expect("a notice");
    assert!(
        notice
            .detail
            .contains("is being edited by a headless signex-cli run (process 1)"),
        "{}",
        notice.detail
    );
    assert!(
        crate::diagnostics::logged_count(
            "HeldByRun1924.snxprj is being edited by a headless signex-cli run (process 1)"
        ) > 0,
        "the refusal is in Messages"
    );
}

#[test]
fn the_project_opens_again_once_the_headless_run_is_gone() {
    let mut fx = fixture("ReleasedRun1924.snxprj");
    fx.held_by(Kind::Headless, 1);
    fx.open();
    assert!(!fx.opened());

    // The run ends (its entry is removed), or it died (its pid is gone).
    fx.held_by(Kind::Headless, DEAD_PID);
    fx.open();
    assert!(fx.opened(), "a dead run's entry still holds the project");
}

#[test]
fn a_project_held_by_another_app_still_opens() {
    let mut fx = fixture("HeldByApp1924.snxprj");
    let _listening = is_listening(&fx.dir);
    fx.held_by(Kind::Gui, 1);
    fx.open();
    assert!(
        fx.opened(),
        "PR A's behaviour changed: another app holds it"
    );
    assert!(fx.app.document_state.error_notice.is_none());
}

#[test]
fn a_headless_app_never_refuses_it_is_the_run_that_holds_the_project() {
    // A headless run boots its own app on the project it holds, and a second
    // run beside another one goes on as it always could.
    let mut fx = fixture("HeadlessApp1924.snxprj");
    fx.app.ui_state.io = crate::io_ports::IoPorts::headless(Default::default());
    fx.held_by(Kind::Headless, 1);
    fx.open();
    assert!(fx.opened());
}
