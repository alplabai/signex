//! Coordinate-unit helpers for the on-disk wire format.
//!
//! On disk, schematic and PCB positions are emitted as integer
//! nanometres so the wire format is precision-stable across hand
//! edits. In memory, `Point` is `f64` mm. The conversion happens at
//! the row-row boundary: [`mm_to_nm`] / [`nm_to_mm`].
//!
//! Pure code motion out of `mod.rs`; `pub(in crate::format)` so the
//! sibling row-translation modules can reach the converters, exactly
//! as when they lived in the single-file module.
//!
//! NB: this module keeps its *own* `NM_PER_MM` (an `f64` scale used by
//! the file-format rounding logic) — the on-disk wire format is the
//! only place nanometres are used; `crate::coord` no longer defines a
//! competing integer-nm coordinate type (#394).

const MM_PER_NM: f64 = 1.0e-6;
const NM_PER_MM: f64 = 1.0e6;

/// A document value that cannot be written to the wire format: NaN,
/// an infinity, or a length too large for an `i64` nanometre count.
/// Carries the row field's name; the writer adds the block and row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NonFiniteField(pub &'static str);

/// Convert mm to nm, refusing values the wire format cannot hold.
///
/// A NaN, an infinity, or a length beyond `i64` nanometres (~9.2e12 mm)
/// fails here instead of being clamped: a clamped coordinate saves
/// quietly and corrupts the drawing, a refusal fails the save before any
/// file is touched.
pub(in crate::format) fn mm_to_nm(mm: f64, field: &'static str) -> Result<i64, NonFiniteField> {
    let scaled = (mm * NM_PER_MM).round();
    if !scaled.is_finite() || scaled >= i64::MAX as f64 || scaled <= i64::MIN as f64 {
        return Err(NonFiniteField(field));
    }
    Ok(scaled as i64)
}

pub(in crate::format) fn nm_to_mm(nm: i64) -> f64 {
    (nm as f64) * MM_PER_NM
}
