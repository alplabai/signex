//! What the app keeps about document ids besides the documents themselves
//! (#2172); `app::document_ids` reads, assigns and lists them.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::document_id::DocumentId;

use crate::app::core::document_key::DocumentKey;

/// One cached head: the id read and the file's modification time then.
#[derive(Debug, Clone, Copy)]
struct HeaderRecord {
    id: Option<DocumentId>,
    modified: Option<std::time::SystemTime>,
}

/// What the app keeps about document ids besides the documents themselves.
///
/// A loaded document carries its own id, so this holds only what a document
/// cannot say: the ids of project files that are not loaded (read from their
/// headers in the background), and the order documents were first seen in,
/// which decides who keeps an id two documents claim.
#[derive(Debug, Default)]
pub struct DocumentIds {
    /// The id at the head of each project-referenced file that is not
    /// loaded; `None` when the file has none or could not be read.
    headers: HashMap<PathBuf, HeaderRecord>,
    seen: HashMap<DocumentKey, u64>,
    next_seen: u64,
    /// Sheets a headless query loaded only to answer and drops afterwards:
    /// they are listed as not loaded and carry no revision.
    transient: std::collections::HashSet<PathBuf>,
}

impl DocumentIds {
    pub(crate) fn is_transient(&self, path: &std::path::Path) -> bool {
        self.transient.contains(path)
    }

    pub(crate) fn begin_transient(&mut self, path: PathBuf) {
        self.transient.insert(path);
    }

    pub(crate) fn end_transient(&mut self, path: &std::path::Path) {
        self.transient.remove(path);
    }

    pub(crate) fn header(&self, path: &std::path::Path) -> Option<Option<DocumentId>> {
        self.headers.get(path).map(|record| record.id)
    }

    /// Whether `path` needs its head read: never read, read and holding no
    /// id, or `modified_now` (asked only then) differs from the time the head
    /// was read at. An `Err` time keeps the head.
    pub(crate) fn header_wanted_given(
        &self,
        path: &std::path::Path,
        modified_now: impl FnOnce() -> std::io::Result<std::time::SystemTime>,
    ) -> bool {
        self.headers.get(path).is_none_or(|record| {
            record.id.is_none() || modified_now().is_ok_and(|now| record.modified != Some(now))
        })
    }

    pub(crate) fn set_header(
        &mut self,
        path: PathBuf,
        id: Option<DocumentId>,
        modified: Option<std::time::SystemTime>,
    ) {
        self.headers.insert(path, HeaderRecord { id, modified });
    }

    /// The cached head of `path` no longer describes the file (it was
    /// written, its tab closed, or it changed on disk): read it again.
    pub(crate) fn forget_header(&mut self, path: &std::path::Path) {
        self.headers.remove(path);
    }

    /// Forget the heads of exactly these files (their project closed).
    pub(crate) fn forget_headers_of<'a>(
        &mut self,
        paths: impl IntoIterator<Item = &'a std::path::Path>,
    ) {
        for path in paths {
            self.headers.remove(path);
        }
    }

    /// Forget every head whose file's modification time `modified_of` knows
    /// and finds different from the one the head was read at.
    pub(crate) fn forget_changed_headers(
        &mut self,
        modified_of: impl Fn(&std::path::Path) -> Option<std::time::SystemTime>,
    ) {
        self.headers.retain(|path, record| match modified_of(path) {
            Some(now) => record.modified == Some(now),
            None => true,
        });
    }

    /// Forget the first-seen order of every document not in `held`: one that
    /// was closed has no claim on an id, so a copy opened later does not
    /// outrank the document that holds it now.
    pub(crate) fn forget_unheld_orders<'a>(&mut self, held: impl Iterator<Item = &'a DocumentKey>) {
        let held: std::collections::HashSet<&DocumentKey> = held.collect();
        self.seen.retain(|key, _| held.contains(key));
    }

    /// The first-seen order of `key`, assigned on first sight.
    pub(crate) fn seen_order(&mut self, key: &DocumentKey) -> u64 {
        if let Some(order) = self.seen.get(key) {
            return *order;
        }
        self.next_seen += 1;
        self.seen.insert(key.clone(), self.next_seen);
        self.next_seen
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use super::*;

    fn never_asked() -> std::io::Result<SystemTime> {
        panic!("the modification time must not be read")
    }

    #[test]
    fn a_head_never_read_or_without_an_id_is_wanted_without_a_stat() {
        let path = Path::new("/p/a.snxsch");
        let mut ids = DocumentIds::default();
        assert!(ids.header_wanted_given(path, never_asked));
        ids.set_header(path.to_path_buf(), None, Some(SystemTime::UNIX_EPOCH));
        assert!(ids.header_wanted_given(path, never_asked));
    }

    #[test]
    fn a_head_with_an_id_is_wanted_only_when_its_file_changed() {
        let path = Path::new("/p/a.snxsch");
        let read_at = SystemTime::UNIX_EPOCH;
        let mut ids = DocumentIds::default();
        ids.set_header(path.to_path_buf(), Some(DocumentId::new()), Some(read_at));
        assert!(!ids.header_wanted_given(path, || Ok(read_at)));
        assert!(ids.header_wanted_given(path, || Ok(read_at + Duration::from_secs(1))));
        assert!(!ids.header_wanted_given(path, || Err(std::io::Error::other("no stat"))));
    }
}
