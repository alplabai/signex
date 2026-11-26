//! Bulk library writer (#3110): build a whole library in memory, then write
//! its directory in one pass, with no git operations.
//!
//! Converters and importers collect part rows and primitives here and call
//! [`LibraryWriter::write`] once. The bytes come from the same serialisers
//! the adapter uses ([`LibraryFile::write`], `SymbolFile::to_toml_string`,
//! ...), so the output is canonical per `docs/LIBRARY_FORMAT.md` and writing
//! the same input twice gives the same bytes. [`LibraryWriter::write`]
//! creates a new library atomically; [`LibraryWriter::write_update`]
//! overwrites an earlier one per `docs/LIBRARY_CONVERTER_CONTRACT.md` §5.

mod render;
mod update;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::adapter::LibraryError;
use crate::component::ComponentRow;
use crate::library_file::{ColumnType, SnxlibManifest};
use crate::pricing::RowPricing;
use crate::primitive::{FootprintFile, SimFile, SymbolFile};

pub use update::UpdateSummary;

/// Folder of the symbol envelopes, relative to the library root.
pub const SYMBOLS_DIR: &str = "symbols";
/// Folder of the footprint envelopes.
pub const FOOTPRINTS_DIR: &str = "footprints";
/// Folder of the simulation models.
pub const SIMS_DIR: &str = "sims";
/// Folder of the STEP models named by their SHA-256.
pub const STEP_DIR: &str = "step";

/// One part table as the writer holds it.
#[derive(Debug, Clone, Default)]
pub struct TableDraft {
    /// The rows, in any order: the `.snxlib` writer sorts them by `row_id`.
    pub rows: Vec<ComponentRow>,
    /// `[tables.<name>.column_types]`.
    pub column_types: BTreeMap<String, ColumnType>,
    /// `[tables.<name>.pricing...]`, keyed by `row_id`. Normally empty.
    pub pricing: BTreeMap<String, RowPricing>,
}

/// A library being assembled in memory. See the module docs.
#[derive(Debug, Clone)]
pub struct LibraryWriter {
    file_stem: String,
    manifest: SnxlibManifest,
    tables: BTreeMap<String, TableDraft>,
    /// Envelope file name (`<name>.snxsym`) -> envelope.
    symbol_files: BTreeMap<String, SymbolFile>,
    /// Footprint uuid -> its one-footprint envelope.
    footprints: BTreeMap<Uuid, FootprintFile>,
    /// Model uuid -> its one-model envelope.
    sims: BTreeMap<Uuid, SimFile>,
    /// Lower-case hex SHA-256 -> raw STEP bytes.
    step_models: BTreeMap<String, Vec<u8>>,
    /// The `updated` time [`Self::write_update`] gives a changed row.
    update_time: Option<DateTime<Utc>>,
}

impl LibraryWriter {
    /// A library written as `<dir>/<file_stem>.snxlib` with `manifest`.
    pub fn new(file_stem: impl Into<String>, manifest: SnxlibManifest) -> Self {
        Self {
            file_stem: file_stem.into(),
            manifest,
            tables: BTreeMap::new(),
            symbol_files: BTreeMap::new(),
            footprints: BTreeMap::new(),
            sims: BTreeMap::new(),
            step_models: BTreeMap::new(),
            update_time: None,
        }
    }

    pub fn manifest(&self) -> &SnxlibManifest {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut SnxlibManifest {
        &mut self.manifest
    }

    /// The table `name`, created empty on first use.
    pub fn table_mut(&mut self, name: &str) -> &mut TableDraft {
        self.tables.entry(name.to_string()).or_default()
    }

    /// Add `row` to table `table`. Its `content_hash` is recomputed on write.
    pub fn add_row(&mut self, table: &str, row: ComponentRow) {
        self.table_mut(table).rows.push(row);
    }

    /// Add a symbol envelope and return its file name under `symbols/`:
    /// the slug of its display name (or first symbol's name), or
    /// `<file_uuid>.snxsym` when that name is taken, as the editor names a
    /// new envelope.
    pub fn add_symbol_file(&mut self, file: SymbolFile) -> String {
        let label = if file.display_name.trim().is_empty() {
            file.symbols.first().map(|s| s.name.as_str()).unwrap_or("")
        } else {
            file.display_name.as_str()
        };
        let mut name = format!("{}.snxsym", crate::adapters::local_git::slugify(label));
        if self.symbol_files.contains_key(&name) {
            name = format!("{}.snxsym", file.file_uuid);
        }
        self.symbol_files.insert(name.clone(), file);
        name
    }

    /// Add a footprint envelope. It must hold exactly one footprint, which
    /// names the file (`footprints/<uuid>.snxfpt`); see the format's open
    /// point 1.
    pub fn add_footprint_file(&mut self, file: FootprintFile) -> Result<(), LibraryError> {
        let [fp] = file.footprints.as_slice() else {
            return Err(LibraryError::InvalidData(format!(
                "footprint envelope {} holds {} footprints; a library footprint file holds exactly one",
                file.file_uuid,
                file.footprints.len()
            )));
        };
        let uuid = fp.uuid;
        if self.footprints.insert(uuid, file).is_some() {
            return Err(LibraryError::Conflict(format!(
                "footprint {uuid} added twice"
            )));
        }
        Ok(())
    }

    /// Add a simulation envelope holding exactly one model.
    pub fn add_sim_file(&mut self, file: SimFile) -> Result<(), LibraryError> {
        let [model] = file.models.as_slice() else {
            return Err(LibraryError::InvalidData(format!(
                "sim envelope {} holds {} models; a library sim file holds exactly one",
                file.file_uuid,
                file.models.len()
            )));
        };
        let uuid = model.uuid;
        if self.sims.insert(uuid, file).is_some() {
            return Err(LibraryError::Conflict(format!(
                "sim model {uuid} added twice"
            )));
        }
        Ok(())
    }

    /// Store STEP bytes as `step/<sha256>.step` and return the hash, the
    /// value a footprint's `step_attachment.content_hash` takes.
    pub fn add_step_model(&mut self, bytes: Vec<u8>) -> String {
        let hash = crate::datasheet_asset::datasheet_hash(&bytes);
        self.step_models.insert(hash.clone(), bytes);
        hash
    }

    /// Pin the `updated` time [`Self::write_update`] stamps on a changed
    /// row; the current time when unset.
    pub fn set_update_time(&mut self, at: DateTime<Utc>) {
        self.update_time = Some(at);
    }

    /// Every file of the library, path relative to the root -> bytes.
    pub fn render(&self) -> Result<BTreeMap<PathBuf, Vec<u8>>, LibraryError> {
        render::render(self)
    }

    /// Write the library into `dir`, which must not exist or be an empty
    /// folder. The files are written into a temporary sibling folder that is
    /// then renamed to `dir`, so `dir` either holds the whole library or is
    /// left as it was. Returns the path of the `.snxlib`.
    pub fn write(&self, dir: &Path) -> Result<PathBuf, LibraryError> {
        let files = self.render()?;
        render::write_whole(dir, &files)?;
        Ok(dir.join(self.snxlib_file_name()))
    }

    /// Overwrite the library this writer's converter wrote into `dir` before
    /// (contract §5). See [`update`] for what is kept and what is replaced.
    pub fn write_update(&self, dir: &Path) -> Result<UpdateSummary, LibraryError> {
        update::write_update(self, dir)
    }

    fn snxlib_file_name(&self) -> String {
        format!("{}.snxlib", self.file_stem)
    }
}
