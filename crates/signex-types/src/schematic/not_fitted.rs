//! How a part that is not fitted draws (#1294): its body greyed toward the
//! paper and a cross over its body. The canvas, the SVG and the PDF read this
//! one rule, so they draw the same mark. Which parts are not fitted is
//! decided in `signex_output::variant_fit`. Rules: `docs/RENDERING_RULES.md`,
//! "sch-not-fitted".

use super::{LibSymbol, Point, Symbol, body_bounds};

/// The share of the paper colour mixed into a not-fitted part's ink.
pub const NOT_FITTED_PAPER_SHARE: f32 = 0.6;

/// The cross's stroke, as a multiple of the unset symbol line width.
pub const NOT_FITTED_CROSS_WIDTH_SCALE: f64 = 2.0;

/// `ink` greyed toward `paper` by [`NOT_FITTED_PAPER_SHARE`]; channels in
/// `0.0..=1.0`.
#[must_use]
pub fn greyed_ink(ink: [f32; 3], paper: [f32; 3]) -> [f32; 3] {
    let share = NOT_FITTED_PAPER_SHARE;
    [0_usize, 1, 2].map(|i| ink[i] + (paper[i] - ink[i]) * share)
}

/// The two strokes of the cross over `sym`'s body, corner to corner of the
/// body box (`body_bounds`: no pins, no text), in world mm. `None` when the
/// symbol draws no body; a symbol without library graphics already draws a
/// crossed placeholder.
#[must_use]
pub fn not_fitted_cross(lib: &LibSymbol, sym: &Symbol) -> Option<[(Point, Point); 2]> {
    let b = body_bounds(lib, sym)?;
    Some([
        (Point::new(b.min_x, b.min_y), Point::new(b.max_x, b.max_y)),
        (Point::new(b.min_x, b.max_y), Point::new(b.max_x, b.min_y)),
    ])
}

#[cfg(test)]
#[path = "not_fitted_tests.rs"]
mod tests;
