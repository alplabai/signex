//! The Components Panel's searchable row list, built once per change
//! instead of once per frame (#1353).
//!
//! `view` must stay cheap and pure, so it only reads this cache. `update`
//! keeps it current through [`LibraryState::sync_view_caches`]: the
//! lowercase search keys are rebuilt only when the set of components
//! changes (`data_version`), and the filtered list only when the query or
//! a library name changes.

use super::*;

/// One cached row: where it lives, plus the lowercase keys the panel's
/// substring filter matches against.
#[derive(Debug, Clone)]
struct RowKey {
    table: Arc<str>,
    index: usize,
    mpn: String,
    manufacturer: String,
    internal_pn: String,
}

impl RowKey {
    fn matches(&self, needle: &str) -> bool {
        self.mpn.contains(needle)
            || self.manufacturer.contains(needle)
            || self.internal_pn.contains(needle)
    }
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
    fn build(lib: &OpenLibrary) -> Self {
        let mut tables: Vec<(&String, &Vec<ComponentRow>)> = lib.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        let mut keys = Vec::with_capacity(lib.total_rows());
        for (name, rows) in tables {
            let table: Arc<str> = Arc::from(name.as_str());
            keys.extend(rows.iter().enumerate().map(|(index, r)| RowKey {
                table: Arc::clone(&table),
                index,
                mpn: r.primary_mpn.mpn.to_lowercase(),
                manufacturer: r.primary_mpn.manufacturer.to_lowercase(),
                internal_pn: r.internal_pn.as_str().to_lowercase(),
            }));
        }
        Self {
            name_lc: String::new(),
            keys,
            visible: Vec::new(),
        }
    }

    fn filter(&mut self, name_lc: String, needle: &str) {
        let all = needle.is_empty() || name_lc.contains(needle);
        self.visible = (0..self.keys.len())
            .filter(|&i| all || self.keys[i].matches(needle))
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
    /// The normalised query `visible` lists were filtered for.
    needle: Option<String>,
    libs: HashMap<PathBuf, LibraryRows>,
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

    pub(super) fn sync(&mut self, libraries: &[OpenLibrary], data_version: u64, filter: &str) {
        let needle = filter.trim().to_lowercase();
        let keys_stale = self.built_at != Some(data_version);
        if keys_stale {
            self.libs = libraries
                .iter()
                .map(|lib| (lib.root.clone(), LibraryRows::build(lib)))
                .collect();
            self.built_at = Some(data_version);
            #[cfg(test)]
            counters::KEY_BUILDS.with(|c| c.set(c.get() + 1));
        }
        let needle_changed = self.needle.as_deref() != Some(needle.as_str());
        for lib in libraries {
            let name_lc = lib.display_name.to_lowercase();
            let Some(rows) = self.libs.get_mut(&lib.root) else {
                continue;
            };
            if keys_stale || needle_changed || rows.name_lc != name_lc {
                rows.filter(name_lc, &needle);
            }
        }
        self.needle = Some(needle);
    }
}

impl LibraryState {
    pub(super) fn sync_panel_rows(&mut self) {
        let version = self.data_version;
        self.components_panel.rows.sync(
            &self.open_libraries,
            version,
            &self.components_panel.filter,
        );
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
