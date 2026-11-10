//! What the app keeps for the agent channel (#1924): this boot's id and token,
//! where the runtime directory is, and what the listener has reported. Nothing
//! here opens a file or a socket; that happens in the listener's subscription.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::files;
use super::listener::ListenerConfig;
use super::registry::Owner;
use super::token::{Token, boot_id};

/// How many boot ids the quit path remembers (restarts and toggles each add
/// one); cleaning an id twice is harmless.
pub(super) const MAX_TOUCHED: usize = 32;

pub(crate) struct AgentState {
    /// `None` when the system gave no randomness: the listener never starts.
    ids: Option<(String, Token)>,
    pub(super) runtime_dir: Result<PathBuf, String>,
    /// The temporary folder a test run's runtime directory is in, removed with
    /// the app (see `files::test_runtime_dir`); `None` outside tests.
    _test_dir: Option<tempfile::TempDir>,
    /// The user's other possible runtime folders to look in for a headless
    /// run's lock; `None` asks `files::alternate_dirs` each time (a test names
    /// its own).
    pub(super) alternates: Option<Vec<PathBuf>>,
    /// The socket (or pipe) the listener reported once it was up; `None`
    /// while it is not listening.
    pub(super) endpoint: Option<String>,
    /// The projects the registry was last written for.
    pub(super) registered: Option<Vec<PathBuf>>,
    /// True while an agent request runs, so the code under it can tell an
    /// agent's call from a click.
    pub(crate) serving: bool,
    /// Who the command running now comes from; the person outside
    /// [`Signex::with_origin`].
    pub(crate) origin: super::Origin,
    /// Whether agent commands and connections are journaled (#2253): the
    /// window always does; a headless run only when it may save.
    pub(crate) journals_on: bool,
    /// The name a headless run gave itself, for the lines of its commands.
    pub(crate) run_client: Option<String>,
    /// Whether the request being answered got as far as its command's
    /// handler; a refusal before that is an access event.
    pub(super) reached_handler: bool,
    /// The journal lines waiting to be written, by project folder.
    pub(crate) journal: super::journal::queue::JournalQueue,
    /// What each client process last said it may do, so a change is recorded
    /// once and not at every request.
    pub(super) grants_seen: std::collections::BTreeMap<Option<u32>, (Option<String>, Vec<String>)>,
    /// Whether the open projects are listed in the registry: the window the
    /// person works in (`Signex::boot`) does. An app made otherwise (a test, a
    /// headless run, which holds its project itself) does not.
    pub(crate) lists_projects: bool,
    /// Serialises registry writes and removals, which run on worker threads;
    /// holds the number of the last write that ran (`registry::sync_in_order`).
    pub(super) registry_lock: Arc<Mutex<u64>>,
    /// The number of the last write started.
    pub(super) registry_seq: u64,
    /// The shared lock this window holds on each project it has open, by the
    /// project's path, for as long as it stays open (see `claim_project`).
    pub(super) project_locks: std::collections::BTreeMap<PathBuf, std::fs::File>,
    /// The open projects whose lock could not be made (it failed, or a run
    /// took the project first): not tried again at every update, only at the
    /// registry retry tick.
    pub(super) lock_failed: std::collections::BTreeSet<PathBuf>,
    /// Of those, the ones already said in Messages, so it is said once.
    pub(super) lock_warned: std::collections::BTreeSet<PathBuf>,
    /// The boot ids whose files this app may have made, for the quit path.
    pub(super) touched: Vec<String>,
    /// The last registry write failed or left a project to another instance;
    /// while it is set (and a listener is up) a timer retries it.
    pub(super) registry_retry: bool,
    /// A registry write is running; a retry tick waits for it.
    pub(super) registry_in_flight: bool,
    /// How many times a failed listener was restarted since it last started.
    pub(super) restarts: u32,
}

impl AgentState {
    pub(crate) fn new() -> Self {
        let ids = boot_id().zip(Token::generate());
        let (runtime_dir, test_dir) = if cfg!(test) {
            let (dir, root) = files::test_runtime_dir();
            (Ok(dir), root)
        } else {
            (files::runtime_dir(), None)
        };
        Self {
            ids,
            runtime_dir,
            _test_dir: test_dir,
            alternates: None,
            endpoint: None,
            registered: None,
            serving: false,
            origin: super::Origin::Person,
            journals_on: false,
            run_client: None,
            reached_handler: false,
            journal: super::journal::queue::JournalQueue::default(),
            grants_seen: std::collections::BTreeMap::new(),
            lists_projects: false,
            registry_lock: Arc::default(),
            registry_seq: 0,
            project_locks: std::collections::BTreeMap::new(),
            lock_failed: std::collections::BTreeSet::new(),
            lock_warned: std::collections::BTreeSet::new(),
            touched: Vec::new(),
            registry_retry: false,
            registry_in_flight: false,
            restarts: 0,
        }
    }

    /// Use the runtime directory of a test run (an integration test boots the
    /// real window): it must never list its projects in the user's registry.
    pub(crate) fn use_test_runtime_dir(&mut self) {
        let (dir, root) = files::test_runtime_dir();
        self.runtime_dir = Ok(dir);
        self._test_dir = root;
    }

    /// The other runtime folders to look in for a headless run's lock.
    pub(super) fn alternate_dirs(&self, primary: &std::path::Path) -> Vec<PathBuf> {
        self.alternates
            .clone()
            .unwrap_or_else(|| files::alternate_dirs(primary))
    }

    pub(super) fn token(&self) -> Option<&Token> {
        self.ids.as_ref().map(|(_, token)| token)
    }

    /// The session token as text, for the one place that must keep it out of
    /// a journal.
    pub(super) fn token_text(&self) -> Option<&str> {
        self.token().map(Token::as_str)
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
            registry_lock: self.registry_lock.clone(),
        })
    }

    /// This run as the registry entries name it. Its endpoint is empty while no
    /// listener is up: the entries then say the app has the project open and
    /// does not listen.
    pub(super) fn owner(&self) -> Option<Owner> {
        Some(Owner {
            pid: std::process::id(),
            boot_id: self.ids.as_ref()?.0.clone(),
            endpoint: self.endpoint.clone().unwrap_or_default(),
        })
    }

    /// Forget a running listener: the setting went off, or it failed.
    pub(super) fn listener_closed(&mut self) {
        self.endpoint = None;
        self.registered = None;
        self.registry_retry = false;
        self.registry_in_flight = false;
    }

    /// New ids for the next time the setting goes on. A listener that is
    /// being dropped removes the files named by its own boot id, which a new
    /// one must not share, and an old token never works again. The old id
    /// stays in `touched`: a registry write of that listener can still land
    /// after its cleanup, and the quit path removes it.
    pub(super) fn rotate_ids(&mut self) {
        self.ids = boot_id().zip(Token::generate());
    }

    /// Remember that the listener `boot_id` ran, for the quit path. The list
    /// is bounded: past [`MAX_TOUCHED`] the oldest id is dropped.
    pub(super) fn note_touched(&mut self, boot_id: String) {
        if self.touched.contains(&boot_id) {
            return;
        }
        self.touched.push(boot_id);
        if self.touched.len() > MAX_TOUCHED {
            self.touched.remove(0);
        }
    }

    /// The id of the listener the app wants now.
    pub(super) fn boot_id(&self) -> Option<&str> {
        self.ids.as_ref().map(|(id, _)| id.as_str())
    }
}
