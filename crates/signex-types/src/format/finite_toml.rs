//! Save-time guard for the f64 values carried in the `[extras.*]` TOML
//! tables (#828).
//!
//! The TSV rows refuse a non-finite number through `mm_to_nm` /
//! `format_f64`. The extras tables are written by serde instead, and TOML
//! has `nan` / `inf` literals, so a NaN text size or an infinite outline
//! point would save without complaint and come back as the same poison.
//! The writer runs this over the extras tree first and fails the save,
//! naming the offending key path, before any byte reaches the file.

use serde::Serialize;

use super::FormatError;
use crate::numeric::first_non_finite_path;

/// Refuse `tree` if any float in it is NaN or infinite.
pub(in crate::format) fn ensure_finite<T: Serialize>(tree: &T) -> Result<(), FormatError> {
    match first_non_finite_path(tree)? {
        Some(path) => Err(FormatError::NonFiniteExtra { path }),
        None => Ok(()),
    }
}
