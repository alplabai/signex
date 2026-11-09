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
use super::super::listener::token_path;
use super::super::listener::{ListenerConfig, serve};
use super::super::registry::{self, FORMAT};
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
    let Message::Agent(AgentMsg::ListenerStarted { endpoint }) = &first else {
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
        let Some(Message::Agent(AgentMsg::ListenerFailed(why))) = messages.next().await else {
            panic!("the listener started in an open folder");
        };
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
        let Some(Message::Agent(AgentMsg::ListenerFailed(why))) = messages.next().await else {
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
    let task = app.update(Message::Agent(AgentMsg::ListenerStarted {
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
