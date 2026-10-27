//! Cell helper shared by the symbol and footprint TSV codecs.

/// Format an `f64` for a TSV cell. `0.0` emits literally as `"0"` so
/// the most common default is short; non-zero values use the
/// shortest precision-preserving form via `Display`. Cells must be
/// re-parseable by `f64::from_str`.
///
/// `None` for NaN / ±Inf: `Display` would write `NaN` / `inf`, which
/// the reader refuses (#678), and an empty cell would silently reset the
/// value on reopen. The caller turns it into a save error naming the
/// pin or pad and column (#828).
pub(super) fn fmt_f64(v: f64) -> Option<String> {
    if v == 0.0 {
        Some("0".to_string())
    } else if !v.is_finite() {
        None
    } else {
        Some(format!("{v}"))
    }
}
