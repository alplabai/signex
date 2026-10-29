//! One document per file: a file opened under another spelling of a path the
//! app already holds reuses that document (#1149).
//!
//! `Lib.snxsch` and `lib.snxsch` are one file on a case-insensitive volume
//! (and `é` as NFC / NFD is one name on APFS). Two documents for it can both
//! end clean after Ctrl+S wrote different bytes, so the second one is never
//! created: every open route first swaps the requested path for the spelling
//! the existing document holds, and its exact-path lookups then do the rest.
//!
//! The comparison is [`path_key_with`] under [`Signex::identity_fold_for`], so
//! it never touches the disk: the volume rule comes from a probe that ran off
//! the update thread, and folds until it answers.

use std::path::{Path, PathBuf};

use iced::Task;

use super::super::super::*;
use crate::app::state::scope::path_key_with;

impl Signex {
    /// The spelling a document, an in-flight open or a parked dirty engine
    /// already holds for the file `path` names, `path` itself when that is
    /// the held spelling, `None` when no document holds the file.
    pub(crate) fn held_spelling(&self, path: &Path) -> Option<PathBuf> {
        let fold = self.identity_fold_for(path);
        let key = path_key_with(path, fold);
        let document_state = &self.document_state;
        let parked_dirty = document_state
            .engines
            .keys()
            .filter(|held| document_state.is_document_dirty(held));
        document_state
            .tabs
            .iter()
            .map(|tab| &tab.path)
            .chain(document_state.pending_opens.paths())
            // A restore still running (even a cancelled one) holds its file's
            // spelling too, so a reopen under another spelling meets the same
            // save refusal.
            .chain(document_state.pending_restores.paths())
            .chain(parked_dirty)
            .find(|held| path_key_with(held, fold) == key)
            .cloned()
    }

    /// `path` re-spelled as the document that already holds the file, or
    /// unchanged when none does. The first call of every open route.
    pub(crate) fn resolve_held_spelling(&self, path: PathBuf) -> PathBuf {
        self.held_spelling(&path).unwrap_or(path)
    }

    /// Settle an open request against the documents already held. A path that
    /// collides with a held spelling only under the stand-in fold, in a
    /// directory whose rule is still unknown, cannot be decided yet: it is
    /// parked (and marked in flight so a repeat open is blocked) until the
    /// probe answers, then opened again by `handle_dir_case_probed`.
    pub(crate) fn gate_open(&mut self, path: PathBuf) -> OpenGate {
        let probe = self.probe_dir_case_if_unknown(&path);
        if self.document_state.dir_case.is_deferred_open(&path) {
            return OpenGate::Deferred(probe);
        }
        let Some(held) = self.held_spelling(&path) else {
            return OpenGate::Proceed { path, probe };
        };
        if held == path || self.identity_answer(&path).is_some() {
            return OpenGate::Proceed { path: held, probe };
        }
        self.document_state.pending_opens.insert(path.clone());
        self.document_state.dir_case.defer_open(path);
        OpenGate::Deferred(probe)
    }

    /// [`Self::resolve_held_spelling`] for a route that cannot wait for the
    /// probe: while the directory's rule is unknown it folds, and says so once.
    pub(crate) fn resolve_held_spelling_now(&mut self, path: PathBuf) -> PathBuf {
        let held = self.resolve_held_spelling(path.clone());
        if held != path
            && self.identity_answer(&path).is_none()
            && let Some(dir) = path.parent()
            && self.document_state.dir_case.note_assumed_once(dir)
        {
            crate::diagnostics::log_info(assumed_fold_message(&path, &held));
        }
        held
    }
}

/// What an open request does about the documents already held (#1447).
pub(crate) enum OpenGate {
    /// Open `path` (re-spelled as the held document), and run `probe`.
    Proceed { path: PathBuf, probe: Task<Message> },
    /// Held back until its directory answers; run `probe`.
    Deferred(Task<Message>),
}

/// The advice leads; the paths come last because Messages clips the line.
pub(super) fn assumed_fold_message(path: &Path, held: &Path) -> String {
    format!(
        "Folder case rule not known yet, so these are treated as one file: {} is {}",
        path.display(),
        held.display()
    )
}
