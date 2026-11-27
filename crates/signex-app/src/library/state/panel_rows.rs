//! The searchable row lists of the Components panel and the Library
//! panel's Parts kind, built once per change instead of once per frame
//! (#1353; the Library panel: mockup row 119).
//!
//! `view` must stay cheap and pure, so it only reads this cache. `update`
//! keeps it current through [`LibraryState::sync_view_caches_in`]: the
//! lowercase search keys are rebuilt only when the set of components
//! changes (`data_version`), and the filtered list only when the query or
//! a library name changes.

use super::*;
use crate::panels::components_panel::query::{MatchLine, PanelQuery, SearchKeys, match_summary};

/// One cached row: where it lives, plus the keys the panel's value-aware
/// search matches against (mockup row 52).
#[derive(Debug, Clone)]
struct RowKey {
    table: Arc<str>,
    index: usize,
    search: SearchKeys,
}

#[derive(Debug, Clone, Default)]
struct LibraryRows {
    /// Lowercased display name the `visible` list was filtered for.
    name_lc: String,
    keys: Vec<RowKey>,
    /// Indices into `keys` matching the current query, in table-name then
    /// row order.
    visible: Vec<usize>,
}

impl LibraryRows {
    fn build(i18n: &signex_i18n::I18n, lib: &OpenLibrary) -> Self {
        let mut tables: Vec<(&String, &Vec<ComponentRow>)> = lib.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        let mut keys = Vec::with_capacity(lib.total_rows());
        for (name, rows) in tables {
            let table: Arc<str> = Arc::from(name.as_str());
            keys.extend(rows.iter().enumerate().map(|(index, r)| RowKey {
                table: Arc::clone(&table),
                index,
                search: SearchKeys::of(i18n, r),
            }));
        }
        Self {
            name_lc: String::new(),
            keys,
            visible: Vec::new(),
        }
    }

    /// Every word must match the row or the library name.
    fn filter(&mut self, name_lc: String, query: &PanelQuery) {
        let words: Vec<_> = query
            .words()
            .iter()
            .filter(|w| !name_lc.contains(&w.folded))
            .collect();
        self.visible = (0..self.keys.len())
            .filter(|&i| words.iter().all(|w| self.keys[i].search.matches(w)))
            .collect();
        self.name_lc = name_lc;
        #[cfg(test)]
        counters::FILTER_PASSES.with(|c| c.set(c.get() + 1));
    }
}

/// The panel's rows for every open library, with the inputs they were
/// built from.
#[derive(Debug, Clone, Default)]
pub struct PanelRows {
    /// `LibraryState::data_version` the keys were built at.
    built_at: Option<u64>,
    /// The message locale the keys were folded in.
    built_in: Option<&'static str>,
    /// The trimmed query `visible` lists were filtered for, as typed:
    /// prefixes are case-sensitive, so `100m` and `100M` differ.
    needle: Option<String>,
    libs: HashMap<PathBuf, LibraryRows>,
    /// What each bare number of the query matched, over every library.
    summary: Vec<MatchLine>,
}

impl PanelRows {
    /// Number of rows of the library at `root` matching the query.
    pub fn visible_count(&self, root: &Path) -> usize {
        self.libs.get(root).map_or(0, |l| l.visible.len())
    }

    /// The first `limit` matching rows of the library at `root`, as
    /// `(table, row index within the table)`.
    pub fn visible_rows<'a>(
        &'a self,
        root: &Path,
        limit: usize,
    ) -> impl Iterator<Item = (&'a str, usize)> + 'a {
        self.libs.get(root).into_iter().flat_map(move |l| {
            l.visible.iter().take(limit).map(|&i| {
                let k = &l.keys[i];
                (&*k.table, k.index)
            })
        })
    }

    /// The match summary line of each bare number in the query (`100`
    /// finds 100 pF, 100 nF and 100 µF), empty without one.
    pub(crate) fn match_summary(&self) -> &[MatchLine] {
        &self.summary
    }

    pub(super) fn sync(
        &mut self,
        i18n: &signex_i18n::I18n,
        libraries: &[OpenLibrary],
        data_version: u64,
        filter: &str,
    ) {
        let needle = filter.trim().to_owned();
        let keys_stale =
            self.built_at != Some(data_version) || self.built_in != Some(i18n.locale());
        if keys_stale {
            self.libs = libraries
                .iter()
                .map(|lib| (lib.root.clone(), LibraryRows::build(i18n, lib)))
                .collect();
            self.built_at = Some(data_version);
            self.built_in = Some(i18n.locale());
            #[cfg(test)]
            counters::KEY_BUILDS.with(|c| c.set(c.get() + 1));
        }
        let needle_changed = self.needle.as_deref() != Some(needle.as_str());
        let query = PanelQuery::parse(i18n, &needle);
        let mut refiltered = false;
        for lib in libraries {
            let name_lc = i18n.fold_for_search(&lib.display_name);
            let Some(rows) = self.libs.get_mut(&lib.root) else {
                continue;
            };
            if keys_stale || needle_changed || rows.name_lc != name_lc {
                rows.filter(name_lc, &query);
                refiltered = true;
            }
        }
        if refiltered || keys_stale {
            let matched = self
                .libs
                .values()
                .flat_map(|l| l.visible.iter().map(|&i| &l.keys[i].search));
            self.summary = match_summary(&query, matched);
        }
        self.needle = Some(needle);
    }
}

impl LibraryState {
    pub(super) fn sync_panel_rows(&mut self, i18n: &signex_i18n::I18n) {
        let version = self.data_version;
        self.components_panel.rows.sync(
            i18n,
            &self.open_libraries,
            version,
            &self.components_panel.filter,
        );
        self.sync_components_table(i18n);
        // The Library panel's Parts filter matches rows the same way
        // (mockup row 119).
        self.library_panel
            .rows
            .sync(i18n, &self.open_libraries, version, &self.panel_search);
    }
}

impl LibraryState {
    /// Rebuild what the Components panel table shows, if its inputs
    /// changed (the search must be current first).
    pub(super) fn sync_components_table(&mut self, i18n: &signex_i18n::I18n) {
        let panel = &mut self.components_panel;
        let src = crate::panels::components_panel::table::TableSource {
            libraries: &self.open_libraries,
            rows: &panel.rows,
            registry: &self.template_registry,
            data_version: self.data_version,
            needle: &panel.filter,
            selected: panel.selected.as_ref(),
        };
        crate::panels::components_panel::table::sync(&mut panel.table, &src, i18n);
    }
}

/// Rebuild counts, so a test can tell "recomputed" from "reused".
#[cfg(test)]
pub(crate) mod counters {
    use std::cell::Cell;

    thread_local! {
        pub static KEY_BUILDS: Cell<usize> = const { Cell::new(0) };
        pub static FILTER_PASSES: Cell<usize> = const { Cell::new(0) };
    }
}
