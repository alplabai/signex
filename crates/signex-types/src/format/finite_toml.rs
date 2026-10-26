//! Save-time guard for the f64 values carried in the serde-written TOML
//! tables: the manifest, `[stackup]`, `[[zones]]` and `[extras.*]` (#828).
//!
//! The TSV rows refuse a non-finite number through `mm_to_nm` /
//! `format_f64`. These tables are written by serde instead, and TOML has
//! `nan` / `inf` literals, so a NaN text size or an infinite outline point
//! would save without complaint and come back as the same poison. The
//! writer runs this over each tree first and fails the save, naming the
//! offending key path, before any byte reaches the file.

use serde::Serialize;

use super::FormatError;
use crate::numeric::first_non_finite_path;

/// Refuse `tree` if any float in it is NaN or infinite. The wrapper trees
/// (`stackup`, `zones`, `extras`) already carry their table name as the
/// first path segment.
pub(in crate::format) fn ensure_finite<T: Serialize>(tree: &T) -> Result<(), FormatError> {
    refuse_non_finite(tree, "")
}

/// [`ensure_finite`] for the manifest, whose keys sit at the top of the
/// file with no table header: the path reads `manifest.thickness`, not a
/// bare `thickness`.
pub(in crate::format) fn ensure_finite_manifest<T: Serialize>(
    manifest: &T,
) -> Result<(), FormatError> {
    refuse_non_finite(manifest, "manifest.")
}

fn refuse_non_finite<T: Serialize>(tree: &T, prefix: &str) -> Result<(), FormatError> {
    match first_non_finite_path(tree)? {
        Some(path) => Err(FormatError::NonFiniteTomlField {
            path: format!("{prefix}{path}"),
        }),
        None => Ok(()),
    }
}
