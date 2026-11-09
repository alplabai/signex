//! What the app keeps for the agent channel (#1924): this boot's id and token,
//! where the runtime directory is, and what the listener has reported. Nothing
//! here opens a file or a socket; that happens in the listener's subscription.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::files;
use super::listener::ListenerConfig;
use super::registry::Owner;
use super::token::{Token, boot_id};

pub(crate) struct AgentState {
    /// `None` when the system gave no randomness: the listener never starts.
    ids: Option<(String, Token)>,
    pub(super) runtime_dir: Result<PathBuf, String>,
    /// The socket (or pipe) the listener reported once it was up; `None`
    /// while it is not listening.
    pub(super) endpoint: Option<String>,
    /// The projects the registry was last written for.
    pub(super) registered: Option<Vec<PathBuf>>,
    /// True while an agent request runs, so the code under it can tell an
    /// agent's call from a click.
    pub(crate) serving: bool,
    /// Serialises registry writes, which run on worker threads.
    pub(super) registry_lock: Arc<Mutex<()>>,
}

impl AgentState {
    pub(crate) fn new() -> Self {
        let ids = boot_id().zip(Token::generate());
        Self {
            ids,
            runtime_dir: files::runtime_dir(),
            endpoint: None,
            registered: None,
            serving: false,
            registry_lock: Arc::default(),
        }
    }

    pub(super) fn token(&self) -> Option<&Token> {
        self.ids.as_ref().map(|(_, token)| token)
    }

    pub(super) fn runtime_dir(&self) -> Result<&PathBuf, &str> {
        self.runtime_dir.as_ref().map_err(String::as_str)
    }

    /// What the listener's subscription runs on, `None` while the setting is
    /// off (nothing is then created) or this boot has no ids or directory.
    pub(super) fn listener_config(&self, enabled: bool) -> Option<ListenerConfig> {
        if !enabled {
            return None;
        }
        let (boot_id, token) = self.ids.as_ref()?;
        Some(ListenerConfig {
            dir: self.runtime_dir.as_ref().ok()?.clone(),
            boot_id: boot_id.clone(),
            token: token.clone(),
        })
    }

    /// This run as the registry entries name it, once the listener is up.
    pub(super) fn owner(&self) -> Option<Owner> {
        let endpoint = self.endpoint.clone()?;
        Some(Owner {
            pid: std::process::id(),
            boot_id: self.ids.as_ref()?.0.clone(),
            endpoint,
        })
    }

    /// Forget a running listener: the setting went off, or it failed.
    pub(super) fn listener_closed(&mut self) {
        self.endpoint = None;
        self.registered = None;
    }

    /// New ids for the next time the setting goes on. A listener that is
    /// being dropped removes the files named by its own boot id, which a new
    /// one must not share, and an old token never works again.
    pub(super) fn rotate_ids(&mut self) {
        self.ids = boot_id().zip(Token::generate());
    }

    /// This boot's id, for the tests that look for its files.
    #[cfg(test)]
    pub(super) fn boot_id(&self) -> Option<&str> {
        self.ids.as_ref().map(|(id, _)| id.as_str())
    }
}
