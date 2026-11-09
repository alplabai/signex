//! The listener, as an iced `Subscription` (#1924). One future owns the
//! socket, every connection and the files in the runtime directory: when the
//! setting goes off the runtime drops the subscription, and with it the
//! listener, the connections and (through [`Guard`]) the socket, the token and
//! the registry entries. A connection only frames lines: each becomes a
//! `Message::Agent` that `Signex::update` answers, so nothing here touches the
//! app's state. See `docs/AGENT_SOCKET.md`.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use iced::Subscription;
use iced::futures::channel::mpsc;
use iced::futures::stream::FuturesUnordered;
use iced::futures::{SinkExt as _, Stream, StreamExt as _};
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, BufReader,
};

use super::files;
use super::registry::{self, Locks, Owner};
use super::token::Token;
use super::transport::{Accepted, Listener, endpoint_name};
use super::wire::MAX_REQUEST_BYTES;
use super::{AgentCall, AgentMsg, AgentReply, Input};
use crate::app::Message;

/// How many agent connections are served at once; more are dropped.
pub(super) const MAX_CONNECTIONS: usize = 8;

/// How long a connection may sit without sending a request.
pub(super) const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// How much of an oversized request is read and dropped, and for how long,
/// before its connection is closed.
const MAX_DRAINED_BYTES: u64 = 64 * 1024 * 1024;
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// How long writing one reply may take; a client that does not read its
/// replies is dropped after it.
pub(super) const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Failed accepts in a row before the listener gives up.
const MAX_ACCEPT_FAILURES: u32 = 20;

/// What one run of the listener is made from. Its identity (the subscription's)
/// is the folder, the boot id and the token: a new boot or token is a new
/// listener.
#[derive(Clone)]
pub(super) struct ListenerConfig {
    pub(super) dir: PathBuf,
    pub(super) boot_id: String,
    pub(super) token: Token,
    /// Held while the registry is written or removed.
    pub(super) registry_lock: Arc<Mutex<Locks>>,
}

impl Hash for ListenerConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.dir.hash(state);
        self.boot_id.hash(state);
        self.token.hash(state);
    }
}

/// The subscription that runs the listener, or none while `config` is `None`.
pub(super) fn subscription(config: Option<ListenerConfig>) -> Subscription<Message> {
    match config {
        Some(config) => Subscription::run_with(config, |config| run(config.clone())),
        None => Subscription::none(),
    }
}

fn run(config: ListenerConfig) -> impl Stream<Item = Message> {
    iced::stream::channel(16, async move |out| serve(config, out).await)
}

/// Removes this boot's token, socket and registry entries when the listener
/// ends, however it ends: the setting went off, the app is quitting or the
/// runtime dropped the task.
struct Guard {
    dir: PathBuf,
    boot_id: String,
    registry_lock: Arc<Mutex<Locks>>,
}

impl Drop for Guard {
    /// The entries stay: the app still has its projects open, and the next
    /// registry write makes them the entries of an app that does not listen.
    fn drop(&mut self) {
        cleanup(&self.dir, &self.boot_id, &self.registry_lock, false);
    }
}

/// Remove the token and the socket of `boot_id`, and with `entries` the
/// registry entries of this process and its locks (the app is quitting), under
/// the registry lock so it does not interleave with a write. A write that runs
/// after it can still land; the quit path cleans again. Safe to repeat; what
/// cannot be removed is said in Messages.
pub(super) fn cleanup(
    dir: &std::path::Path,
    boot_id: &str,
    registry_lock: &Mutex<Locks>,
    entries: bool,
) {
    let owner = Owner {
        pid: std::process::id(),
        boot_id: boot_id.to_string(),
        endpoint: endpoint_name(dir, boot_id),
    };
    let mut locks = registry_lock.lock().unwrap_or_else(PoisonError::into_inner);
    let mut results = vec![
        files::remove_if_ours(&token_path(dir, boot_id)),
        Listener::remove(dir, boot_id),
    ];
    if entries {
        results.push(registry::remove_all(dir, &owner, &mut locks));
    }
    for why in results.into_iter().filter_map(Result::err) {
        crate::diagnostics::log_warning(format!("Agent connections: cleanup: {why}"));
    }
}

/// Where the session token of `boot_id` is stored, beside the socket.
pub(super) fn token_path(dir: &std::path::Path, boot_id: &str) -> PathBuf {
    dir.join(format!("{boot_id}.token"))
}

/// Prepare the folder, store the token and bind, off the update thread. The
/// guard is made before the first file, so a failure part way removes it.
async fn start(config: &ListenerConfig) -> Result<(Listener, Guard), String> {
    let config = config.clone();
    tokio::task::spawn_blocking(move || {
        files::prepare_dir(&config.dir)?;
        let guard = Guard {
            dir: config.dir.clone(),
            boot_id: config.boot_id.clone(),
            registry_lock: config.registry_lock.clone(),
        };
        files::write_private(
            &token_path(&config.dir, &config.boot_id),
            config.token.as_str().as_bytes(),
        )?;
        let listener = Listener::bind(&config.dir, &config.boot_id)
            .map_err(|error| format!("cannot listen: {error}"))?;
        Ok((listener, guard))
    })
    .await
    .map_err(|error| format!("the listener's start did not finish: {error}"))?
}

/// Run the listener until its future is dropped (or it fails): bind, say so,
/// then accept and serve connections.
pub(super) async fn serve(config: ListenerConfig, mut out: mpsc::Sender<Message>) {
    let (mut listener, _guard) = match start(&config).await {
        Ok(started) => started,
        Err(reason) => {
            let boot_id = config.boot_id.clone();
            say(&mut out, AgentMsg::ListenerFailed { boot_id, reason }).await;
            return;
        }
    };
    let endpoint = endpoint_name(&config.dir, &config.boot_id);
    let boot_id = config.boot_id.clone();
    if !say(&mut out, AgentMsg::ListenerStarted { boot_id, endpoint }).await {
        return;
    }
    let mut connections = FuturesUnordered::new();
    let mut failures = 0_u32;
    let mut healthy = false;
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Err(error) => {
                    failures += 1;
                    if failures > MAX_ACCEPT_FAILURES {
                        let boot_id = config.boot_id.clone();
                        let reason = format!("accept failed: {error}");
                        say(&mut out, AgentMsg::ListenerFailed { boot_id, reason }).await;
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Ok(Accepted::Refused { pid, reason }) => {
                    failures = 0;
                    if !say(&mut out, AgentMsg::PeerRefused { pid, reason }).await {
                        return;
                    }
                }
                Ok(Accepted::Connection { conn, pid }) => {
                    failures = 0;
                    if !healthy {
                        // The first connection shows the listener works.
                        healthy = true;
                        let boot_id = config.boot_id.clone();
                        if !say(&mut out, AgentMsg::ListenerHealthy { boot_id }).await {
                            return;
                        }
                    }
                    if connections.len() >= MAX_CONNECTIONS {
                        drop(conn);
                        let reason = format!("more than {MAX_CONNECTIONS} connections at once");
                        if !say(&mut out, AgentMsg::PeerRefused { pid, reason }).await {
                            return;
                        }
                    } else {
                        connections.push(serve_connection(conn, pid, out.clone()));
                    }
                }
            },
            Some(()) = connections.next(), if !connections.is_empty() => {}
        }
    }
}

/// Send `msg` to the app; `false` once the app is no longer listening.
async fn say(out: &mut mpsc::Sender<Message>, msg: AgentMsg) -> bool {
    out.send(Message::Agent(msg)).await.is_ok()
}

/// One connection: each line is a request, answered by one line, in order.
/// The connection ends when the client closes it, sits idle too long, sends a
/// line over the limit or the app stops answering.
pub(super) async fn serve_connection<S>(stream: S, pid: Option<u32>, out: mpsc::Sender<Message>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    serve_connection_within(stream, pid, out, WRITE_TIMEOUT).await;
}

/// [`serve_connection`] closing the connection when a reply takes longer than
/// `write_timeout` to write.
pub(super) async fn serve_connection_within<S>(
    stream: S,
    pid: Option<u32>,
    mut out: mpsc::Sender<Message>,
    write_timeout: Duration,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read, mut write) = tokio::io::split(stream);
    let mut read = BufReader::new(read);
    loop {
        let Some(input) = read_input(&mut read).await else {
            return;
        };
        // After a line over the limit the bytes cannot be framed again.
        let last = matches!(input, Input::Oversized);
        let (reply, answer) = AgentReply::channel();
        if !say(&mut out, AgentMsg::Request(AgentCall { pid, input, reply })).await {
            return;
        }
        let Ok(line) = answer.await else {
            return;
        };
        let written = async {
            write.write_all(line.as_bytes()).await?;
            write.write_all(b"\n").await?;
            write.flush().await
        };
        // A client that does not read must not hold its slot for ever.
        if !matches!(
            tokio::time::timeout(write_timeout, written).await,
            Ok(Ok(()))
        ) {
            return;
        }
        if last {
            refuse_the_rest(write, read).await;
            return;
        }
    }
}

/// End a connection whose request was over the limit: the client gets its
/// reply and the end of the stream, then what it was still sending is read
/// and dropped for a short while, so closing does not reset the connection
/// before the reply is read.
async fn refuse_the_rest<W, R>(mut write: W, mut read: BufReader<R>)
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    write.shutdown().await.ok();
    let mut limited = (&mut read).take(MAX_DRAINED_BYTES);
    let mut sink = tokio::io::sink();
    let rest = tokio::io::copy(&mut limited, &mut sink);
    tokio::time::timeout(DRAIN_TIMEOUT, rest).await.ok();
}

/// The next request line: at most [`MAX_REQUEST_BYTES`] of it is read. `None`
/// when the client is gone, idle for [`IDLE_TIMEOUT`] or unreadable.
async fn read_input<R>(read: &mut BufReader<R>) -> Option<Input>
where
    R: AsyncRead + Unpin,
{
    let mut line = Vec::new();
    // Room for the newline and an optional carriage return.
    let window = (MAX_REQUEST_BYTES + 2) as u64;
    let mut limited = (&mut *read).take(window);
    let next = limited.read_until(b'\n', &mut line);
    match tokio::time::timeout(IDLE_TIMEOUT, next).await {
        Err(_) | Ok(Err(_)) | Ok(Ok(0)) => return None,
        Ok(Ok(_)) => {}
    }
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    }
    if line.len() > MAX_REQUEST_BYTES {
        return Some(Input::Oversized);
    }
    Some(match String::from_utf8(line) {
        Ok(text) => Input::Line(text),
        Err(_) => Input::NotUtf8,
    })
}
