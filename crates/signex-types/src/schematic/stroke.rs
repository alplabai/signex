//! The one zoom-to-stroke law for schematic and symbol strokes (#1858).
//!
//! Every stroke is a world width in mm, drawn at `mm * scale` px, so the
//! ratios between stroke kinds never change with zoom. Only the minimum
//! visible width is in pixels, and it is the same for every kind. Output
//! (PDF/SVG/print) uses the mm widths alone. See docs/RENDERING_RULES.md,
//! "Stroke law".

/// Thinnest world width any schematic stroke lowers to, and the width a
/// graphic stored with width `0` ("default") draws at (mm).
pub const SCHEMATIC_RENDER_MIN_STROKE_MM: f64 = 0.15;

/// Bus stroke width (mm).
pub const SCHEMATIC_RENDER_BUS_STROKE_MM: f64 = 0.45;

/// No-connect X and No-ERC diamond stroke width (mm). This and the two below
/// keep the pre-#1858 look at 400 % zoom after the whole-pixel snap.
pub const SCHEMATIC_NO_CONNECT_STROKE_MM: f64 = 0.2;

/// Global / hierarchical label marker outline width (mm).
pub const SCHEMATIC_LABEL_MARKER_STROKE_MM: f64 = 0.15;

/// Child sheet frame outline width (mm).
pub const SCHEMATIC_SHEET_FRAME_STROKE_MM: f64 = 0.15;

/// Thinnest a schematic stroke draws on screen, in device pixels.
pub const SCHEMATIC_MIN_STROKE_DEVICE_PX: f32 = 0.6;

/// Device pixels per logical pixel for surfaces the window's display scale
/// does not reach yet: the floor and snap then work in logical pixels.
pub const UNSCALED_DEVICE_PX_PER_LOGICAL_PX: f32 = 1.0;

/// The world width a stroke stored as `width_mm` draws at: never thinner
/// than [`SCHEMATIC_RENDER_MIN_STROKE_MM`]; a non-finite width reads as `0`.
#[must_use]
pub fn effective_stroke_mm(width_mm: f64) -> f64 {
    let width = if width_mm.is_finite() { width_mm } else { 0.0 };
    width.max(SCHEMATIC_RENDER_MIN_STROKE_MM)
}

/// The shared floor in logical pixels on a surface with
/// `device_px_per_logical_px` device pixels per logical pixel.
#[must_use]
pub const fn min_stroke_logical_px(device_px_per_logical_px: f32) -> f32 {
    if device_px_per_logical_px.is_finite() && device_px_per_logical_px > 0.0 {
        SCHEMATIC_MIN_STROKE_DEVICE_PX / device_px_per_logical_px
    } else {
        SCHEMATIC_MIN_STROKE_DEVICE_PX
    }
}

/// The law: the logical-pixel width a `width_mm` stroke draws at under
/// `scale_px_per_mm`, raised to the shared floor. The whole-pixel snap of
/// axis-aligned hairlines runs after this.
#[must_use]
pub fn stroke_px(width_mm: f32, scale_px_per_mm: f32, device_px_per_logical_px: f32) -> f32 {
    (width_mm * scale_px_per_mm).max(min_stroke_logical_px(device_px_per_logical_px))
}

/// The symbol's library stroke width, which its pins draw at: the most
/// common effective width among its stroked graphics (the wider one on a
/// tie), or the default when it has none.
#[must_use]
pub fn symbol_stroke_mm(graphic_widths_mm: impl IntoIterator<Item = f64>) -> f64 {
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for width in graphic_widths_mm.into_iter().map(effective_stroke_mm) {
        match counts.iter_mut().find(|(w, _)| *w == width) {
            Some((_, n)) => *n += 1,
            None => counts.push((width, 1)),
        }
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(a.0.total_cmp(&b.0)))
        .map_or(SCHEMATIC_RENDER_MIN_STROKE_MM, |(width, _)| width)
}

#[cfg(test)]
#[path = "stroke_tests.rs"]
mod tests;
