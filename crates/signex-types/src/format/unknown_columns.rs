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
//!
//! # Hostile-input caps
//!
//! `.snxsch` / `.snxpcb` files may come from untrusted sources, so a
//! block's foreign columns are bounded. A file that exceeds a cap fails
//! to load with a precise [`FormatError`](super::FormatError); nothing
//! is ever truncated silently.
//!
//! - [`MAX_FOREIGN_COLUMNS`] (256) foreign columns per block —
//!   `FormatError::TsvTooManyForeignColumns` (names the first column
//!   over the cap; on the write side also the objects, by uuid and
//!   designator/kind, that carry a column over the cap, at most
//!   8 then a count).
//! - [`MAX_FOREIGN_COLUMN_NAME_BYTES`] (256) bytes per foreign column
//!   name — `FormatError::TsvForeignColumnNameTooLong`.
//!
//! The same caps bind both directions, so a file this build writes is
//! always one it can read back: `write_tsv_block` checks the union of
//! its rows' foreign names before emitting a byte and returns the same
//! errors, plus `TsvForeignColumnShadowsKnown` when a foreign name equals
//! a known column (the header would repeat it and the loader would
//! refuse the file) (`write_string` fails, the atomic write is never reached and
//! the previous file stays intact). A row that is deserialised (not
//! parsed) is validated on the way in too, so an `UnknownColumns` with an
//! over-long or repeated name cannot be constructed.
//!
//! Each foreign column name is allocated once per block and shared
//! (`Arc<str>`) by every row, so memory grows with the cell values, not
//! with `rows x name length`.

use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize};

use super::FormatError;

/// Most foreign columns one TSV block may carry.
pub const MAX_FOREIGN_COLUMNS: usize = 256;

/// Longest foreign column name, in bytes.
pub const MAX_FOREIGN_COLUMN_NAME_BYTES: usize = 256;

/// Foreign cells of one TSV row, in original column order. The column
/// names are shared with every other row of the same block.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct UnknownColumns(Vec<(Arc<str>, String)>);

/// Enforce the caps on a block's foreign column names (in file or
/// first-seen order). Shared by the reader, the writer and
/// deserialisation so the three can never disagree.
pub(in crate::format) fn check_foreign_names<S: AsRef<str>>(
    block: &str,
    names: &[S],
) -> Result<(), FormatError> {
    if names.len() > MAX_FOREIGN_COLUMNS {
        return Err(FormatError::TsvTooManyForeignColumns {
            block: block.to_string(),
            count: names.len(),
            max: MAX_FOREIGN_COLUMNS,
            first_over: bounded_name(names[MAX_FOREIGN_COLUMNS].as_ref()),
            objects: Vec::new(),
            more_objects: 0,
        });
    }
    check_foreign_name_lengths(block, names)
}

/// Longest name prefix an error message may echo, in characters.
pub(in crate::format) const ERROR_NAME_PREFIX_CHARS: usize = 32;

/// Most header names a header-mismatch error may echo.
pub(in crate::format) const MAX_ERROR_HEADER_NAMES: usize = 16;

/// Most objects an over-cap write error may name.
pub(in crate::format) const MAX_NAMED_OBJECTS: usize = 8;

/// `name` cut to [`ERROR_NAME_PREFIX_CHARS`] characters (char-boundary
/// safe), for an error message about untrusted text.
pub(in crate::format) fn bounded_name(name: &str) -> String {
    name.chars().take(ERROR_NAME_PREFIX_CHARS).collect()
}

/// The ` ; objects ...` tail of `TsvTooManyForeignColumns`, empty when
/// no object is known.
pub(in crate::format) fn objects_clause(objects: &[String], more: usize) -> String {
    if objects.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "; objects carrying columns over the limit: {}",
        objects.join(", ")
    );
    if more > 0 {
        out.push_str(&format!(" and {more} more"));
    }
    out
}

fn check_foreign_name_lengths<S: AsRef<str>>(block: &str, names: &[S]) -> Result<(), FormatError> {
    for name in names {
        let name = name.as_ref();
        if name.len() > MAX_FOREIGN_COLUMN_NAME_BYTES {
            return Err(FormatError::TsvForeignColumnNameTooLong {
                block: block.to_string(),
                prefix: bounded_name(name),
                len: name.len(),
                max: MAX_FOREIGN_COLUMN_NAME_BYTES,
            });
        }
    }
    Ok(())
}

impl<'de> Deserialize<'de> for UnknownColumns {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let pairs = Vec::<(Arc<str>, String)>::deserialize(deserializer)?;
        let names: Vec<&str> = pairs.iter().map(|(k, _)| &**k).collect();
        check_foreign_names("<row>", &names).map_err(serde::de::Error::custom)?;
        let mut seen = std::collections::HashSet::with_capacity(names.len());
        if let Some(dup) = names.iter().find(|n| !seen.insert(**n)) {
            return Err(serde::de::Error::custom(format!(
                "repeated foreign column {dup:?}"
            )));
        }
        Ok(Self(pairs))
    }
}

impl UnknownColumns {
    /// `true` when the row carries no foreign cells.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Foreign `(column, cell)` pairs in original order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (&**k, v.as_str()))
    }

    /// The cell under `column`, if this row has one.
    pub fn get(&self, column: &str) -> Option<&str> {
        self.iter().find(|(k, _)| *k == column).map(|(_, v)| v)
    }

    pub(in crate::format) fn from_pairs(pairs: Vec<(Arc<str>, String)>) -> Self {
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
