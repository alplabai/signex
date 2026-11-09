//! #1924: an open project is listed in the registry whether or not agent
//! connections are on. With them off the entry has no socket and no token (the
//! app listens to nobody) and its liveness is the project's lock, so a
//! headless run never works on a file open in a window. Unix only: it binds a
//! socket for the "on" case.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use super::super::client::{self, Lookup};
use super::super::files::{self, REGISTRY_DIR};
use super::super::registry::{Entry, FORMAT, Kind};
use super::super::transport::endpoint_name;
use crate::app::{Message, Signex};
use crate::task_driver::TaskDriver;

struct Fixture {
    _root: tempfile::TempDir,
    dir: PathBuf,
    project: PathBuf,
    app: Signex,
    driver: TaskDriver,
}

/// An app with agent connections off (the default), a scratch runtime folder
/// and an open project.
fn fixture() -> Fixture {
    let root = tempfile::Builder::new()
        .prefix("snx-1924p")
        .tempdir()
        .expect("temp dir");
    let dir = root.path().join("run");
    files::prepare_dir(&dir).expect("runtime dir");
    let project = root.path().join("P.snxprj");
    std::fs::write(&project, b"").expect("project file");
    let (mut app, _boot) = Signex::new();
    app.agent.runtime_dir = Ok(dir.clone());
    app.agent.lists_projects = true;
    assert!(
        !app.ui_state.agent_connections_enabled,
        "the default is off"
    );
    let _task = app.handle_document_file_opened(Some(project.clone()));
    let mut fx = Fixture {
        _root: root,
        dir,
        project,
        app,
        driver: TaskDriver::new(),
    };
    fx.settle();
    fx
}

impl Fixture {
    /// Run an update, and the registry write it asks for.
    fn settle(&mut self) {
        let task = self.app.update(Message::noop());
        self.driver
            .settle(&mut self.app, task)
            .expect("the write ran");
    }

    fn entry(&self) -> Option<Entry> {
        let path = client::entry_path(&self.dir, &self.project).ok()?;
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    fn lookup(&self) -> Lookup {
        client::lookup(&self.dir, &self.project).expect("a lookup")
    }
}

/// Every file in the runtime folder, not the registry's: sockets and tokens.
fn top_level_files(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|item| item.file_name() != REGISTRY_DIR)
        .map(|item| item.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn an_open_project_is_listed_without_a_socket_or_a_token_while_agent_connections_are_off() {
    let fx = fixture();
    let entry = fx.entry().expect("the open project is listed");
    assert_eq!(entry.kind, Kind::Gui);
    assert_eq!(entry.format, FORMAT);
    assert_eq!(entry.pid, std::process::id());
    assert!(
        entry.socket.is_empty(),
        "an app that does not listen names no socket"
    );
    // Nothing listens and no token exists: PR A's guarantee with the setting off.
    assert!(
        top_level_files(&fx.dir).is_empty(),
        "{:?}",
        top_level_files(&fx.dir)
    );
    // A client sees an app that has the project and cannot be attached to.
    assert!(
        matches!(fx.lookup(), Lookup::GuiNotListening { pid } if pid == std::process::id()),
        "{:?}",
        fx.lookup()
    );
}

#[test]
fn a_project_that_closes_loses_its_entry_and_its_lock() {
    let mut fx = fixture();
    assert!(fx.entry().is_some());
    let kept = std::mem::take(&mut fx.app.document_state.projects);
    fx.settle();
    assert!(fx.entry().is_none(), "a closed project is still listed");
    assert!(matches!(fx.lookup(), Lookup::Free), "{:?}", fx.lookup());
    // It opens again: listed again.
    fx.app.document_state.projects = kept;
    fx.settle();
    assert!(fx.entry().is_some());
}

/// A crash leaves the entry, but the kernel drops the lock with the process.
#[test]
fn an_entry_left_by_a_crashed_app_is_stale() {
    let fx = fixture();
    assert!(fx.entry().is_some());
    // The process is gone, its pid is not (this test's process, or a reused one).
    fx.app.agent.registry_lock.lock().unwrap().clear();
    assert!(fx.entry().is_some(), "the entry stays");
    assert!(matches!(fx.lookup(), Lookup::Stale(_)), "{:?}", fx.lookup());
}

/// Turning the setting on makes the entry the attachable one; off, back.
#[test]
fn the_entry_follows_the_setting_attachable_when_listening_and_not_when_it_is_off() {
    let mut fx = fixture();
    // The person turned the setting on (the Save of Preferences).
    fx.app.ui_state.agent_connections_enabled = true;
    let boot_id = fx.app.agent.boot_id().unwrap().to_string();
    let socket = endpoint_name(&fx.dir, &boot_id);
    let _listening = UnixListener::bind(&socket).unwrap();
    let task = fx
        .app
        .update(Message::Agent(super::super::AgentMsg::ListenerStarted {
            boot_id,
            endpoint: socket.clone(),
        }));
    fx.driver.settle(&mut fx.app, task).expect("the write ran");
    assert_eq!(fx.entry().unwrap().socket, socket);
    assert!(matches!(fx.lookup(), Lookup::Gui(_)), "{:?}", fx.lookup());

    // The listener ends (the setting went off): the project stays listed.
    fx.app.agent.listener_closed();
    fx.settle();
    assert!(fx.entry().unwrap().socket.is_empty());
    assert!(
        matches!(fx.lookup(), Lookup::GuiNotListening { .. }),
        "{:?}",
        fx.lookup()
    );
}

/// A second app, or a headless run, cannot hold what an app has open.
#[test]
fn a_headless_run_cannot_hold_a_project_an_app_has_open() {
    let fx = fixture();
    let why = client::hold(&fx.dir, &fx.project).unwrap_err();
    assert!(why.contains("is also open in process"), "{why}");
    // And the entry is still the app's.
    assert_eq!(fx.entry().unwrap().kind, Kind::Gui);
}

/// Only the window the person works in lists its projects: not an app made
/// for a test, nor a headless run, which holds its project itself.
#[test]
fn the_boot_of_the_window_lists_its_projects_and_a_plain_app_does_not() {
    let (plain, _task) = Signex::new();
    assert!(!plain.agent.lists_projects);
    let (window, _task) = Signex::boot(Vec::new());
    assert!(window.agent.lists_projects);
}
