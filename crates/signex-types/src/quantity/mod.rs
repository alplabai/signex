//! Typed electrical quantities: units, SI prefixes, lossless parsing of
//! the editor's number field and formatting. Library-free; no UI, no IO.
//!
//! Users never type a prefix or unit. Editors pair a number field
//! ([`parse_mantissa`]) with a prefix picker ([`SiPrefix::all_for`]) and a
//! fixed or picked unit ([`Unit::STANDARD`]); [`quantity_from_parts`]
//! joins them into a [`Quantity`] without float multiplication.
//!
//! # Relation to `Measurement { value, unit }`
//!
//! The canonical stored form is the base value plus the unit token
//! (`{4700, "ohm"}`). Rows written earlier keep the SI prefix inside the
//! unit string (`{4.7, "kΩ"}`); [`quantity_from_stored`] reads both as
//! base 4700 Ω. Storage is unchanged: nothing here writes a file, a row
//! is never rewritten just to normalise it, and an unrecognised unit
//! string is kept as [`Unit::Other`].
//!
//! Lengths are a separate home (epic #1058, `units/`); this module covers
//! electrical values only.

mod format;
mod parse;
mod prefix;
mod unit;
mod value;

#[cfg(test)]
mod tests;

pub use format::{ValueStyle, format_eng, format_quantity};
pub use parse::{
    MantissaError, parse_mantissa, quantity_from_parts, quantity_from_stored, split_unit_string,
    unit_string_matches,
};
pub use prefix::SiPrefix;
pub use unit::Unit;
pub use value::Quantity;
