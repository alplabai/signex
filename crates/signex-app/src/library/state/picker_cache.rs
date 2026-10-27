//! The Place Component picker's list, built once per change instead of once
//! per frame (#1374).
//!
//! `view` must stay cheap and pure, so it only reads this cache. `update`
//! keeps it current through [`LibraryState::sync_view_caches`]: the entries
//! (with their lowercase search keys) are rebuilt only when the set of
//! components changes, and the filtered list only when the query changes.

use super::*;

/// One component of an open library, with what the picker draws and
/// matches on computed once.
#[derive(Debug, Clone)]
pub struct PickerEntry {
    /// The `.snxlib` the component belongs to; shared by its siblings.
    pub path: Arc<Path>,
    pub summary: ComponentSummary,
    /// `{:?}` of the lifecycle state, as the row shows it.
    pub state_label: String,
    internal_pn_key: String,
    mpn_key: String,
    description_key: String,
}

impl PickerEntry {
    fn new(path: &Arc<Path>, summary: &ComponentSummary) -> Self {
        Self {
            path: Arc::clone(path),
            summary: summary.clone(),
            state_label: format!("{:?}", summary.state),
            internal_pn_key: summary.internal_pn.as_str().to_lowercase(),
            mpn_key: summary.mpn.to_lowercase(),
            description_key: summary.description.to_lowercase(),
        }
    }

    fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || self.internal_pn_key.contains(needle)
            || self.mpn_key.contains(needle)
            || self.description_key.contains(needle)
    }
}

/// The picker's rows, with the inputs they were built from.
#[derive(Debug, Clone, Default)]
pub struct PickerRows {
    /// `LibraryState::data_version` the entries were built at.
    entries_at: Option<u64>,
    entries: Vec<PickerEntry>,
    /// The normalised query `visible` was filtered for.
    needle: Option<String>,
    visible: Vec<usize>,
}

impl PickerRows {
    /// The rows matching the current query, in library order.
    pub fn visible(&self) -> impl ExactSizeIterator<Item = &PickerEntry> {
        self.visible.iter().map(|&i| &self.entries[i])
    }

    /// No row matches.
    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    /// Bring the rows up to date with `libraries` at `data_version` and the
    /// picker's `filter`, doing only the work that input change needs.
    pub(super) fn sync(&mut self, libraries: &[OpenLibrary], data_version: u64, filter: &str) {
        let needle = filter.trim().to_lowercase();
        let entries_stale = self.entries_at != Some(data_version);
        if entries_stale {
            self.entries = build_entries(libraries);
            self.entries_at = Some(data_version);
            #[cfg(test)]
            counters::ENTRY_BUILDS.with(|c| c.set(c.get() + 1));
        }
        if entries_stale || self.needle.as_deref() != Some(needle.as_str()) {
            self.visible = (0..self.entries.len())
                .filter(|&i| self.entries[i].matches(&needle))
                .collect();
            self.needle = Some(needle);
            #[cfg(test)]
            counters::FILTER_PASSES.with(|c| c.set(c.get() + 1));
        }
    }
}

fn build_entries(libraries: &[OpenLibrary]) -> Vec<PickerEntry> {
    let mut entries = Vec::new();
    for lib in libraries {
        let path: Arc<Path> = Arc::from(lib.root.as_path());
        entries.extend(
            lib.cached_components
                .iter()
                .map(|summary| PickerEntry::new(&path, summary)),
        );
    }
    entries
}

impl LibraryState {
    /// Record that an open library's rows, prices or set of libraries
    /// changed, so caches derived from them rebuild at the next
    /// [`Self::sync_view_caches`].
    pub fn touch_data(&mut self) {
        self.data_version += 1;
    }

    /// Bring the caches `view` reads up to date. Called from `update`
    /// (`finish_update`, and the picker handlers); cheap when nothing
    /// they depend on changed.
    pub fn sync_view_caches(&mut self) {
        let version = self.data_version;
        if let Some(picker) = self.picker.as_mut() {
            picker
                .rows
                .sync(&self.open_libraries, version, &picker.filter);
        }
    }
}

/// Rebuild counts, so a test can tell "recomputed" from "reused".
#[cfg(test)]
pub(crate) mod counters {
    use std::cell::Cell;

    thread_local! {
        pub static ENTRY_BUILDS: Cell<usize> = const { Cell::new(0) };
        pub static FILTER_PASSES: Cell<usize> = const { Cell::new(0) };
    }
}
