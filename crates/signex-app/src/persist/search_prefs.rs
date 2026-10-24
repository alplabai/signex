//! Debounced persistence of the search boxes (#859): the Components-panel
//! filter and the per-library Library Browser queries.
//!
//! Typing used to rewrite `prefs.json` inside `update` on every keystroke.
//! The in-memory state still changes immediately; only the disk write is
//! debounced and moved off the UI thread, exactly like the dock layout.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::debounced::{DebouncedStore, DebouncedSync, PendingWrite, WriteBarrier};

/// How long the search text must stay still before it is written.
pub const SEARCH_PREFS_SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// What this session changed in the search boxes, applied key by key onto
/// whatever `prefs.json` holds at write time. It is a delta on purpose:
/// replacing the whole map from an in-memory copy would erase entries this
/// session never touched (another library's query, a filter typed by a
/// second instance, or anything lost to an unreadable file at boot).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchPrefsChange {
    /// The Components filter, `Some` only once this session changed it.
    pub component_filter: Option<String>,
    /// Touched library queries; an empty query clears that library's
    /// on-disk entry.
    pub library_searches: BTreeMap<PathBuf, String>,
    /// Bumped on every observation so two identical deltas are never
    /// mistaken for "already written".
    pub revision: u64,
}

/// One search-prefs write to run off the UI thread.
pub type SearchPrefsWrite = PendingWrite<SearchPrefsChange>;

/// Messages of the search-prefs persistence loop. Namespaced under
/// `Message::SearchPrefs`.
#[derive(Debug, Clone)]
pub enum SearchPrefsMsg {
    /// The quiet period for generation `.0` ran out. Stale generations
    /// (the text changed again meanwhile) are ignored.
    SaveDebounceElapsed(u64),
    /// A background write finished; the error text is what the Messages
    /// panel shows.
    Saved(Result<(), String>),
}

/// The changes made this session plus the debounce that persists them.
#[derive(Debug)]
pub struct SearchPrefsSync {
    sync: DebouncedSync<SearchPrefsChange>,
    changes: SearchPrefsChange,
}

impl SearchPrefsSync {
    pub fn new(store: Arc<dyn DebouncedStore<SearchPrefsChange>>) -> Self {
        Self {
            sync: DebouncedSync::new(store, SearchPrefsChange::default()),
            changes: SearchPrefsChange::default(),
        }
    }

    /// The query typed for `library` this session, if any (`Some("")`
    /// means cleared). `None` means the caller should consult the disk.
    pub fn library_search(&self, library: &Path) -> Option<String> {
        self.changes.library_searches.get(library).cloned()
    }

    /// Record the Components-panel filter. Returns the generation to arm
    /// a debounce timer for.
    pub fn set_component_filter(&mut self, filter: &str) -> Option<u64> {
        self.changes.component_filter = Some(filter.to_string());
        self.observe()
    }

    /// Record one library's browser query (empty clears its entry).
    pub fn set_library_search(&mut self, library: &Path, query: &str) -> Option<u64> {
        self.changes
            .library_searches
            .insert(library.to_path_buf(), query.to_string());
        self.observe()
    }

    fn observe(&mut self) -> Option<u64> {
        self.changes.revision += 1;
        self.sync.observe(self.changes.clone())
    }

    pub fn take_if_current(&mut self, generation: u64) -> Option<SearchPrefsWrite> {
        self.sync.take_if_current(generation)
    }

    pub fn flush(&mut self) -> Option<SearchPrefsWrite> {
        self.sync.flush()
    }

    pub fn barrier(&self) -> WriteBarrier {
        self.sync.barrier()
    }

    /// A write failed. Typed text must not be dropped, so the changes are
    /// queued again and the next flush (quit) or edit retries them.
    pub fn write_failed(&mut self) {
        self.sync.write_failed_retaining(self.changes.clone());
    }
}
