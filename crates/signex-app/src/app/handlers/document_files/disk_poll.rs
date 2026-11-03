//! Idle poll for open documents changed on disk (#1877).
//!
//! The save-time check (#1826) finds an outside change only when the user
//! saves. This is the earlier warning: each poll tick re-reads the files of
//! open schematic and board tabs off the UI thread, and a file that no longer
//! holds what the document knew raises the same card a save would, once per
//! change. Files with a write in flight are not read: the save ledger holds
//! them, and their own completion records the new baseline.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use iced::Task;

use super::save::file_label;
use crate::app::state::ErrorNotice;
use crate::app::{FileMsg, Message, Signex, TabKind};
use crate::persist::{ContentHash, DiskDrift};

/// What one read of a document's file found: its fingerprint, or why there is none.
pub type DiskObservation = Result<ContentHash, DiskDrift>;

/// Length and modification time of a file when it was last read in full.
pub type FileStamp = (u64, SystemTime);

/// One polled file: what it held, and its stamp when that read can be reused.
pub type DiskRead = (PathBuf, DiskObservation, Option<FileStamp>);

/// A file modified more recently than this may still change within the
/// timestamp's resolution, so its stamp is not trusted.
const RACY_WINDOW: Duration = Duration::from_secs(2);

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
    /// Stamp and hash at each path's last full read: a file that still looks
    /// the same is not read again.
    stamps: HashMap<PathBuf, (FileStamp, ContentHash)>,
    /// Paths whose Reload read is running; the file differs from the baseline
    /// until it lands, which is not a new change.
    reloading: HashSet<PathBuf>,
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

    /// A Reload read of `path` began.
    pub(crate) fn begin_reload(&mut self, path: PathBuf) {
        self.reloading.insert(path);
    }

    /// The Reload read of `path` finished, whatever its outcome.
    pub(crate) fn end_reload(&mut self, path: &std::path::Path) {
        self.reloading.remove(path);
    }
}

/// Blocking half; run it on a worker. A file whose stamp matches the cached
/// one keeps its cached hash; the hash, not the stamp, decides what changed.
fn read_all(paths: Vec<(PathBuf, Option<(FileStamp, ContentHash)>)>) -> Vec<DiskRead> {
    paths
        .into_iter()
        .filter_map(|(path, cached)| {
            let stamp = std::fs::metadata(&path)
                .ok()
                .and_then(|m| Some((m.len(), m.modified().ok()?)));
            if let (Some(now), Some((old, hash))) = (stamp, cached)
                && now == old
            {
                return Some((path, Ok(hash), Some(now)));
            }
            let seen = match std::fs::read(&path) {
                Ok(bytes) => Ok(ContentHash::of(&bytes)),
                // Absence is told apart from a gone folder the way a save does.
                Err(_) => match crate::persist::drift_from(&path, &[]) {
                    Ok(Some(gone @ (DiskDrift::Missing | DiskDrift::FolderMissing))) => Err(gone),
                    // Unreadable for another reason: say nothing.
                    _ => return None,
                },
            };
            let reusable = stamp.filter(|(_, mtime)| {
                seen.is_ok() && mtime.elapsed().is_ok_and(|age| age > RACY_WINDOW)
            });
            Some((path, seen, reusable))
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
            .filter(|tab| !d.disk_poll.reloading.contains(&tab.path))
            .filter(|tab| !d.pending_restores.contains(&tab.path))
            .map(|tab| tab.path.clone())
            .collect();
        if paths.is_empty() {
            return Task::none();
        }
        let d = &mut self.document_state;
        let work: Vec<_> = paths
            .iter()
            .map(|p| (p.clone(), d.disk_poll.stamps.get(p).copied()))
            .collect();
        d.disk_poll.started_with = paths
            .iter()
            .filter_map(|p| Some((p.clone(), d.document_saves.baseline(p)?)))
            .collect();
        d.disk_poll.in_flight = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_all(work))
                    .await
                    .unwrap_or_default()
            },
            |seen| Message::File(FileMsg::DocumentDiskPolled(seen)),
        )
    }

    pub(crate) fn handle_document_disk_polled(&mut self, seen: Vec<DiskRead>) -> Task<Message> {
        self.document_state.disk_poll.in_flight = false;
        let started_with = std::mem::take(&mut self.document_state.disk_poll.started_with);
        for (path, observed, stamp) in seen {
            let d = &mut self.document_state;
            match (stamp, observed) {
                (Some(stamp), Ok(hash)) => d.disk_poll.stamps.insert(path.clone(), (stamp, hash)),
                _ => d.disk_poll.stamps.remove(&path),
            };
            let open = d.tabs.iter().any(|tab| tab.path == path);
            // A write begun since the read owns the file's state now.
            if !open
                || d.document_saves.is_saving(&path)
                || d.disk_poll.reloading.contains(&path)
                || d.pending_restores.contains(&path)
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

    #[test]
    fn an_old_unchanged_file_is_not_read_again_but_a_new_one_is() {
        let dir = std::env::temp_dir().join(format!("snx-poll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.snxsch");
        std::fs::write(&path, b"one").unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        let stamp = (meta.len(), meta.modified().unwrap());
        // A cached hash that does not match the bytes proves the gate was used.
        let fake = ContentHash::of(b"cached");
        let out = read_all(vec![(path.clone(), Some((stamp, fake)))]);
        assert_eq!(out[0].1, Ok(fake));
        // A different stamp forces a real read.
        let stale = (stamp.0 + 1, stamp.1);
        let out = read_all(vec![(path, Some((stale, fake)))]);
        assert_eq!(out[0].1, Ok(ContentHash::of(b"one")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
