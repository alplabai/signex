//! Bounding-box silk text (item ③). A drag-rect sets the frame; the
//! string is aligned/clipped inside it at render time
//! (`canvas/draw_silk.rs`). No auto-wrap/reflow.

use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind};

/// Append a framed silk string. `content` starts empty; the user edits it
/// via the existing text-edit flow after placement. `size` / `stroke_width`
/// match the `FootprintAddText` (Place String) defaults so a framed string
/// renders identically to an unframed one at the same font size.
pub fn add_text_frame(fp: &mut Footprint, x_mm: f64, y_mm: f64, w_mm: f64, h_mm: f64) {
    fp.silk_f.push(FpGraphic {
        kind: FpGraphicKind::Text {
            position: [x_mm, y_mm],
            content: String::new(),
            size: 1.0,
            frame: Some((w_mm as f32, h_mm as f32)),
        },
        stroke_width: 0.0,
        filled: false,
    });
}
