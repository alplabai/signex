//! Forward-compatible capture of TSV columns this build does not
//! recognise — the row-level twin of [`UnknownFields`](super::UnknownFields).
//!
//! A newer Signex may append columns to a bulk block. Each row keeps
//! its own foreign cells here, so they survive a load -> edit -> save
//! cycle on the in-memory model and stay attached to the right row.
//!
//! # Contract
//!
//! - **Captured** by `parse_tsv_block`: every header column that is not
//!   in `SnxTable::columns()`, as `(name, decoded cell)` in file order.
//! - **Re-emitted** by `write_tsv_block` after the known columns, in
//!   first-seen order across the block's rows (the union). A row that
//!   lacks one of the block's foreign columns writes an empty cell, so
//!   the block stays rectangular. Original position among the known
//!   columns is not kept; relative order of the foreign ones is.
//! - Names are unique per block; a repeated header name is rejected.

use serde::{Deserialize, Serialize};

/// Foreign cells of one TSV row, in original column order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnknownColumns(Vec<(String, String)>);

impl UnknownColumns {
    /// `true` when the row carries no foreign cells.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Foreign `(column, cell)` pairs in original order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The cell under `column`, if this row has one.
    pub fn get(&self, column: &str) -> Option<&str> {
        self.iter().find(|(k, _)| *k == column).map(|(_, v)| v)
    }

    pub(in crate::format) fn from_pairs(pairs: Vec<(String, String)>) -> Self {
        Self(pairs)
    }
}

/// The two `SnxTable` accessors every row DTO with an
/// `unknown_columns` field implements identically.
macro_rules! impl_unknown_columns_accessors {
    () => {
        fn unknown_columns(&self) -> &$crate::format::UnknownColumns {
            &self.unknown_columns
        }

        fn with_unknown_columns(mut self, columns: $crate::format::UnknownColumns) -> Self {
            self.unknown_columns = columns;
            self
        }
    };
}
pub(in crate::format) use impl_unknown_columns_accessors;
