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

/// HI-12: convert mm → nm without overflow. The unchecked `as i64` cast
/// would wrap to garbage for boards larger than ~9.2 m (and for any
/// NaN / Inf input). Real PCBs are < 1 m, so we clamp to `i64::MIN/MAX`
/// rather than panicking — that surfaces a non-finite value as the
/// largest representable coordinate, which is visibly wrong instead of
/// silently corrupt.
pub(in crate::format) fn mm_to_nm(mm: f64) -> i64 {
    let scaled = (mm * NM_PER_MM).round();
    if scaled.is_nan() {
        return 0;
    }
    if scaled >= i64::MAX as f64 {
        return i64::MAX;
    }
    if scaled <= i64::MIN as f64 {
        return i64::MIN;
    }
    scaled as i64
}

pub(in crate::format) fn nm_to_mm(nm: i64) -> f64 {
    (nm as f64) * MM_PER_NM
}
