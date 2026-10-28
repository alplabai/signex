//! What the grid derives from a table, computed once per change (#1374).
//!
//! Filtering the rows, deriving the columns and sorting are all pure
//! functions of the cached table, the search/lifecycle/class filters, the
//! sort selection and the price clock. `update` keeps a [`BrowserViewCache`]
//! on the browser tab through `LibraryState::sync_view_caches`; `view`
//! reuses it while its key matches, and only if it does not (a message path
//! that skipped the sync) recomputes for that one frame.

use chrono::{DateTime, Utc};

use super::columns::{ColumnKind, GridColumn, derive_columns, row_matches_filter};
use super::price::PriceView;
use super::sort::{CellSortKey, sort_by_cell_key};
use super::*;
use crate::library::state::{BrowserSort, OpenLibrary};

/// The inputs a cache was built from.
#[derive(Debug, Clone)]
struct ViewKey {
    data_version: u64,
    clock: DateTime<Utc>,
    table: String,
    search: String,
    lifecycle_filter: LifecycleFilter,
    class_filter: Option<String>,
    sort_by: Option<BrowserSort>,
}

impl ViewKey {
    fn matches(
        &self,
        data_version: u64,
        clock: DateTime<Utc>,
        table: &str,
        browser: &LibraryBrowserState,
    ) -> bool {
        self.data_version == data_version
            && self.clock == clock
            && self.table == table
            && self.search == browser.search
            && self.lifecycle_filter == browser.lifecycle_filter
            && self.class_filter == browser.class_filter
            && self.sort_by == browser.sort_by
    }
}

/// The visible rows (as indices into the table, in display order), the
/// derived columns and the price column's main currency.
#[derive(Debug, Clone)]
pub struct BrowserViewCache {
    key: ViewKey,
    pub(super) visible: Vec<usize>,
    pub(super) columns: Vec<GridColumn>,
    pub(super) main_currency: String,
}

/// The table the browser shows: the selected one, else the first by name —
/// the open-tab handler's seeding rule.
pub(super) fn active_table<'a>(lib: &'a OpenLibrary, browser: &'a LibraryBrowserState) -> &'a str {
    browser.active_table.as_deref().unwrap_or_else(|| {
        lib.tables
            .keys()
            .map(String::as_str)
            .min()
            .unwrap_or_default()
    })
}

impl BrowserViewCache {
    pub(super) fn build(
        lib: &OpenLibrary,
        registry: &signex_library::TemplateRegistry,
        clock: DateTime<Utc>,
        data_version: u64,
        browser: &LibraryBrowserState,
    ) -> Self {
        #[cfg(test)]
        counters::BUILDS.with(|c| c.set(c.get() + 1));
        let table = active_table(lib, browser);
        let rows: &[ComponentRow] = lib.tables.get(table).map(Vec::as_slice).unwrap_or(&[]);

        let needle = browser.search.trim().to_lowercase();
        let lifecycle_filter = browser.lifecycle_filter;
        let class_filter = browser.class_filter.as_deref();
        let mut visible: Vec<usize> = (0..rows.len())
            .filter(|&i| {
                let r = &rows[i];
                lifecycle_filter.allows(r.state)
                    && class_filter.is_none_or(|cls| r.class.as_str() == cls)
                    && (needle.is_empty() || row_matches_filter(r, &needle))
            })
            .collect();

        let prices = PriceView::new(&lib.pricing, clock, rows);
        let columns = derive_columns(rows, lib.library_id, registry, table, &prices);

        // Stage 8: the user's sort selection, applied to the filtered rows.
        if let Some(sort) = browser.sort_by.as_ref()
            && let Some(column) = columns.iter().find(|c| c.kind.sort_key() == sort.key)
        {
            sort_by_cell_key(
                &mut visible,
                |&i| match column.kind {
                    ColumnKind::Price => CellSortKey::from_cell(&prices.sort_value(&rows[i])),
                    _ => column.kind.sort_key_for(&rows[i]),
                },
                sort.descending,
            );
        }

        Self {
            key: ViewKey {
                data_version,
                clock,
                table: table.to_string(),
                search: browser.search.clone(),
                lifecycle_filter,
                class_filter: browser.class_filter.clone(),
                sort_by: browser.sort_by.clone(),
            },
            visible,
            columns,
            main_currency: prices.main_currency().to_string(),
        }
    }

    pub(super) fn matches(
        &self,
        data_version: u64,
        clock: DateTime<Utc>,
        table: &str,
        browser: &LibraryBrowserState,
    ) -> bool {
        self.key.matches(data_version, clock, table, browser)
    }
}

impl LibraryState {
    /// Position of the browser's selected row within the filtered, sorted
    /// grid rows, if the selection is one of them.
    pub(crate) fn browser_row_position_of_selection(
        &self,
        library_path: &std::path::Path,
    ) -> Option<usize> {
        let browser = self.library_browsers.get(library_path)?;
        let selected = browser.selected_row?;
        let lib = self
            .open_libraries
            .iter()
            .find(|l| l.root == library_path)?;
        let rows = lib.tables.get(active_table(lib, browser))?;
        let cache = browser.view_cache.as_ref()?;
        cache
            .visible
            .iter()
            .position(|&i| rows.get(i).is_some_and(|r| r.row_id == selected.as_uuid()))
    }

    /// Bring every browser tab's cache up to date; a tab rebuilds only when
    /// a key input changed.
    pub(in crate::library) fn sync_browser_caches(&mut self) {
        let (version, clock) = (self.data_version, self.clock);
        for (path, browser) in self.library_browsers.iter_mut() {
            let Some(lib) = self.open_libraries.iter().find(|lib| lib.root == *path) else {
                continue;
            };
            let table = active_table(lib, browser);
            if browser
                .view_cache
                .as_ref()
                .is_some_and(|c| c.matches(version, clock, table, browser))
            {
                continue;
            }
            let cache =
                BrowserViewCache::build(lib, &self.template_registry, clock, version, browser);
            browser.view_cache = Some(cache);
        }
    }
}

#[cfg(test)]
mod tests;

/// Rebuild count, so a test can tell "recomputed" from "reused".
#[cfg(test)]
pub(crate) mod counters {
    use std::cell::Cell;

    thread_local! {
        pub static BUILDS: Cell<usize> = const { Cell::new(0) };
    }
}
