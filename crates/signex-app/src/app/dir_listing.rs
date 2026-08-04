//! Directory listing that tells "not there" apart from "could not be read".
//!
//! `std::fs::read_dir(dir).ok()` collapses a permission error, a broken
//! mount and a stale network path into the same empty listing an absent
//! directory produces. Every caller of that shape in the app feeds a
//! picker or a tree, so the failure renders as "there is nothing here"
//! — the user is told their standard libraries, their symbols or their
//! sibling footprints do not exist while they are sitting on disk.
//!
//! `NotFound` really does mean empty (it is the normal first-run state)
//! and stays silent. Everything else reaches the Messages panel.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// List the immediate children of `dir`, reporting a real failure.
///
/// `context` names the caller so the record identifies which listing
/// went missing (`"standard symbol libraries"`, `"library symbols"`, …).
/// An unreadable directory is reported once per session and then read as
/// empty, so the caller can carry on; individual unreadable entries are
/// skipped with a warn.
///
/// Entry order is whatever the filesystem hands back — callers that
/// render a list sort it themselves.
pub(in crate::app) fn list_dir_or_report(dir: &Path, context: &str) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // The directory not existing is the normal first-run state:
        // no library installed yet, no `symbols/` created yet.
        Err(error) if error.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            if claim_first_report(dir) {
                tracing::error!(
                    target: "signex::fs",
                    error = %error,
                    dir = %dir.display(),
                    context = context,
                    "directory unreadable; listed as empty"
                );
            }
            return Vec::new();
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => tracing::warn!(
                target: "signex::fs",
                error = %error,
                dir = %dir.display(),
                context = context,
                "directory entry unreadable; skipped"
            ),
        }
    }
    paths
}

/// True the first time `dir` fails this session.
///
/// The hot callers rebuild a whole panel context: `refresh_panel_ctx`
/// alone has 169 call sites, 41 of them in the pad editor, and the
/// footprint context is rebuilt on the same cadence. Reporting per call
/// would push the user's ERC results out of the 200-entry Messages ring
/// within a few keystrokes, so a given directory is reported once and
/// the repeats are suppressed. A listing failure of this kind is a
/// standing condition, not an event — one record is the whole story.
fn claim_first_report(dir: &Path) -> bool {
    static REPORTED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    REPORTED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map(|mut reported| reported.insert(dir.to_path_buf()))
        // A poisoned mutex means some other thread panicked mid-insert.
        // Reporting again is the safe side of that coin.
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique directory name so the shared diagnostics ring can be
    /// asserted on while other tests write to it. Kept short: the panel
    /// compacts a record to 160 characters and the path has to survive.
    fn unique_dir(tag: &str) -> PathBuf {
        let id = uuid::Uuid::new_v4().simple().to_string();
        std::env::temp_dir().join(format!("snx-{tag}-{}", &id[..8]))
    }

    fn records_mentioning(marker: &str) -> Vec<crate::diagnostics::DiagnosticEntry> {
        let _ = crate::diagnostics::init_logging();
        crate::diagnostics::recent_entries()
            .into_iter()
            .filter(|entry| entry.message.contains(marker))
            .collect()
    }

    #[test]
    fn a_missing_directory_lists_empty_and_says_nothing() {
        // First run: no library installed, no `symbols/` yet. Empty is
        // the right answer and a message here would be noise.
        let dir = unique_dir("absent");
        let marker = dir.file_name().unwrap().to_string_lossy().to_string();
        let _ = crate::diagnostics::init_logging();

        let paths = list_dir_or_report(&dir, "test: absent directory");

        assert!(paths.is_empty(), "a missing directory has no children");
        assert!(
            records_mentioning(&marker).is_empty(),
            "NotFound is the normal first-run state and must stay silent"
        );
    }

    #[test]
    fn an_unreadable_directory_reaches_the_messages_panel() {
        // A path that exists but is not a directory fails with
        // `NotADirectory` — the same non-`NotFound` shape a permission
        // error takes, and the one this helper exists to surface.
        let file = unique_dir("unreadable");
        let marker = file.file_name().unwrap().to_string_lossy().to_string();
        std::fs::write(&file, b"not a directory").expect("write probe file");
        let _ = crate::diagnostics::init_logging();

        let paths = list_dir_or_report(&file, "test: unreadable directory");

        assert!(paths.is_empty(), "an unreadable directory reads as empty");
        let records = records_mentioning(&marker);
        assert_eq!(
            records.len(),
            1,
            "the failure must reach the Messages panel, exactly once"
        );
        assert_eq!(
            records[0].level,
            crate::diagnostics::DiagnosticLevel::Error,
            "an unreadable directory withholds content that is on disk"
        );
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn a_standing_failure_does_not_flood_the_messages_panel() {
        // The panel-context builders re-list on every rebuild. Without
        // the once-per-directory guard, one unreadable `symbols/` dir
        // evicts the user's ERC results from the 200-entry ring.
        let file = unique_dir("flood");
        let marker = file.file_name().unwrap().to_string_lossy().to_string();
        std::fs::write(&file, b"not a directory").expect("write probe file");
        let _ = crate::diagnostics::init_logging();

        for _ in 0..25 {
            let paths = list_dir_or_report(&file, "test: repeated listing");
            assert!(paths.is_empty());
        }

        assert_eq!(
            records_mentioning(&marker).len(),
            1,
            "a standing failure is reported once, not once per rebuild"
        );
        std::fs::remove_file(&file).ok();
    }
}
