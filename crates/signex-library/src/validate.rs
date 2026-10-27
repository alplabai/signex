//! Write-time validation of a component row.
//!
//! Every write path (TSV table, `.snxlib`, the database adapter and the
//! library server) calls [`validate_row_for_write`] on the row being
//! written, before serialising it.

use crate::adapter::LibraryError;
use crate::alternate_symbol::alternate_symbol_issues;
use crate::component::ComponentRow;
use crate::footprint_binding::ensure_footprint_bindings_valid;
use crate::hash::check_param_map_finite;

/// Refuse a row that must not be stored, naming the row. Three steps:
/// [`check_parameters_finite`], then [`check_alternate_symbols`], then the
/// footprint bindings (alternates need a default; none bound twice, #1234).
pub fn validate_row_for_write(row: &ComponentRow) -> Result<(), LibraryError> {
    check_parameters_finite(row)?;
    check_alternate_symbols(row)?;
    ensure_footprint_bindings_valid(row)
}

/// Refuse a NaN / infinite parameter, naming the parameter key.
/// `serde_json` writes a non-finite float as `null`, which cannot be read
/// back, so storing one would leave the table or library unloadable (#1056).
fn check_parameters_finite(row: &ComponentRow) -> Result<(), LibraryError> {
    check_param_map_finite(&row.parameters).map_err(|e| match e {
        LibraryError::Backend(msg) => LibraryError::Backend(format!(
            "row {} ({}): {msg}; nothing was written",
            row.internal_pn, row.row_id
        )),
        other => other,
    })
}

/// Refuse an unbound, default-equal, duplicate or mislabelled alternate
/// symbol (see [`alternate_symbol_issues`]).
fn check_alternate_symbols(row: &ComponentRow) -> Result<(), LibraryError> {
    match alternate_symbol_issues(row).first() {
        Some(issue) => Err(LibraryError::Backend(format!(
            "row {} ({}): {issue}; nothing was written",
            row.internal_pn, row.row_id
        ))),
        None => Ok(()),
    }
}
