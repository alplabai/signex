//! The running app's local agent channel (#1924): a person keeps Signex open
//! and an agent edits the same project through the CLI, without taking the
//! person's focus, tab, selection or tool.
//!
//! The listener (`listener`, `transport`) is an iced `Subscription`; each
//! request line becomes a [`AgentMsg::Request`] that `Signex::update` runs
//! like a click (`handler`), so a person's edit and an agent's request never
//! interleave. Which commands are accepted is derived from the catalog
//! (`allow`); the reply is the headless runner's envelope (`reply`). The
//! security rules, the wire format and the registry format are in
//! `docs/AGENT_SOCKET.md`.

mod allow;
mod files;
mod handler;
mod listener;
mod registry;
mod reply;
mod state;
mod token;
mod transport;
mod wire;

#[cfg(test)]
mod tests;

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use iced::futures::channel::oneshot;

pub use reply::{
    ChangeSnapshot, MessageLine, Status, changed_between, messages_since, outcome_verdict,
};
pub(crate) use state::AgentState;

/// The agent channel's messages, routed from `Message::Agent`.
#[derive(Debug, Clone)]
pub enum AgentMsg {
    /// A request line arrived on a connection (or the connection could not
    /// frame one); the app answers it through the call's reply.
    Request(AgentCall),
    /// A client was turned away at connect, before a byte was read.
    PeerRefused { pid: Option<u32>, reason: String },
    /// The listener of `boot_id` is up at `endpoint` (a socket path or a pipe
    /// name). A report of a boot id that is no longer the app's is ignored.
    ListenerStarted { boot_id: String, endpoint: String },
    /// The listener of `boot_id` could not start or gave up.
    ListenerFailed { boot_id: String, reason: String },
    /// The listener of `boot_id` accepted its first connection: it works, so
    /// a failure after this starts the restart count again.
    ListenerHealthy { boot_id: String },
    /// A registry write of the listener `boot_id` finished. A result of a
    /// listener that is gone is ignored.
    RegistrySynced {
        boot_id: String,
        result: Result<(), String>,
    },
    /// The retry timer: write the registry again (see `registry_retry`).
    RegistryRetryTick,
    /// The backoff after a failed listener is over: start a new one, unless
    /// `boot_id` is no longer the app's.
    RestartListener { boot_id: String },
}

/// What a connection read, before the app has looked at it.
#[derive(Clone)]
pub(crate) enum Input {
    Line(String),
    /// A line over [`wire::MAX_REQUEST_BYTES`].
    Oversized,
    NotUtf8,
}

/// One request and the way to answer it.
#[derive(Clone)]
pub struct AgentCall {
    pub(crate) pid: Option<u32>,
    pub(crate) input: Input,
    pub(crate) reply: AgentReply,
}

/// Never prints the request: its line carries the session token.
impl fmt::Debug for AgentCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size = match &self.input {
            Input::Line(line) => line.len(),
            Input::Oversized | Input::NotUtf8 => 0,
        };
        write!(f, "AgentCall {{ pid: {:?}, bytes: {size} }}", self.pid)
    }
}

/// Where the answer to one request goes: back to its connection. A `Message`
/// is `Clone`, so the sender sits behind a lock and is used once.
#[derive(Clone)]
pub struct AgentReply(Arc<Mutex<Option<oneshot::Sender<String>>>>);

impl AgentReply {
    pub(crate) fn channel() -> (Self, oneshot::Receiver<String>) {
        let (sender, receiver) = oneshot::channel();
        (Self(Arc::new(Mutex::new(Some(sender)))), receiver)
    }

    /// Answer with `line`. The first answer is sent; a second is dropped.
    pub(crate) fn send(&self, line: String) {
        let sender = self.0.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(sender) = sender {
            // The connection may have closed meanwhile; there is nobody to tell.
            sender.send(line).ok();
        }
    }
}

impl fmt::Debug for AgentReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AgentReply")
    }
}
