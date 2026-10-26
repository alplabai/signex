//! How wide a text is, in the face it is drawn in (#510).
//!
//! Hit boxes and label flags are sized here, the text inside them is shaped
//! by the renderer in the resolved family. Iosevka keeps the monospace
//! estimate the geometry was tuned with, so nothing that draws in the
//! default moves. Any other family is measured with the pipeline's own
//! shaper (`fonts::measure`); until that measurer exists (the startup scan
//! has not finished) the monospace estimate stands in.

use signex_gfx::primitive::text::MM_PER_EM;
use signex_types::font::BUNDLED_DEFAULT_FAMILY;

/// Width in mm of `text` drawn at glyph height `size_mm` in `family`
/// (`None`, or the bundled default, is Iosevka). `monospace_estimate_mm` is
/// the caller's existing estimate for the default face.
pub(super) fn text_width_mm(
    text: &str,
    family: Option<&str>,
    size_mm: f64,
    monospace_estimate_mm: f64,
) -> f64 {
    let Some(family) = family.filter(|f| !f.eq_ignore_ascii_case(BUNDLED_DEFAULT_FAMILY)) else {
        return monospace_estimate_mm;
    };
    match crate::fonts::measure::shaped_advance_em(text, family) {
        Some(em) => f64::from(em) * size_mm / f64::from(MM_PER_EM),
        None => monospace_estimate_mm,
    }
}
