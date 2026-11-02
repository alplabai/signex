//! Idle poll for open documents changed on disk (#1877).
//!
//! The save-time check (#1826) finds an outside change only when the user
//! saves. This is the earlier warning: each poll tick re-reads the files of
//! open schematic and board tabs off the UI thread, and a file that no longer
//! holds what the document knew raises the same card a save would, once per
//! change. Files with a write in flight are not read: the save ledger holds
//! them, and their own completion records the new baseline.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::Task;

use super::save::file_label;
use crate::app::state::ErrorNotice;
use crate::app::{FileMsg, Message, Signex, TabKind};
use crate::persist::{ContentHash, DiskDrift};

/// What one read of a document's file found: its fingerprint, or why there is none.
pub type DiskObservation = Result<ContentHash, DiskDrift>;

/// Poll bookkeeping, owned by `DocumentState`.
#[derive(Default)]
pub struct DiskPoll {
    in_flight: bool,
    /// Baselines as the running pass started; an observation whose baseline
    /// moved since (a save landed meanwhile) is stale and dropped.
    started_with: HashMap<PathBuf, ContentHash>,
    /// The change already reported per path; a poll that sees the same file
    /// again stays quiet.
    reported: HashMap<PathBuf, DiskObservation>,
}

impl DiskPoll {
    /// Fold one observation in; true when it is a change not yet reported.
    /// `known` is whether the document knew that state of the file.
    fn observe(&mut self, path: PathBuf, seen: DiskObservation, known: bool) -> bool {
        if known {
            self.reported.remove(&path);
            return false;
        }
        self.reported.insert(path, seen) != Some(seen)
    }
}

/// Blocking half; run it on a worker.
fn read_all(paths: Vec<PathBuf>) -> Vec<(PathBuf, DiskObservation)> {
    paths
        .into_iter()
        .filter_map(|path| {
            let seen = match std::fs::read(&path) {
                Ok(bytes) => Ok(ContentHash::of(&bytes)),
                // Absence is told apart from a gone folder the way a save does.
                Err(_) => match crate::persist::drift_from(&path, &[]) {
                    Ok(Some(gone @ (DiskDrift::Missing | DiskDrift::FolderMissing))) => Err(gone),
                    // Unreadable for another reason: say nothing.
                    _ => return None,
                },
            };
            Some((path, seen))
        })
        .collect()
}

impl Signex {
    /// Poll tick: re-read the open documents' files, unless the last pass is
    /// still running.
    pub(crate) fn start_document_disk_poll(&mut self) -> Task<Message> {
        let d = &self.document_state;
        if d.disk_poll.in_flight {
            return Task::none();
        }
        let paths: Vec<PathBuf> = d
            .tabs
            .iter()
            .filter(|tab| matches!(tab.kind, TabKind::Schematic | TabKind::Pcb))
            .filter(|tab| d.document_saves.baseline(&tab.path).is_some())
            .filter(|tab| !d.document_saves.is_saving(&tab.path))
            .map(|tab| tab.path.clone())
            .collect();
        if paths.is_empty() {
            return Task::none();
        }
        let d = &mut self.document_state;
        d.disk_poll.started_with = paths
            .iter()
            .filter_map(|p| Some((p.clone(), d.document_saves.baseline(p)?)))
            .collect();
        d.disk_poll.in_flight = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_all(paths))
                    .await
                    .unwrap_or_default()
            },
            |seen| Message::File(FileMsg::DocumentDiskPolled(seen)),
        )
    }

    pub(crate) fn handle_document_disk_polled(
        &mut self,
        seen: Vec<(PathBuf, DiskObservation)>,
    ) -> Task<Message> {
        self.document_state.disk_poll.in_flight = false;
        let started_with = std::mem::take(&mut self.document_state.disk_poll.started_with);
        for (path, observed) in seen {
            let d = &mut self.document_state;
            let open = d.tabs.iter().any(|tab| tab.path == path);
            // A write begun since the read owns the file's state now.
            if !open
                || d.document_saves.is_saving(&path)
                || started_with.get(&path).copied() != d.document_saves.baseline(&path)
            {
                continue;
            }
            let known = match observed {
                Ok(hash) => d.document_saves.knows(&path, hash),
                Err(_) => false,
            };
            // A card already up keeps the slot; leave the change unreported so
            // a later tick raises it once the slot is free.
            if !known && d.error_notice.is_some() {
                continue;
            }
            if !d.disk_poll.observe(path.clone(), observed, known) {
                continue;
            }
            let drift = observed.err().unwrap_or(DiskDrift::Changed);
            let name = file_label(&path);
            crate::diagnostics::log_warning(format!(
                "[disk] {name} {}; nothing was changed in the open document",
                drift.describe()
            ));
            self.document_state.error_notice = Some(ErrorNotice::disk_changed(path, drift));
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_reported_once_and_again_only_when_the_file_changes_again() {
        let mut poll = DiskPoll::default();
        let path = PathBuf::from("a.snxsch");
        let one = Ok(ContentHash::of(b"one"));
        let two = Ok(ContentHash::of(b"two"));
        assert!(poll.observe(path.clone(), one, false));
        assert!(!poll.observe(path.clone(), one, false));
        assert!(poll.observe(path.clone(), two, false));
        // Back to a state the document knows (reload / save): re-armed.
        assert!(!poll.observe(path.clone(), one, true));
        assert!(poll.observe(path, one, false));
    }
}
