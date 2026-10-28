//! `ReadOnlyAdapter` — marks a mounted library read-only.
//!
//! Read-only is a property of the **mount**, not of the filesystem: the
//! stock library ships in a system directory that may or may not be
//! writable, and the behaviour must be identical either way. Wrapping
//! the adapter keeps the guarantee at the adapter boundary — every
//! caller that holds a `&dyn LibraryAdapter` (UI, cascade, tooling)
//! gets the same typed [`LibraryError::ReadOnly`] before any byte is
//! touched — and keeps the [`LibrarySet`](super::library_set::LibrarySet)
//! resolver untouched: reads delegate straight through.
//!
//! The wrapper implements the whole trait explicitly (no reliance on
//! trait defaults) so a write entry point can't slip through to the
//! inner adapter by omission.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::adapter::{HistoryEntry, LibraryAdapter, LibraryError, PrimitiveSummary, Versioned};
use crate::component::ComponentRow;
use crate::identity::{InternalPn, RowId};
use crate::library_file::{ClassEntry, LibraryFile};
use crate::manifest::Manifest;
use crate::primitive::{Footprint, SimModel, Symbol};

/// Wraps any adapter and refuses every write with
/// [`LibraryError::ReadOnly`]. Reads delegate to the inner adapter.
pub struct ReadOnlyAdapter {
    inner: Box<dyn LibraryAdapter>,
}

impl ReadOnlyAdapter {
    /// Mark `inner` read-only.
    pub fn new(inner: Box<dyn LibraryAdapter>) -> Self {
        Self { inner }
    }

    fn refuse<T>(&self) -> Result<T, LibraryError> {
        Err(LibraryError::ReadOnly {
            library: self.inner.manifest().library.name.clone(),
        })
    }
}

impl LibraryAdapter for ReadOnlyAdapter {
    fn library_id(&self) -> Uuid {
        self.inner.library_id()
    }

    fn manifest(&self) -> &Manifest {
        self.inner.manifest()
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn library_file(&self) -> Option<&LibraryFile> {
        self.inner.library_file()
    }

    fn root_dir(&self) -> Option<&Path> {
        self.inner.root_dir()
    }

    fn library_file_path(&self) -> Option<&Path> {
        self.inner.library_file_path()
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        self.inner.list_tables()
    }

    fn create_empty_table(&self, _name: &str, _msg: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn delete_empty_table(&self, _name: &str, _msg: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn rename_table(&self, _old: &str, _new: &str, _msg: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn library_classes(&self) -> Vec<ClassEntry> {
        self.inner.library_classes()
    }

    fn update_library_classes(
        &self,
        _classes: Vec<ClassEntry>,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn add_library_class(&self, _entry: ClassEntry, _msg: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn remove_library_class(&self, _key: &str, _msg: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn rename_library_class(
        &self,
        _old_key: &str,
        _new_entry: ClassEntry,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn read_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        self.inner.read_table(name)
    }

    fn iter_rows(&self) -> Result<Vec<(String, ComponentRow)>, LibraryError> {
        self.inner.iter_rows()
    }

    fn read_row(&self, table: &str, row_id: RowId) -> Result<ComponentRow, LibraryError> {
        self.inner.read_row(table, row_id)
    }

    fn read_row_by_pn(&self, pn: &InternalPn) -> Result<(String, ComponentRow), LibraryError> {
        self.inner.read_row_by_pn(pn)
    }

    fn insert_row(&self, _table: &str, _row: ComponentRow, _msg: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn update_row(
        &self,
        _table: &str,
        _row: ComponentRow,
        _msg: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.refuse()
    }

    fn delete_row(
        &self,
        _table: &str,
        _row_id: RowId,
        _base_revision: Option<u64>,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.inner.get_symbol(uuid)
    }

    fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.inner.get_footprint(uuid)
    }

    fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        self.inner.get_sim(uuid)
    }

    fn save_symbol(&self, _sym: Symbol, _message: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn save_footprint(&self, _fp: Footprint, _message: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn save_sim(&self, _sm: SimModel, _message: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn get_symbol_versioned(&self, uuid: Uuid) -> Result<Versioned<Symbol>, LibraryError> {
        self.inner.get_symbol_versioned(uuid)
    }

    fn get_footprint_versioned(&self, uuid: Uuid) -> Result<Versioned<Footprint>, LibraryError> {
        self.inner.get_footprint_versioned(uuid)
    }

    fn get_sim_versioned(&self, uuid: Uuid) -> Result<Versioned<SimModel>, LibraryError> {
        self.inner.get_sim_versioned(uuid)
    }

    fn save_symbol_versioned(
        &self,
        _sym: Symbol,
        _base: Option<u64>,
        _message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.refuse()
    }

    fn save_footprint_versioned(
        &self,
        _fp: Footprint,
        _base: Option<u64>,
        _message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.refuse()
    }

    fn save_sim_versioned(
        &self,
        _sm: SimModel,
        _base: Option<u64>,
        _message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.refuse()
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.inner.list_symbols()
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.inner.list_footprints()
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.inner.list_sims()
    }

    fn root_path(&self) -> Option<PathBuf> {
        self.inner.root_path()
    }

    fn rewrite_file_locked(
        &self,
        _abs_path: &Path,
        _commit_message: &str,
        _rewrite: &mut crate::adapter::FileRewrite<'_>,
    ) -> Result<bool, LibraryError> {
        // The trait default writes the file directly, which would bypass
        // the mount's read-only guarantee.
        self.refuse()
    }

    fn bind_row_primitive_version(
        &self,
        _table: &str,
        _row_id: RowId,
        _save: &crate::cascade::PrimitiveSave,
        _message: &str,
    ) -> Result<bool, LibraryError> {
        self.refuse()
    }

    fn update_rows(
        &self,
        _targets: &[(String, RowId)],
        _edit: &mut crate::adapter::RowEdit<'_>,
        _message: &str,
    ) -> Result<Vec<crate::adapter::RowEditOutcome>, LibraryError> {
        self.refuse()
    }

    fn relocate_library_file(&mut self, _new_file: &Path) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn symbol_listing_is_partial(&self) -> bool {
        self.inner.symbol_listing_is_partial()
    }

    fn commit_external_change(&self, _abs_path: &Path, _message: &str) -> Result<(), LibraryError> {
        self.refuse()
    }

    fn table_pricing(
        &self,
        table: &str,
    ) -> Result<std::collections::BTreeMap<Uuid, crate::pricing::RowPricing>, LibraryError> {
        self.inner.table_pricing(table)
    }

    fn store_pricing(
        &self,
        _updates: Vec<crate::pricing::PricingUpdate>,
        _cancel: &std::sync::atomic::AtomicBool,
        _msg: &str,
    ) -> Result<crate::pricing::StoreOutcome, LibraryError> {
        self.refuse()
    }

    fn history(&self, primitive_path: &Path) -> Result<Vec<HistoryEntry>, LibraryError> {
        self.inner.history(primitive_path)
    }
}
