//! Estimated drawn extent of a text graphic, shared by the symbol and
//! footprint editors' hit tests so they cannot drift apart.

/// Estimated glyph advance as a fraction of the text size (em). The
/// renderer shapes text itself and exposes no per-glyph metrics.
pub const TEXT_ADVANCE_EM: f64 = 0.6;

/// Estimated `(width, height)` in mm of `content` drawn at `size`:
/// the longest line sets the width, the line count the height.
pub fn text_extent(content: &str, size: f64) -> (f64, f64) {
    let longest = content
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let lines = content.lines().count().max(1);
    (longest as f64 * size * TEXT_ADVANCE_EM, lines as f64 * size)
}
