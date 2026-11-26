//! The cell result cache: the last result of each cell with the source it
//! ran, the values it read and the runtime it ran in. A cell runs again only
//! when one of those changes, so reopening a notebook runs nothing.
//!
//! The project keeps it in [`CELL_CACHE_PATH`], a JSON file with the format
//! token [`CELL_CACHE_FORMAT`]. It is a cache, not a document: a file this
//! build cannot read is set aside and the cells show as never run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::inputs::InputValue;
use super::runner::RunOutput;

/// The cache file, inside the project folder, beside the other files
/// Signex keeps for a project.
pub const CELL_CACHE_PATH: &str = ".signex/notebook-cells.json";
/// The format token of the cache file.
pub const CELL_CACHE_FORMAT: &str = "notebook-cells/1";

/// The last result of one cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedCell {
    /// [`source_hash`] of the source it ran.
    pub source_hash: String,
    /// The runtime it ran in, as [`super::RuntimeStatus::Ready`] names it.
    pub runtime: String,
    /// The values it read, as they were.
    pub inputs: Vec<InputValue>,
    pub output: RunOutput,
    /// When it ran, as the caller's clock wrote it (`2026-11-24T14:02`).
    pub ran_at: String,
}

/// Every cell's last result, by label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellCache {
    cells: BTreeMap<String, CachedCell>,
}

/// Why a cache file was set aside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CacheNote {
    /// Not JSON this build reads; `reason` says why.
    Unreadable { reason: String },
    /// Written with another token, a newer Signex's or none.
    OtherFormat { format: String },
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    format: String,
    #[serde(default)]
    cells: BTreeMap<String, CachedCell>,
}

/// The hash a cached result keeps of the source it ran: SHA-256, in hex.
pub fn source_hash(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl CellCache {
    /// The cache in the bytes of a cache file. A file that is not one this
    /// build reads gives an empty cache and says why.
    pub fn read(bytes: &[u8]) -> (Self, Option<CacheNote>) {
        let file: CacheFile = match serde_json::from_slice(bytes) {
            Ok(file) => file,
            Err(error) => {
                let reason = error.to_string();
                return (Self::default(), Some(CacheNote::Unreadable { reason }));
            }
        };
        if file.format != CELL_CACHE_FORMAT {
            let format = file.format;
            return (Self::default(), Some(CacheNote::OtherFormat { format }));
        }
        (Self { cells: file.cells }, None)
    }

    /// The cache as the bytes of its file: keys sorted, so the same results
    /// write the same file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let file = CacheFile {
            format: CELL_CACHE_FORMAT.to_string(),
            cells: self.cells.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&file).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    /// The last result of the cell labelled `label`.
    pub fn get(&self, label: &str) -> Option<&CachedCell> {
        self.cells.get(label)
    }

    /// Keep `cached` as the last result of `label`.
    pub fn insert(&mut self, label: &str, cached: CachedCell) {
        self.cells.insert(label.to_string(), cached);
    }

    /// Drop the results of cells whose label is not in `labels`, so a
    /// deleted cell's result does not stay in the file.
    pub fn retain_labels<'a>(&mut self, labels: impl IntoIterator<Item = &'a str>) {
        let keep: std::collections::BTreeSet<&str> = labels.into_iter().collect();
        self.cells.retain(|label, _| keep.contains(label.as_str()));
    }

    /// How many results it holds.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}
