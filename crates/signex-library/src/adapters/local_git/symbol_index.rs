//! uuid -> `.snxsym` container path, so a symbol lookup reads one file
//! instead of parsing every container (#1362).
//!
//! The index is only a *hint*. A hit is trusted after the named file is
//! re-read and still holds the uuid, so an external edit, rename or delete
//! can never yield a wrong file: a mismatch drops the entry and the caller
//! rescans the directory, which also rebuilds the index.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use uuid::Uuid;

use super::LocalGitAdapter;
use crate::primitive::SymbolFile;

#[derive(Debug, Default)]
pub(super) struct SymbolIndex {
    paths: Mutex<HashMap<Uuid, PathBuf>>,
}

impl SymbolIndex {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, PathBuf>> {
        self.paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Replace the whole index from a fresh full scan.
    pub(super) fn rebuild(&self, scanned: &[(PathBuf, SymbolFile)]) {
        let mut map = self.map();
        map.clear();
        for (path, file) in scanned {
            for sym in &file.symbols {
                map.insert(sym.uuid, path.clone());
            }
        }
    }

    /// Point every symbol of `file` at `path` after a write.
    pub(super) fn record(&self, file: &SymbolFile, path: &Path) {
        let mut map = self.map();
        for sym in &file.symbols {
            map.insert(sym.uuid, path.to_path_buf());
        }
    }

    fn path_for(&self, uuid: Uuid) -> Option<PathBuf> {
        self.map().get(&uuid).cloned()
    }

    fn forget(&self, uuid: Uuid) {
        self.map().remove(&uuid);
    }
}

impl LocalGitAdapter {
    /// The container the index names for `uuid`, re-read and verified.
    /// `None` on a miss or any mismatch (entry dropped); never a wrong file.
    pub(super) fn indexed_symbol_file(&self, uuid: Uuid) -> Option<(PathBuf, SymbolFile)> {
        let path = self.symbol_index.path_for(uuid)?;
        let file = fs::read(&path)
            .ok()
            .and_then(|bytes| SymbolFile::from_bytes(&bytes).ok())
            .filter(|f| f.symbols.iter().any(|s| s.uuid == uuid));
        match file {
            Some(file) => Some((path, file)),
            None => {
                self.symbol_index.forget(uuid);
                None
            }
        }
    }
}
