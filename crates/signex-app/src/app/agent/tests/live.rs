//! The listener over a real Unix socket, with the test standing in for iced:
//! it takes each `Message` the listener sends and runs it through
//! `Signex::update`, as the runtime does. (Unix only; the Windows pipe shares
//! the framing and the handler and is not run here.)

use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::futures::StreamExt as _;
use iced::futures::channel::mpsc;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::UnixStream;

use super::super::AgentMsg;
use super::super::client::{self, Lookup, Reply};
use super::super::listener::token_path;
use super::super::listener::{ListenerConfig, serve};
use super::super::registry::{self, FORMAT};
use super::super::reply::Status;
use super::super::state::MAX_TOUCHED;
use super::super::wire::MAX_REQUEST_BYTES;
use super::{app_at_work, line, path, revision, snapshot};
use crate::app::{Message, Signex};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a tokio runtime")
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// The app pointed at a runtime directory of its own, and the config the
/// listener would run on.
fn app_and_config(root: &Path) -> (Signex, ListenerConfig, PathBuf) {
    let dir = root.join("run");
    let mut app = app_at_work();
    app.agent.runtime_dir = Ok(dir.clone());
    app.agent.endpoint = None;
    let config = app
        .agent
        .listener_config(true)
        .expect("agent connections are on");
    (app, config, dir)
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("snx-1924-live")
        .tempdir()
        .expect("temp dir")
}

/// Feed the app what the listener sends until `client` has a reply line.
async fn read_reply(
    app: &mut Signex,
    messages: &mut mpsc::Receiver<Message>,
    client: &mut BufReader<UnixStream>,
) -> Value {
    let mut text = String::new();
    {
        let read = client.read_line(&mut text);
        tokio::pin!(read);
        loop {
            tokio::select! {
                message = messages.next() => {
                    let message = message.expect("the listener is running");
                    let _task = app.update(message);
                }
                read = &mut read => {
                    read.expect("the socket is readable");
                    break;
                }
            }
        }
    }
    serde_json::from_str(text.trim_end()).unwrap_or(Value::Null)
}

async fn ask(
    app: &mut Signex,
    messages: &mut mpsc::Receiver<Message>,
    client: &mut BufReader<UnixStream>,
    text: &str,
) -> Value {
    client.get_mut().write_all(text.as_bytes()).await.unwrap();
    client.get_mut().write_all(b"\n").await.unwrap();
    read_reply(app, messages, client).await
}

/// Start the listener and take the app through its `ListenerStarted`.
async fn start(
    app: &mut Signex,
    config: &ListenerConfig,
) -> (
    tokio::task::JoinHandle<()>,
    mpsc::Receiver<Message>,
    PathBuf,
) {
    let (sender, mut messages) = mpsc::channel(16);
    let server = tokio::spawn(serve(config.clone(), sender));
    let first = messages
        .next()
        .await
        .expect("the listener says how it started");
    let Message::Agent(AgentMsg::ListenerStarted { endpoint, .. }) = &first else {
        panic!("the listener did not start: {first:?}");
    };
    let socket = PathBuf::from(endpoint);
    let _task = app.update(first);
    (server, messages, socket)
}

#[test]
fn the_files_are_private_and_a_query_and_an_edit_run_while_the_person_is_mid_interaction() {
    let root = scratch();
    let (mut app, config, dir) = app_and_config(root.path());
    app.refresh_panel_ctx();
    let person = snapshot(&app);
    let doc = path("b.snxsch").display().to_string();
    let rt = runtime();
    rt.block_on(async {
        let (server, mut messages, socket) = start(&mut app, &config).await;
        assert!(
            app.agent.endpoint.is_some(),
            "the app knows it is listening"
        );

        assert_eq!(mode(&dir), 0o700, "the runtime folder");
        assert_eq!(mode(&socket), 0o600, "the socket");
        let token_file = super::super::listener::token_path(&dir, &config.boot_id);
        assert_eq!(mode(&token_file), 0o600, "the token file");
        assert_eq!(
            std::fs::read_to_string(&token_file).unwrap(),
            config.token.as_str()
        );

        let mut client = BufReader::new(UnixStream::connect(&socket).await.unwrap());
        let logged = crate::diagnostics::logged_count(&format!(
            "Agent request (pid {}): `apply_edits` -> answered",
            std::process::id()
        ));
        let started = revision(&app, "b.snxsch");
        let request = line(&app, "read_document", json!({"document": doc}));
        let read = ask(&mut app, &mut messages, &mut client, &request).await;
        assert_eq!(read["status"], "answered", "{read}");
        assert_eq!(read["data"]["revision"], started);

        let edits = json!([{"id": "place_junction", "args": {"at": [20, 20]}}]);
        let request = line(
            &app,
            "apply_edits",
            json!({"document": doc, "expect_revision": started, "edits": edits}),
        );
        let applied = ask(&mut app, &mut messages, &mut client, &request).await;
        assert_eq!(applied["status"], "answered", "{applied}");
        assert_eq!(applied["changed"], json!([doc]));
        assert!(revision(&app, "b.snxsch") > started, "the change landed");
        assert_eq!(snapshot(&app), person, "the person's state is byte equal");

        let mine = format!(
            "Agent request (pid {}): `apply_edits` -> answered",
            std::process::id()
        );
        assert!(
            crate::diagnostics::logged_count(&mine) > logged,
            "the client's own pid is in the log"
        );

        // Two requests in one write are answered in order.
        let both = format!(
            "{}\n{}\n",
            line(&app, "list_documents", json!({})),
            line(&app, "undo", json!({"document": doc}))
        );
        client.get_mut().write_all(both.as_bytes()).await.unwrap();
        let first = read_reply(&mut app, &mut messages, &mut client).await;
        let second = read_reply(&mut app, &mut messages, &mut client).await;
        assert_eq!(first["status"], "answered");
        assert_eq!(second["status"], "refused");

        server.abort();
    });
}

/// What `signex-cli` does when a person has the project open: it finds the
/// app through the registry, reads the token file, and its query and edit run
/// in the app while the person's own state is untouched.
#[test]
fn the_client_attaches_through_the_registry_and_its_query_and_edit_change_the_running_app() {
    let root = scratch();
    let (mut app, config, dir) = app_and_config(root.path());
    app.refresh_panel_ctx();
    let project = app.document_state.projects[0].path.clone();
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(&project, "{}").unwrap();
    let person = snapshot(&app);
    let doc = path("b.snxsch").display().to_string();
    let started = revision(&app, "b.snxsch");
    let rt = runtime();
    rt.block_on(async {
        let (server, mut messages, _socket) = start(&mut app, &config).await;
        // The write the app's own worker makes once the listener is up.
        let owner = app.agent.owner().expect("the app is listening");
        registry::sync(&dir, &owner, std::slice::from_ref(&project)).unwrap();
        // And the window's lock on the open project, taken when it opened.
        let _window = registry::claim_shared(&dir, &std::fs::canonicalize(&project).unwrap())
            .ok()
            .expect("the window's lock");

        let (client_dir, client_project, client_doc) = (dir.clone(), project.clone(), doc.clone());
        let attached = std::thread::spawn(move || -> Result<Vec<Reply>, String> {
            let Lookup::Gui(peer) = client::lookup(&client_dir, &client_project)? else {
                return Err("no running app was found".into());
            };
            let mut connection = peer.connect()?;
            let on = |name: &str, text: String| (name.to_string(), text);
            let edits = json!([{"id": "place_junction", "args": {"at": [20, 20]}}]).to_string();
            Ok(vec![
                connection.ask("read_document", &[on("document", client_doc.clone())])?,
                connection.ask(
                    "apply_edits",
                    &[
                        on("document", client_doc.clone()),
                        on("expect_revision", started.to_string()),
                        on("edits", edits),
                    ],
                )?,
                connection.ask("undo", &[on("document", client_doc)])?,
            ])
        });
        while !attached.is_finished() {
            let next = tokio::time::timeout(Duration::from_millis(20), messages.next()).await;
            if let Ok(Some(message)) = next {
                let _task = app.update(message);
            }
        }
        let replies = attached.join().expect("the client ran").expect("attached");

        assert_eq!(replies[0].status, Status::Answered);
        assert_eq!(replies[0].data.as_ref().unwrap()["revision"], started);
        assert_eq!(replies[1].status, Status::Answered, "{:?}", replies[1]);
        assert_eq!(replies[1].changed, std::slice::from_ref(&doc));
        assert!(revision(&app, "b.snxsch") > started, "the change landed");
        // The agent channel's refusals come back with the app's reason.
        assert_eq!(replies[2].status, Status::Refused);
        assert!(
            replies[2]
                .reason
                .as_deref()
                .is_some_and(|why| why.contains("undo"))
        );
        assert_eq!(snapshot(&app), person, "the person's state is byte equal");
        server.abort();
    });
}

#[test]
fn an_oversized_line_is_refused_and_the_connection_closed_with_the_reply_delivered() {
    let root = scratch();
    let (mut app, config, _dir) = app_and_config(root.path());
    let rt = runtime();
    rt.block_on(async {
        let (server, mut messages, socket) = start(&mut app, &config).await;
        let mut client = BufReader::new(UnixStream::connect(&socket).await.unwrap());
        let before = snapshot(&app);

        let mut big = vec![b'x'; MAX_REQUEST_BYTES + 100];
        big.push(b'\n');
        client.get_mut().write_all(&big).await.unwrap();
        let reply = read_reply(&mut app, &mut messages, &mut client).await;
        assert_eq!(reply["status"], "refused", "{reply}");
        assert!(reply["reason"].as_str().unwrap().contains("larger than"));

        let mut rest = Vec::new();
        let ended = tokio::time::timeout(Duration::from_secs(5), client.read_to_end(&mut rest))
            .await
            .expect("the connection ends");
        assert!(
            ended.is_ok() && rest.is_empty(),
            "closed, nothing more sent"
        );
        assert_eq!(snapshot(&app), before);

        // A line exactly at the limit is read, and refused as malformed, not as large.
        let mut client = BufReader::new(UnixStream::connect(&socket).await.unwrap());
        let at_limit = "y".repeat(MAX_REQUEST_BYTES);
        let reply = ask(&mut app, &mut messages, &mut client, &at_limit).await;
        assert!(
            reply["reason"]
                .as_str()
                .unwrap()
                .starts_with("malformed request"),
            "{reply}"
        );
        server.abort();
    });
}

#[test]
fn a_wrong_token_over_the_socket_is_refused_and_logged() {
    let root = scratch();
    let (mut app, config, _dir) = app_and_config(root.path());
    let rt = runtime();
    rt.block_on(async {
        let (server, mut messages, socket) = start(&mut app, &config).await;
        let mut client = BufReader::new(UnixStream::connect(&socket).await.unwrap());
        let needle = format!(
            "Agent request (pid {}): (no command) -> refused: the session token is missing or wrong",
            std::process::id()
        );
        let logged = crate::diagnostics::logged_count(&needle);
        let wrong = json!({"token": "00", "id": "list_documents"}).to_string();
        let reply = ask(&mut app, &mut messages, &mut client, &wrong).await;
        assert_eq!(reply["status"], "refused");
        assert_eq!(crate::diagnostics::logged_count(&needle), logged + 1);
        server.abort();
    });
}

#[test]
fn turning_the_setting_off_closes_the_socket_and_drops_the_connections() {
    let root = scratch();
    let (mut app, config, dir) = app_and_config(root.path());
    let rt = runtime();
    let (socket, token_file, mut client) = rt.block_on(async {
        let (server, mut messages, socket) = start(&mut app, &config).await;
        let mut client = BufReader::new(UnixStream::connect(&socket).await.unwrap());
        let request = line(&app, "list_documents", json!({}));
        let listed = ask(&mut app, &mut messages, &mut client, &request).await;
        assert_eq!(listed["status"], "answered");
        assert!(socket.exists());

        // The runtime drops the subscription's future when the setting goes
        // off; aborting the task drops it the same way.
        server.abort();
        assert!(server.await.unwrap_err().is_cancelled());
        let token_file = super::super::listener::token_path(&dir, &config.boot_id);
        (socket, token_file, client)
    });
    rt.block_on(async {
        assert!(!socket.exists(), "the socket file is gone");
        assert!(!token_file.exists(), "the token file is gone");
        let mut rest = Vec::new();
        let ended = tokio::time::timeout(Duration::from_secs(5), client.read_to_end(&mut rest))
            .await
            .expect("the connection was dropped");
        // The end of the stream or a reset: either way no reply came.
        let _ended = ended;
        assert!(rest.is_empty());
        assert!(
            UnixStream::connect(&socket).await.is_err(),
            "nobody listens"
        );
    });
}

#[test]
fn saving_the_setting_off_ends_the_subscription_and_on_starts_one() {
    use crate::preferences::PrefMsg;
    let mut app = app_at_work();
    assert_eq!(
        app.agent_listener_subscription().units(),
        1,
        "on: a listener"
    );

    let first_token = app.agent.token().unwrap().as_str().to_string();
    let first_boot = app.agent.boot_id().unwrap().to_string();
    let inner = |msg| Message::Preferences(crate::app::PreferencesMsg::Inner(msg));
    let _t = app.update(Message::Preferences(crate::app::PreferencesMsg::Open));
    let _t = app.update(inner(PrefMsg::DraftAgentConnectionsEnabled(false)));
    assert!(app.ui_state.preferences_draft_differs());
    assert_eq!(
        app.agent_listener_subscription().units(),
        1,
        "a draft changes nothing"
    );
    let _t = app.update(inner(PrefMsg::Save));

    assert!(!app.ui_state.agent_connections_enabled);
    assert!(app.agent.endpoint.is_none(), "the app forgot the listener");
    assert_eq!(
        app.agent_listener_subscription().units(),
        0,
        "off: no listener"
    );
    let reply = super::call(&mut app, "list_documents", json!({}));
    assert_eq!(reply["status"], "refused");
    assert_eq!(reply["reason"], "agent connections are off");

    let _t = app.update(inner(PrefMsg::DraftAgentConnectionsEnabled(true)));
    let _t = app.update(inner(PrefMsg::Save));
    assert_eq!(app.agent_listener_subscription().units(), 1);
    // Off ends the old token and the old file names for good.
    assert_ne!(app.agent.token().unwrap().as_str(), first_token);
    assert_ne!(app.agent.boot_id().unwrap(), first_boot);
}

#[test]
fn the_listener_does_not_start_in_a_folder_that_is_too_open_or_over_a_link() {
    let root = scratch();
    let (_app, config, dir) = app_and_config(root.path());
    let rt = runtime();
    rt.block_on(async {
        // A folder of ours that others can enter.
        std::fs::DirBuilder::new().mode(0o755).create(&dir).unwrap();
        let (sender, mut messages) = mpsc::channel(4);
        tokio::spawn(serve(config.clone(), sender));
        let Some(Message::Agent(AgentMsg::ListenerFailed {
            boot_id,
            reason: why,
        })) = messages.next().await
        else {
            panic!("the listener started in an open folder");
        };
        assert_eq!(boot_id, config.boot_id, "a failure names its listener");
        assert!(why.contains("wider than 700"), "{why}");
        assert!(!dir.join(format!("{}.sock", config.boot_id)).exists());
        assert!(!token_path(&dir, &config.boot_id).exists());

        // A stale link where the socket goes is neither removed nor followed.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let precious = dir.join("precious");
        std::fs::write(&precious, "keep").unwrap();
        let stale = dir.join(format!("{}.sock", config.boot_id));
        symlink(&precious, &stale).unwrap();
        let (sender, mut messages) = mpsc::channel(4);
        tokio::spawn(serve(config.clone(), sender));
        let Some(Message::Agent(AgentMsg::ListenerFailed { reason: why, .. })) =
            messages.next().await
        else {
            panic!("the listener started over a link");
        };
        assert!(why.contains("is a link"), "{why}");
        assert!(stale.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&precious).unwrap(), "keep");
        assert!(
            !token_path(&dir, &config.boot_id).exists(),
            "the token a failed start wrote is removed"
        );
    });
}

#[test]
fn the_registry_is_written_for_open_projects_and_removed_on_close_and_on_exit() {
    let root = scratch();
    let (mut app, _config, dir) = app_and_config(root.path());
    let project = app.document_state.projects[0].path.clone();
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(&project, "{}").unwrap();
    let canonical = std::fs::canonicalize(&project).unwrap();
    let entry = dir
        .join(super::super::files::REGISTRY_DIR)
        .join(registry::entry_file_name(&canonical));
    let mut driver = crate::task_driver::TaskDriver::new();
    // The listener made the folder before it said it was up.
    super::super::files::prepare_dir(&dir).expect("the runtime folder");

    // The listener reports it is up: the open project gets its entry.
    let boot_id = app.agent.boot_id().unwrap().to_string();
    let task = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id,
        endpoint: "/run/test.sock".into(),
    }));
    driver
        .settle(&mut app, task)
        .expect("the registry write ran");
    let written: registry::Entry =
        serde_json::from_str(&std::fs::read_to_string(&entry).expect("an entry")).unwrap();
    assert_eq!(written.format, FORMAT);
    assert_eq!(written.project, canonical.to_string_lossy());
    assert_eq!(written.pid, std::process::id());
    assert_eq!(written.socket, "/run/test.sock");
    assert_eq!(written.boot_id.len(), 16);
    assert_eq!(mode(&entry), 0o600);

    // A project that closes loses its entry on the next update.
    let kept = std::mem::take(&mut app.document_state.projects);
    let task = app.update(Message::noop());
    driver
        .settle(&mut app, task)
        .expect("the registry write ran");
    assert!(!entry.exists(), "closed project, no entry");

    // Open again, then quit: the exit path removes it before the process goes.
    app.document_state.projects = kept;
    let task = app.update(Message::noop());
    driver
        .settle(&mut app, task)
        .expect("the registry write ran");
    assert!(entry.exists());
    let quit = app.flush_quit_writes_then(iced::Task::none());
    driver.settle(&mut app, quit).expect("the exit path ran");
    assert!(!entry.exists(), "the exit removed the entry");
}

#[test]
fn quitting_with_the_setting_off_removes_an_entry_of_this_run_that_outlived_its_listener() {
    let root = scratch();
    let (mut app, _config, dir) = app_and_config(root.path());
    let project = app.document_state.projects[0].path.clone();
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(&project, "{}").unwrap();
    let entry = dir
        .join(super::super::files::REGISTRY_DIR)
        .join(registry::entry_file_name(
            &std::fs::canonicalize(&project).unwrap(),
        ));
    super::super::files::prepare_dir(&dir).unwrap();
    let mut driver = crate::task_driver::TaskDriver::new();
    let boot_id = app.agent.boot_id().unwrap().to_string();
    let task = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id,
        endpoint: "/run/test.sock".into(),
    }));
    driver.settle(&mut app, task).unwrap();
    assert!(entry.exists());

    // The setting is off, and the listener's own cleanup never ran.
    app.ui_state.agent_connections_enabled = false;
    let quit = app.flush_quit_writes_then(iced::Task::none());
    driver.settle(&mut app, quit).unwrap();
    assert!(!entry.exists(), "the quit path removed it");
}

#[test]
fn cleanup_waits_for_a_registry_write_in_flight() {
    use std::sync::{Arc, Mutex, mpsc};
    let dir = scratch();
    let lock = Arc::new(Mutex::new(0_u64));
    let held = lock.lock().unwrap();
    let (done, finished) = mpsc::channel();
    let worker = {
        let (dir, lock) = (dir.path().to_path_buf(), lock.clone());
        std::thread::spawn(move || {
            super::super::listener::cleanup(&dir, "0123456789abcdef", &lock, true);
            done.send(()).unwrap();
        })
    };
    assert!(
        finished.recv_timeout(Duration::from_millis(300)).is_err(),
        "cleanup ran while a registry write held the lock"
    );
    drop(held);
    finished
        .recv_timeout(Duration::from_secs(10))
        .expect("cleanup runs once the lock is free");
    worker.join().unwrap();
}

#[test]
fn a_late_report_from_a_replaced_listener_changes_nothing() {
    let root = scratch();
    let (mut app, _config, _dir) = app_and_config(root.path());
    let current = app.agent.boot_id().unwrap().to_string();

    let task = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id: "0000000000000000".into(),
        endpoint: "/removed.sock".into(),
    }));
    assert_eq!(task.units(), 0, "no registry write for a stale listener");
    assert!(
        app.agent.endpoint.is_none(),
        "not pointed at a removed socket"
    );

    // A stale failure does not switch the live listener off.
    let _t = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id: current,
        endpoint: "/live.sock".into(),
    }));
    assert_eq!(app.agent.endpoint.as_deref(), Some("/live.sock"));
    let _t = app.update(Message::Agent(AgentMsg::ListenerFailed {
        boot_id: "0000000000000000".into(),
        reason: "old".into(),
    }));
    assert_eq!(app.agent.endpoint.as_deref(), Some("/live.sock"));

    let current = app.agent.boot_id().unwrap().to_string();
    let _t = app.update(Message::Agent(AgentMsg::ListenerFailed {
        boot_id: current,
        reason: "gave up".into(),
    }));
    assert!(app.agent.endpoint.is_none(), "its own failure does");
}

#[test]
fn a_client_that_never_reads_its_reply_is_dropped_after_the_write_timeout() {
    use super::super::listener::serve_connection_within;
    use super::super::{AgentMsg, Input};
    let rt = runtime();
    rt.block_on(async {
        // A pipe that holds 16 bytes, and a client that never reads from it.
        let (server_end, mut client) = tokio::io::duplex(16);
        let (sender, mut messages) = mpsc::channel(4);
        let connection = tokio::spawn(serve_connection_within(
            server_end,
            Some(7),
            sender,
            Duration::from_millis(200),
        ));
        client.write_all(b"{}\n").await.unwrap();
        let Some(Message::Agent(AgentMsg::Request(call))) = messages.next().await else {
            panic!("no request reached the app");
        };
        assert!(matches!(call.input, Input::Line(_)));
        call.reply.send("x".repeat(64 * 1024));

        let ended = tokio::time::timeout(Duration::from_secs(10), connection).await;
        assert!(ended.is_ok(), "the slot is still held");
    });
}

/// A project another live instance holds is registered here once that
/// instance is gone, without a write at every update.
#[test]
fn a_project_another_instance_held_is_registered_once_it_quits() {
    let root = scratch();
    let (mut app, _config, dir) = app_and_config(root.path());
    let project = app.document_state.projects[0].path.clone();
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(&project, "{}").unwrap();
    let canonical = std::fs::canonicalize(&project).unwrap();
    let entry = dir
        .join(super::super::files::REGISTRY_DIR)
        .join(registry::entry_file_name(&canonical));
    super::super::files::prepare_dir(&dir).unwrap();
    // Instance B (process 1, a socket that answers) registered the project first.
    let boot_id = "cccccccccccccccc";
    let socket = super::super::transport::endpoint_name(&dir, boot_id);
    let _answers = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let other = registry::Owner {
        pid: 1,
        boot_id: boot_id.into(),
        endpoint: socket,
    };
    registry::sync(&dir, &other, std::slice::from_ref(&project)).unwrap();
    let pid_in = |entry: &Path| {
        serde_json::from_str::<registry::Entry>(&std::fs::read_to_string(entry).unwrap())
            .unwrap()
            .pid
    };

    let mut driver = crate::task_driver::TaskDriver::new();
    let boot_id = app.agent.boot_id().unwrap().to_string();
    let task = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id,
        endpoint: "/run/ours.sock".into(),
    }));
    driver.settle(&mut app, task).unwrap();
    assert_eq!(pid_in(&entry), 1, "the other instance keeps its entry");
    assert!(app.agent.registry_retry, "the app knows it must try again");
    assert_eq!(
        app.agent_listener_subscription().units(),
        2,
        "listener + timer"
    );

    // The other instance quits and removes its entry.
    std::fs::remove_file(&entry).unwrap();
    // Ordinary updates do not write again.
    for _ in 0..3 {
        let task = app.update(Message::noop());
        driver.settle(&mut app, task).unwrap();
    }
    assert!(!entry.exists(), "no rewrite at every update");

    // The timer ticks: the project is ours.
    let task = app.update(Message::Agent(AgentMsg::RegistryRetryTick));
    driver.settle(&mut app, task).unwrap();
    assert_eq!(pid_in(&entry), std::process::id());
    assert!(!app.agent.registry_retry);
    assert_eq!(
        app.agent_listener_subscription().units(),
        1,
        "the timer is off"
    );
}

#[test]
fn a_failed_listener_is_restarted_with_new_ids_a_few_times_and_then_stays_off() {
    use super::super::handler::{MAX_RESTARTS, restart_delay};
    let mut app = app_at_work();
    // Listing the open project is another task of an update: no folder, no task.
    app.agent.runtime_dir = Err("not under test".into());
    assert_eq!(restart_delay(0), Some(Duration::from_secs(2)));
    assert_eq!(restart_delay(4), Some(Duration::from_secs(32)));
    assert_eq!(restart_delay(MAX_RESTARTS), None);

    let mut seen = vec![app.agent.boot_id().unwrap().to_string()];
    for attempt in 0..MAX_RESTARTS {
        let failed = seen.last().unwrap().clone();
        let task = app.update(Message::Agent(AgentMsg::ListenerFailed {
            boot_id: failed.clone(),
            reason: "no socket".into(),
        }));
        assert_eq!(task.units(), 1, "attempt {attempt}: a restart is scheduled");
        assert!(app.agent.endpoint.is_none());
        // A late timer for an older listener restarts nothing.
        let _t = app.update(Message::Agent(AgentMsg::RestartListener {
            boot_id: "0000000000000000".into(),
        }));
        assert_eq!(app.agent.boot_id(), Some(failed.as_str()));
        // The timer of this one gives the app new ids: a new subscription.
        let before = app.agent.token().unwrap().as_str().to_string();
        let _t = app.update(Message::Agent(AgentMsg::RestartListener {
            boot_id: failed,
        }));
        assert_ne!(app.agent.token().unwrap().as_str(), before);
        seen.push(app.agent.boot_id().unwrap().to_string());
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        seen.len(),
        "every restart has its own boot id"
    );

    // The next failure is the last: it says how to try again.
    let said = "it failed 5 times, so it stays off";
    let logged = crate::diagnostics::logged_count(said);
    let task = app.update(Message::Agent(AgentMsg::ListenerFailed {
        boot_id: seen.last().unwrap().clone(),
        reason: "no socket".into(),
    }));
    assert_eq!(task.units(), 0, "no more restarts");
    assert_eq!(crate::diagnostics::logged_count(said), logged + 1);

    // A start alone does not reset the count; a connection served does.
    app.agent.restarts = 3;
    let boot_id = app.agent.boot_id().unwrap().to_string();
    let _t = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id: boot_id.clone(),
        endpoint: "/live.sock".into(),
    }));
    assert_eq!(app.agent.restarts, 3);
    let _t = app.update(Message::Agent(AgentMsg::ListenerHealthy {
        boot_id: "0000000000000000".into(),
    }));
    assert_eq!(
        app.agent.restarts, 3,
        "a stale listener's health is not ours"
    );
    let _t = app.update(Message::Agent(AgentMsg::ListenerHealthy { boot_id }));
    assert_eq!(app.agent.restarts, 0);
}

/// A listener that binds and then fails again and again (the restart loop the
/// cap exists to stop) is restarted at most `MAX_RESTARTS` times, and the ids
/// it leaves behind do not pile up.
#[test]
fn a_listener_that_starts_and_fails_every_time_is_restarted_at_most_five_times() {
    use super::super::handler::MAX_RESTARTS;
    let mut app = app_at_work();
    app.agent.runtime_dir = Err("not under test".into());
    let mut scheduled = 0;
    for _round in 0..12 {
        let boot_id = app.agent.boot_id().unwrap().to_string();
        let _t = app.update(Message::Agent(AgentMsg::ListenerStarted {
            boot_id: boot_id.clone(),
            endpoint: "/live.sock".into(),
        }));
        let task = app.update(Message::Agent(AgentMsg::ListenerFailed {
            boot_id: boot_id.clone(),
            reason: "accept failed".into(),
        }));
        if task.units() > 0 {
            scheduled += 1;
        }
        let _t = app.update(Message::Agent(AgentMsg::RestartListener { boot_id }));
    }
    assert_eq!(scheduled, MAX_RESTARTS, "no endless restart loop");
    // The ids of the listeners that ended stay for the quit path, bounded.
    assert!(app.agent.touched.len() > 1, "{:?}", app.agent.touched);
    assert!(app.agent.touched.len() <= MAX_TOUCHED);
}

#[test]
fn the_quit_path_remembers_the_last_boot_ids_and_forgets_the_oldest() {
    let mut app = app_at_work();
    let mut ids = Vec::new();
    for _ in 0..(MAX_TOUCHED + 8) {
        let boot_id = app.agent.boot_id().unwrap().to_string();
        let _t = app.update(Message::Agent(AgentMsg::ListenerStarted {
            boot_id: boot_id.clone(),
            endpoint: "/live.sock".into(),
        }));
        ids.push(boot_id);
        app.agent.rotate_ids();
    }
    assert_eq!(app.agent.touched.len(), MAX_TOUCHED);
    assert_eq!(app.agent.touched.first(), ids.get(8), "the oldest are gone");
    assert_eq!(app.agent.touched.last(), ids.last(), "the newest stays");
}

/// A registry write of a listener that has ended can land after that
/// listener's cleanup; the quit path still removes what it wrote.
#[test]
fn a_late_registry_write_of_an_ended_listener_is_removed_on_quit() {
    let root = scratch();
    let (mut app, _config, dir) = app_and_config(root.path());
    let project = app.document_state.projects[0].path.clone();
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(&project, "{}").unwrap();
    let entry = dir
        .join(super::super::files::REGISTRY_DIR)
        .join(registry::entry_file_name(
            &std::fs::canonicalize(&project).unwrap(),
        ));
    super::super::files::prepare_dir(&dir).unwrap();
    let old = app.agent.boot_id().unwrap().to_string();
    let _t = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id: old.clone(),
        endpoint: "/run/old.sock".into(),
    }));
    // The listener fails and is replaced; its slow write lands afterwards.
    let _t = app.update(Message::Agent(AgentMsg::ListenerFailed {
        boot_id: old.clone(),
        reason: "gone".into(),
    }));
    app.agent.rotate_ids();
    let late = registry::Owner {
        pid: std::process::id(),
        boot_id: old,
        endpoint: "/run/old.sock".into(),
    };
    registry::sync(&dir, &late, std::slice::from_ref(&project)).unwrap();
    assert!(entry.exists());

    let mut driver = crate::task_driver::TaskDriver::new();
    let quit = app.flush_quit_writes_then(iced::Task::none());
    driver.settle(&mut app, quit).unwrap();
    assert!(!entry.exists(), "the quit path removed it");
}

#[test]
fn the_listener_reports_healthy_once_after_its_first_connection() {
    let root = scratch();
    let (mut app, config, _dir) = app_and_config(root.path());
    let rt = runtime();
    rt.block_on(async {
        let (server, mut messages, socket) = start(&mut app, &config).await;
        let _first = UnixStream::connect(&socket).await.unwrap();
        let next = tokio::time::timeout(Duration::from_secs(5), messages.next())
            .await
            .expect("a message after the connection");
        let Some(Message::Agent(AgentMsg::ListenerHealthy { boot_id })) = next else {
            panic!("the first connection did not report the listener healthy");
        };
        assert_eq!(boot_id, config.boot_id);
        let _second = UnixStream::connect(&socket).await.unwrap();
        // Each connection is also reported as opened (#2253, the access
        // journal): two, from this process, one for each connection.
        for connection in 1..=2 {
            let message = tokio::time::timeout(Duration::from_secs(5), messages.next())
                .await
                .unwrap_or_else(|_| panic!("no message for connection {connection}"));
            let Some(Message::Agent(AgentMsg::ConnectionOpened { pid })) = message else {
                panic!("connection {connection}: not reported opened: {message:?}");
            };
            assert_eq!(pid, Some(std::process::id()), "connection {connection}");
        }
        // And nothing more: the listener's health is reported once, not for
        // every connection.
        let again = tokio::time::timeout(Duration::from_millis(300), messages.next()).await;
        assert!(again.is_err(), "reported once, not for every connection");
        server.abort();
    });
}

/// The result of a registry write of a listener that is gone must not decide
/// whether the next listener's first failure is said.
#[test]
fn a_late_registry_failure_of_a_gone_listener_does_not_hide_the_next_ones() {
    let mut app = app_at_work();
    let current = app.agent.boot_id().unwrap().to_string();
    let _t = app.update(Message::Agent(AgentMsg::ListenerFailed {
        boot_id: current.clone(),
        reason: "closed".into(),
    }));
    // A failure from the listener that is gone arrives late.
    let _t = app.update(Message::Agent(AgentMsg::RegistrySynced {
        boot_id: "0000000000000000".into(),
        result: Err("late failure of the old one".into()),
    }));
    assert!(!app.agent.registry_retry, "a stale result is ignored");

    let boot_id = app.agent.boot_id().unwrap().to_string();
    let _t = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id: boot_id.clone(),
        endpoint: "/live.sock".into(),
    }));
    let said = "first failure of the new one";
    let logged = crate::diagnostics::logged_count(said);
    let _t = app.update(Message::Agent(AgentMsg::RegistrySynced {
        boot_id,
        result: Err(said.into()),
    }));
    assert_eq!(crate::diagnostics::logged_count(said), logged + 1);

    // A start also clears what the last listener left.
    app.agent.registry_retry = true;
    let boot_id = app.agent.boot_id().unwrap().to_string();
    let _t = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id,
        endpoint: "/live.sock".into(),
    }));
    assert!(!app.agent.registry_retry);
}

#[test]
fn a_retry_tick_waits_for_a_registry_write_still_running() {
    let mut app = app_at_work();
    let boot_id = app.agent.boot_id().unwrap().to_string();
    // The write is started and has not finished.
    let started = app.update(Message::Agent(AgentMsg::ListenerStarted {
        boot_id: boot_id.clone(),
        endpoint: "/live.sock".into(),
    }));
    assert!(started.units() > 0);
    assert!(app.agent.registry_in_flight);
    let tick = app.update(Message::Agent(AgentMsg::RegistryRetryTick));
    assert_eq!(tick.units(), 0, "no second write while one is running");

    // Once it has finished, the tick works again.
    let _t = app.update(Message::Agent(AgentMsg::RegistrySynced {
        boot_id,
        result: Err("a project is held".into()),
    }));
    assert!(!app.agent.registry_in_flight);
    let tick = app.update(Message::Agent(AgentMsg::RegistryRetryTick));
    assert!(tick.units() > 0);
}
