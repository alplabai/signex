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

/// Width a wire stored with width `0` ("default") is drawn and exported at
/// (mm): 10 mil, the width of the stock symbols' graphics and so of their
/// pins. Nothing is written to the file; an explicit width keeps its value.
pub const SCHEMATIC_DEFAULT_WIRE_STROKE_MM: f64 = 0.254;

/// Bus stroke width (mm).
pub const SCHEMATIC_RENDER_BUS_STROKE_MM: f64 = 0.45;

// A default wire stays visibly thinner than a bus, on the canvas and in exports.
const _: () = assert!(SCHEMATIC_DEFAULT_WIRE_STROKE_MM * 1.5 < SCHEMATIC_RENDER_BUS_STROKE_MM);

/// No-connect X and No-ERC diamond stroke width (mm). This and the two below
/// keep the pre-#1858 look at 400 % zoom after the whole-pixel snap.
pub const SCHEMATIC_NO_CONNECT_STROKE_MM: f64 = 0.2;

/// Global / hierarchical label marker outline width (mm).
pub const SCHEMATIC_LABEL_MARKER_STROKE_MM: f64 = 0.15;

/// Child sheet frame outline width (mm).
pub const SCHEMATIC_SHEET_FRAME_STROKE_MM: f64 = 0.15;

/// Thinnest a schematic stroke draws on screen, in device pixels. One whole
/// pixel: the CPU canvas has no anti-aliasing, so a thinner unsnapped outline
/// (a symbol body, a diagonal) can miss every pixel centre and vanish.
pub const SCHEMATIC_MIN_STROKE_DEVICE_PX: f32 = 1.0;

// Under one device pixel a 0.254 mm body at 100 % lost whole rectangle edges.
const _: () = assert!(SCHEMATIC_MIN_STROKE_DEVICE_PX >= 1.0);

/// Device pixels per logical pixel for a surface that has no window (tests,
/// headless draws): the floor and snap then work in logical pixels.
pub const UNSCALED_DEVICE_PX_PER_LOGICAL_PX: f32 = 1.0;

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

#[cfg(test)]
#[path = "stroke_tests.rs"]
mod tests;
