//! The open Preferences dialog every row of this batch drives, and the guard
//! over the config files a Save or a page action writes.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::{PrefMsg, PrefNav};

use crate::settle_support::settle;

/// One config root per test process: the tests that write it take turns.
static SHARED_CONFIG: Mutex<()> = Mutex::new(());

/// Holds the config root and puts its files back when dropped, also after a
/// failed assertion.
pub(super) struct ConfigGuard {
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
    _turn: MutexGuard<'static, ()>,
}

impl ConfigGuard {
    pub fn capture() -> Self {
        let turn = SHARED_CONFIG
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let paths = [
            Some(signex_app::fonts::prefs_file_path()),
            signex_app::keymap::config_path(),
            signex_app::library::settings::persistence::config_path(),
        ];
        let files = paths
            .into_iter()
            .flatten()
            .map(|path| {
                let bytes = std::fs::read(&path).ok();
                (path, bytes)
            })
            .collect();
        Self { files, _turn: turn }
    }
}

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        for (path, bytes) in &self.files {
            let restored = match bytes {
                Some(bytes) => std::fs::write(path, bytes),
                None => match std::fs::remove_file(path) {
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                    _ => Ok(()),
                },
            };
            if let Err(error) = restored {
                eprintln!("could not restore {}: {error}", path.display());
            }
        }
    }
}

pub(super) fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

/// An app with Preferences open.
pub(super) struct Prefs {
    pub app: Signex,
    _config: ConfigGuard,
}

pub(super) fn opened() -> Prefs {
    let config = ConfigGuard::capture();
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    Prefs {
        app,
        _config: config,
    }
}

impl Prefs {
    /// Send a page message. Its background writes are not run, so a Save
    /// commits to the app and leaves `prefs.json` alone.
    pub fn send(&mut self, msg: PrefMsg) {
        let _ = self.app.update(inner(msg));
    }

    /// Send a page message and run every task it starts.
    pub fn settle(&mut self, msg: PrefMsg) {
        settle(&mut self.app, inner(msg));
    }

    pub fn apply(&mut self) {
        self.send(PrefMsg::Save);
    }

    pub fn ok(&mut self) {
        self.send(PrefMsg::SaveAndClose);
    }

    pub fn cancel(&mut self) {
        self.send(PrefMsg::DiscardAndClose);
    }

    pub fn is_open(&self) -> bool {
        self.app.ui_state.preferences_open
    }

    pub fn is_dirty(&self) -> bool {
        self.app.ui_state.preferences_dirty
    }

    pub fn dirty_pages(&self) -> Vec<PrefNav> {
        self.app.ui_state.preferences_pages.dirty_pages.clone()
    }
}
