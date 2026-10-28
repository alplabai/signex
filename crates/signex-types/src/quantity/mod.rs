//! Typed electrical quantities: units, SI prefixes, the editor's number
//! parser and engineering-notation formatting. Users never type a prefix
//! or unit: editors pair a number field ([`parse_mantissa`]) with prefix
//! and unit pickers ([`SiPrefix::all_for`], [`Unit::ALL`]). Library-free; no UI, no IO.
//!
//! # Relation to `Measurement { value, unit }`
//!
//! A stored measurement keeps its SI prefix inside the unit string
//! (`value: 4.7, unit: "kΩ"`). [`split_unit_string`] reads that string as
//! `(SiPrefix, Unit)`, and [`base_value`] gives mantissa × prefix as the
//! base-unit value (`4700.0` Ω) that sort and filter compare. Storage is
//! unchanged: nothing here writes a file, and an unrecognised unit string
//! simply yields `None`.
//!
//! Lengths are a separate home (epic #1058, `units/`); this module covers
//! electrical values only.

mod format;
mod parse;
mod unit;
mod value;

#[cfg(test)]
mod tests;

pub use format::{format_eng, format_parts};
pub use parse::{MantissaError, base_value, parse_mantissa, split_unit_string};
pub use unit::{SiPrefix, Unit};
pub use value::Quantity;
