//! `PdfSurface` — the second render target for the schematic scene graph.
//!
//! Emits PDF operators for:
//! - `stroke_line(x1, y1, x2, y2, width_pt)` — `m` + `l` + `S`
//! - `stroke_rect(x, y, w, h, width_pt)` — `re` + `S`
//! - `fill_rect(x, y, w, h, rgb)` — `re` + `f`
//! - `text_at(x, y, font_name, size_pt, text)` — `BT` + `Tf` + `Td` + `Tj` + `ET`
//! - `set_stroke_color(r, g, b)` — `RG`
//!
//! Tracks current stroke colour/width to avoid redundant ops. Coordinates in
//! PDF points (bottom-left origin).

/// `PdfSurface` emits PDF content-stream operators into a buffer.
pub struct PdfSurface {
    bytes: Vec<u8>,
    current_stroke_r: f32,
    current_stroke_g: f32,
    current_stroke_b: f32,
    current_fill_r: f32,
    current_fill_g: f32,
    current_fill_b: f32,
    current_stroke_width: f32,
}

impl PdfSurface {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            current_stroke_r: 0.0,
            current_stroke_g: 0.0,
            current_stroke_b: 0.0,
            current_fill_r: 0.0,
            current_fill_g: 0.0,
            current_fill_b: 0.0,
            current_stroke_width: 0.0,
        }
    }

    /// Finish and return the encoded Content stream bytes.
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    /// Set stroke color (0.0-1.0 per channel). Emits `RG` operator only if changed.
    pub fn set_stroke_color(&mut self, r: f32, g: f32, b: f32) {
        if (self.current_stroke_r - r).abs() > 1e-5
            || (self.current_stroke_g - g).abs() > 1e-5
            || (self.current_stroke_b - b).abs() > 1e-5
        {
            self.write_operator(&format!("{} {} {} RG\n", r, g, b));
            self.current_stroke_r = r;
            self.current_stroke_g = g;
            self.current_stroke_b = b;
        }
    }

    /// Set non-stroking color (fill/text). Emits `rg` only if changed.
    pub fn set_fill_color(&mut self, r: f32, g: f32, b: f32) {
        if (self.current_fill_r - r).abs() > 1e-5
            || (self.current_fill_g - g).abs() > 1e-5
            || (self.current_fill_b - b).abs() > 1e-5
        {
            self.write_operator(&format!("{} {} {} rg\n", r, g, b));
            self.current_fill_r = r;
            self.current_fill_g = g;
            self.current_fill_b = b;
        }
    }

    /// Set stroke width (in points). Emits `w` operator only if changed.
    pub fn set_stroke_width(&mut self, width: f32) {
        if (self.current_stroke_width - width).abs() > 1e-5 {
            self.write_operator(&format!("{} w\n", width));
            self.current_stroke_width = width;
        }
    }

    /// Emit a raw operator string into the content stream.
    pub fn raw_operator(&mut self, op: &str) {
        self.write_operator(op);
    }

    /// Stroke a line from (x1, y1) to (x2, y2).
    #[allow(dead_code)]
    pub fn stroke_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width_pt: f32) {
        self.set_stroke_width(width_pt);
        self.write_operator(&format!("{} {} m\n", x1, y1));
        self.write_operator(&format!("{} {} l\n", x2, y2));
        self.write_operator("S\n");
    }

    /// Stroke a rectangle outline (top-left at (x, y), width w, height h).
    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, width_pt: f32) {
        self.set_stroke_width(width_pt);
        self.write_operator(&format!("{} {} {} {} re\n", x, y, w, h));
        self.write_operator("S\n");
    }

    /// Fill a rectangle (top-left at (x, y), width w, height h) with RGB color.
    #[allow(dead_code)] // Reserved for v0.9 template backgrounds / fills.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, g: f32, b: f32) {
        self.write_operator(&format!("{} {} {} rg\n", r, g, b));
        self.write_operator(&format!("{} {} {} {} re\n", x, y, w, h));
        self.write_operator("f\n");
    }

    /// Emit text at (x, y) with given font, size (pt), and string.
    /// Font name should match the resource dictionary (e.g. "F1" for the first registered font).
    pub fn text_at(&mut self, x: f32, y: f32, font_name: &str, size_pt: f32, text: &str) {
        let escaped = escape_pdf_string(text);
        self.write_operator("BT\n");
        self.write_operator(&format!("/{} {} Tf\n", font_name, size_pt));
        self.write_operator(&format!("{} {} Td\n", x, y));
        self.write_operator(&format!("({}) Tj\n", escaped));
        self.write_operator("ET\n");
    }

    /// Emit rotated text at (x, y) with a text matrix.
    pub fn text_at_rotated(
        &mut self,
        x: f32,
        y: f32,
        font_name: &str,
        size_pt: f32,
        text: &str,
        rotation_deg: f32,
    ) {
        let escaped = escape_pdf_string(text);
        let rad = rotation_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();

        self.write_operator("BT\n");
        self.write_operator(&format!("/{} {} Tf\n", font_name, size_pt));
        self.write_operator(&format!("{} {} {} {} {} {} Tm\n", cos, sin, -sin, cos, x, y));
        self.write_operator(&format!("({}) Tj\n", escaped));
        self.write_operator("ET\n");
    }

    /// Write raw PDF operator bytes.
    fn write_operator(&mut self, op: &str) {
        self.bytes.extend_from_slice(op.as_bytes());
    }
}

impl Default for PdfSurface {
    fn default() -> Self {
        Self::new()
    }
}

/// Escape a string for use in PDF string literals (minimal escaping).
fn escape_pdf_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_pdf_string_handles_special_chars() {
        assert_eq!(escape_pdf_string("hello"), "hello");
        assert_eq!(escape_pdf_string("(world)"), "\\(world\\)");
        assert_eq!(escape_pdf_string("back\\slash"), "back\\\\slash");
        assert_eq!(
            escape_pdf_string("(back\\slash)"),
            "\\(back\\\\slash\\)"
        );
    }

    #[test]
    fn surface_tracks_stroke_color() {
        let mut surf = PdfSurface::new();
        surf.set_stroke_color(1.0, 0.0, 0.0);
        // Verify internal state is updated.
        assert!((surf.current_stroke_r - 1.0).abs() < 1e-5);
        assert!((surf.current_stroke_g - 0.0).abs() < 1e-5);
        assert!((surf.current_stroke_b - 0.0).abs() < 1e-5);
    }
}
