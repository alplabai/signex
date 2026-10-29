//! The Place Component picker's list, built once per change instead of once
//! per frame (#1374).
//!
//! `view` must stay cheap and pure, so it only reads this cache. `update`
//! keeps it current through [`LibraryState::sync_view_caches`]: the entries
//! (with their lowercase search keys) are rebuilt only when the set of
//! components changes, and the filtered list only when the query changes.

use super::*;
use crate::library::range_filter::{FilterColumn, RangeFilters, retain_in_range};

/// One component of an open library, with what the picker draws and
/// matches on computed once.
#[derive(Debug, Clone)]
pub struct PickerEntry {
    /// The `.snxlib` the component belongs to; shared by its siblings.
    pub path: Arc<Path>,
    pub summary: ComponentSummary,
    /// `{:?}` of the lifecycle state, as the row shows it.
    pub state_label: String,
    /// The row's parameters, for the range filter. Empty when the library's
    /// tables do not hold the row.
    parameters: signex_library::ParamMap,
    internal_pn_key: String,
    mpn_key: String,
    description_key: String,
}

impl PickerEntry {
    fn new(
        path: &Arc<Path>,
        summary: &ComponentSummary,
        parameters: signex_library::ParamMap,
    ) -> Self {
        Self {
            parameters,
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
    /// The range filters `visible` was filtered with.
    range: Vec<(String, signex_library::RangeFilter)>,
    visible: Vec<usize>,
    /// The parameter columns a range filter can go on; built with
    /// `entries`.
    filter_columns: Vec<FilterColumn>,
    /// Entries each range filter excluded for having no comparable value.
    excluded: std::collections::BTreeMap<String, usize>,
}

impl PickerRows {
    /// The rows matching the current query, in library order.
    pub fn visible(&self) -> impl ExactSizeIterator<Item = &PickerEntry> {
        self.visible.iter().map(|&i| &self.entries[i])
    }

    /// The parameter columns a range filter can go on.
    pub(crate) fn filter_columns(&self) -> &[FilterColumn] {
        &self.filter_columns
    }

    /// How many entries each range filter excluded because their cell is
    /// not a measurement in the column's unit.
    pub fn range_excluded(&self) -> &std::collections::BTreeMap<String, usize> {
        &self.excluded
    }

    /// No row matches.
    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    /// Bring the rows up to date with `libraries` at `data_version` and the
    /// picker's `filter`, doing only the work that input change needs.
    pub(super) fn sync(
        &mut self,
        libraries: &[OpenLibrary],
        data_version: u64,
        filter: &str,
        range_filters: &RangeFilters,
    ) {
        let needle = filter.trim().to_lowercase();
        let range = range_filters.active();
        let entries_stale = self.entries_at != Some(data_version);
        if entries_stale {
            self.entries = build_entries(libraries);
            self.filter_columns = picker_filter_columns(&self.entries);
            self.entries_at = Some(data_version);
            #[cfg(test)]
            counters::ENTRY_BUILDS.with(|c| c.set(c.get() + 1));
        }
        if entries_stale || self.needle.as_deref() != Some(needle.as_str()) || self.range != range {
            let mut visible: Vec<usize> = (0..self.entries.len())
                .filter(|&i| self.entries[i].matches(&needle))
                .collect();
            self.excluded = retain_in_range(&mut visible, &range, |&i, key| {
                self.entries[i].parameters.get(key)
            });
            self.visible = visible;
            self.needle = Some(needle);
            self.range = range;
            #[cfg(test)]
            counters::FILTER_PASSES.with(|c| c.set(c.get() + 1));
        }
    }
}

fn build_entries(libraries: &[OpenLibrary]) -> Vec<PickerEntry> {
    let mut entries = Vec::new();
    for lib in libraries {
        let path: Arc<Path> = Arc::from(lib.root.as_path());
        let by_id: HashMap<Uuid, &signex_library::ComponentRow> = lib
            .tables
            .values()
            .flatten()
            .map(|r| (r.row_id, r))
            .collect();
        entries.extend(lib.cached_components.iter().map(|summary| {
            let parameters = by_id
                .get(&summary.row_id)
                .map(|r| r.parameters.clone())
                .unwrap_or_default();
            PickerEntry::new(&path, summary, parameters)
        }));
    }
    entries
}

/// The parameter keys that hold measurements across the picker's
/// entries, each with the unit most of its cells are in.
fn picker_filter_columns(entries: &[PickerEntry]) -> Vec<FilterColumn> {
    let keys: std::collections::BTreeSet<&String> =
        entries.iter().flat_map(|e| e.parameters.keys()).collect();
    keys.into_iter()
        .filter_map(|key| {
            let unit =
                signex_library::column_unit(entries.iter().filter_map(|e| e.parameters.get(key)))?;
            Some(FilterColumn {
                key: key.clone(),
                label: key.clone(),
                unit,
            })
        })
        .collect()
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
            picker.rows.sync(
                &self.open_libraries,
                version,
                &picker.filter,
                &picker.range_filters,
            );
        }
        self.sync_panel_rows();
        self.sync_browser_caches();
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
