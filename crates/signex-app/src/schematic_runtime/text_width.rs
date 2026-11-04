//! How wide a text is, in the face it is drawn in (#510).
//!
//! Hit boxes and label flags are sized here, the text inside them is shaped
//! by the renderer in the resolved family. Every family, the bundled default
//! included (#2023), is measured with the pipeline's own shaper
//! (`fonts::measure`); until that measurer exists (the startup scan has not
//! finished) the caller's estimate stands in.

use signex_types::font::BUNDLED_DEFAULT_FAMILY;

/// Width in mm of `text` drawn at glyph height `size_mm` in `family`
/// (`None` is the bundled default, Iosevka). `monospace_estimate_mm` is the
/// caller's estimate, used until the shaper is ready.
pub(super) fn text_width_mm(
    text: &str,
    family: Option<&str>,
    size_mm: f64,
    monospace_estimate_mm: f64,
) -> f64 {
    let family = family.unwrap_or(BUNDLED_DEFAULT_FAMILY);
    match crate::fonts::measure::shaped_advance_em(text, family) {
        Some(em) => {
            f64::from(em) * size_mm / f64::from(signex_types::font::mm_per_em(Some(family)))
        }
        None => monospace_estimate_mm,
    }
}
