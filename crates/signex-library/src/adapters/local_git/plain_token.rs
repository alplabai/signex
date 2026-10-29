//! The one-time warning that a saved database token sits in the `.snxlib`
//! in plain text.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex};

use super::LocalGitAdapter;
use crate::manifest::{LibraryMode, auth_is_plain_token};

/// Libraries already warned about in this process, keyed by canonical file
/// path. The app opens short-lived adapters for pricing, placing, cascades
/// and drift checks, so a per-adapter flag would repeat the warning. Only
/// log dedupe lives here; no behaviour depends on it.
static WARNED_LIBRARIES: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Mutex::default);

impl LocalGitAdapter {
    /// Warn once per library per session that a saved database token sits in
    /// the `.snxlib` in plain text, and name the `${env:SIGNEX_NAME}`
    /// alternative. Returns whether it warned.
    pub(super) fn warn_plain_token_once(&self, mode: &LibraryMode) -> bool {
        let LibraryMode::Database { auth, .. } = mode else {
            return false;
        };
        if !auth_is_plain_token(auth) {
            return false;
        }
        let key = self
            .file_path
            .canonicalize()
            .unwrap_or_else(|_| self.file_path.clone());
        let first = WARNED_LIBRARIES
            .lock()
            .map(|mut warned| warned.insert(key))
            .unwrap_or(false);
        if !first {
            return false;
        }
        self.plain_token_warnings.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            target: "signex::library",
            path = %self.file_path.display(),
            "this library's database token is saved in plain text in the .snxlib; set auth to \
             ${{env:SIGNEX_NAME}} and keep the token in that environment variable (its name must \
             start with SIGNEX_) instead"
        );
        true
    }
}
