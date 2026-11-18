//! The address of a library row opened in a Component Preview tab.

use std::path::PathBuf;

use signex_library::RowId;

/// Identity for an open Component Preview tab — the lookup key for
/// `LibraryState::editors` and the address that preview view closures
/// clone into messages. Rows live in `tables/<name>.tsv` and are
/// addressed by `(library_path, table, row_id)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorAddress {
    pub library_path: PathBuf,
    pub table: String,
    pub row_id: RowId,
}

impl EditorAddress {
    pub fn new(library_path: PathBuf, table: String, row_id: RowId) -> Self {
        Self {
            library_path,
            table,
            row_id,
        }
    }

    /// Is `path` a [`Self::synthetic_tab_path`]? Such a path names no file:
    /// its last segment is `<table>.tsv#<row id>`.
    pub fn is_synthetic_tab_path(path: &std::path::Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(".tsv#"))
    }

    /// Synthetic on-disk identity for a Component Preview tab — used by
    /// `TabInfo.path` so the tab bar, undock detector, and dirty-paths
    /// machinery have a single unique `PathBuf` per row without needing
    /// a second identity scheme. The path points at the row's home table
    /// with the `row_id` as a suffix so the synthetic key is unique
    /// per-row even when multiple rows share a table.
    pub fn synthetic_tab_path(&self) -> PathBuf {
        self.library_path
            .join("tables")
            .join(format!("{}.tsv#{}", self.table, self.row_id))
    }
}
