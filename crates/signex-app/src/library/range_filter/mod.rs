//! Range filter on a measurement column, for the library browser and the
//! Place Component picker (#1572, phase 4).
//!
//! Users never type a prefix or unit here, as in the editors: an operator
//! picker (`≥ ≤ = between`), a number field (digits, a sign and a decimal
//! point only), an SI-prefix picker, and the unit fixed to the column's.
//! `between` has two number-and-prefix pairs.
//!
//! * [`draft`]: the model: what the user has set, and the validated
//!   [`signex_library::RangeFilter`] it yields. Pure; no IO.
//! * [`retain_in_range`]: applies the active filters to a row list and
//!   counts the rows a filter excluded.
//! * [`view`]: the pure widget both surfaces draw.
//!
//! The predicate itself (base-unit comparison, the `=` tolerance, the
//! exclusion rule) lives in `signex_library::range_filter`.

mod apply;
mod draft;
mod view;

#[cfg(test)]
mod tests;

pub(crate) use apply::retain_in_range;
pub use draft::{BoundDraft, RangeFilterDraft, RangeFilters};
pub(crate) use view::{FilterColumn, range_filter_bar};
