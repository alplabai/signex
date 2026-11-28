//! `PdfSurface` — the second render target for the schematic scene graph.
//!
//! Emits PDF operators for:
//! - `stroke_line(x1, y1, x2, y2, width_pt)` — `m` + `l` + `S`
//! - `stroke_rect(x, y, w, h, width_pt)` — `re` + `S`
//! - `fill_rect(x, y, w, h, rgb)` — `re` + `f`
//! - `text_at(x, y, font_name, size_pt, text)` — `BT` + `Tf` + `Td` + `Tj` + `ET`
//!   (text is shown as 2-byte glyph ids for the Identity-H Type0 fonts)
//! - `set_stroke_color(r, g, b)` — `RG`
//! - `set_stroke_alpha(alpha)` — `gs` with a page `/ExtGState` holding `/CA`
//! - `set_fill_alpha(alpha)` — `gs` with a page `/ExtGState` holding `/ca`
//!
//! Tracks current stroke colour/width/alpha to avoid redundant ops.
//! Coordinates in PDF points (bottom-left origin).

use std::collections::BTreeSet;

use pdf_writer::Name;
use pdf_writer::writers::{ExtGraphicsState, Resources};

use super::embed::FontUsage;
use super::font::font_for_alias;

/// An RGB colour (each channel 0.0-1.0). Groups the three channels that
/// travel together as a single fill colour — built at the `fill_rect` call
/// site (currently reserved for the v0.9 template backgrounds/fills) instead
/// of passed as three bare floats.
pub struct RgbColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// The stroke opacity a PDF draws with by default.
const OPAQUE_LEVEL: u8 = u8::MAX;

/// A finished page: its content stream and the stroke- and fill-alpha
/// levels the stream names with `gs`, which the page's `/ExtGState` must
/// define. `links` are the text links the page drew (#1298); the stream
/// itself carries none.
pub struct PageContent {
    pub bytes: Vec<u8>,
    pub stroke_alphas: BTreeSet<u8>,
    pub fill_alphas: BTreeSet<u8>,
    pub links: Vec<super::links::PageLink>,
    /// The pictures the stream paints, `/Im<index>` in this order (#1116).
    pub images: Vec<std::sync::Arc<crate::svg::SvgPicture>>,
}

/// The `/XObject` resource name of a page's picture `index`, e.g. `Im0`.
pub(super) fn image_name(index: usize) -> String {
    format!("Im{index}")
}

/// Stroke alpha (0.0-1.0) as a 0-255 level, the precision a wire colour has.
fn alpha_level(alpha: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The `/ExtGState` resource name of a stroke-alpha level, e.g. `CA128`.
fn stroke_alpha_name(level: u8) -> String {
    format!("CA{level}")
}

/// The `/ExtGState` resource name of a fill-alpha level, e.g. `ca46`.
fn fill_alpha_name(level: u8) -> String {
    format!("ca{level}")
}

/// Define one `/ExtGState` entry per stroke- and fill-alpha level a page
/// uses: the stroke levels first, then the fill levels.
pub(super) fn write_alpha_states(resources: &mut Resources<'_>, content: &PageContent) {
    if content.stroke_alphas.is_empty() && content.fill_alphas.is_empty() {
        return;
    }
    let mut states = resources.ext_g_states();
    for &level in &content.stroke_alphas {
        let name = stroke_alpha_name(level);
        states
            .insert(Name(name.as_bytes()))
            .start::<ExtGraphicsState<'_>>()
            .stroking_alpha(f32::from(level) / 255.0);
    }
    for &level in &content.fill_alphas {
        let name = fill_alpha_name(level);
        states
            .insert(Name(name.as_bytes()))
            .start::<ExtGraphicsState<'_>>()
            .non_stroking_alpha(f32::from(level) / 255.0);
    }
}

/// `PdfSurface` emits PDF content-stream operators into a buffer.
pub struct PdfSurface<'a> {
    bytes: Vec<u8>,
    fonts: &'a mut FontUsage,
    current_stroke_alpha: u8,
    stroke_alphas: BTreeSet<u8>,
    current_fill_alpha: u8,
    fill_alphas: BTreeSet<u8>,
    current_stroke_r: f32,
    current_stroke_g: f32,
    current_stroke_b: f32,
    current_fill_r: f32,
    current_fill_g: f32,
    current_fill_b: f32,
    current_stroke_width: f32,
    images: Vec<std::sync::Arc<crate::svg::SvgPicture>>,
}

impl<'a> PdfSurface<'a> {
    /// New surface that registers every shown glyph in `fonts`.
    pub fn new(fonts: &'a mut FontUsage) -> Self {
        Self {
            bytes: Vec::new(),
            fonts,
            current_stroke_alpha: OPAQUE_LEVEL,
            stroke_alphas: BTreeSet::new(),
            current_fill_alpha: OPAQUE_LEVEL,
            fill_alphas: BTreeSet::new(),
            current_stroke_r: 0.0,
            current_stroke_g: 0.0,
            current_stroke_b: 0.0,
            current_fill_r: 0.0,
            current_fill_g: 0.0,
            current_fill_b: 0.0,
            current_stroke_width: 0.0,
            images: Vec::new(),
        }
    }

    /// Paint `picture` stretched over the box with its bottom-left corner at
    /// (`x`, `y`) and size `w` × `h` (points), at `opacity`. One picture of
    /// the same file is one `/XObject`, however often it is painted.
    pub fn draw_image(
        &mut self,
        picture: &std::sync::Arc<crate::svg::SvgPicture>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        opacity: f32,
    ) {
        let index = match self
            .images
            .iter()
            .position(|known| std::sync::Arc::ptr_eq(&known.bytes, &picture.bytes))
        {
            Some(index) => index,
            None => {
                self.images.push(std::sync::Arc::clone(picture));
                self.images.len() - 1
            }
        };
        // Set before `q`, so `Q` leaves the tracked alpha true.
        self.set_fill_alpha(opacity);
        self.write_operator(&format!(
            "q\n{w} 0 0 {h} {x} {y} cm\n/{} Do\nQ\n",
            image_name(index)
        ));
    }

    /// Finish and return the encoded Content stream bytes.
    /// A stream that set a stroke or fill alpha must use `finish_page()`
    /// instead.
    pub fn finish(self) -> Vec<u8> {
        debug_assert!(
            self.stroke_alphas.is_empty() && self.fill_alphas.is_empty(),
            "alpha set on a surface finished with finish(); use finish_page()"
        );
        self.bytes
    }

    /// Finish a page that may draw with alpha: the stream plus the
    /// `/ExtGState` levels it names.
    pub fn finish_page(self) -> PageContent {
        PageContent {
            bytes: self.bytes,
            stroke_alphas: self.stroke_alphas,
            fill_alphas: self.fill_alphas,
            links: Vec::new(),
            images: self.images,
        }
    }

    /// Set fill (and text) opacity (0.0-1.0). Emits `gs` only if the level
    /// changed.
    pub fn set_fill_alpha(&mut self, alpha: f32) {
        let level = alpha_level(alpha);
        if level != self.current_fill_alpha {
            self.write_operator(&format!("/{} gs\n", fill_alpha_name(level)));
            self.fill_alphas.insert(level);
            self.current_fill_alpha = level;
        }
    }

    /// Set stroke opacity (0.0-1.0). Emits `gs` only if the level changed.
    pub fn set_stroke_alpha(&mut self, alpha: f32) {
        let level = alpha_level(alpha);
        if level != self.current_stroke_alpha {
            self.write_operator(&format!("/{} gs\n", stroke_alpha_name(level)));
            self.stroke_alphas.insert(level);
            self.current_stroke_alpha = level;
        }
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
    pub fn stroke_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width_pt: f32) {
        self.set_stroke_width(width_pt);
        self.write_operator(&format!("{} {} m\n", x1, y1));
        self.write_operator(&format!("{} {} l\n", x2, y2));
        self.write_operator("S\n");
    }

    /// Fill a rectangle (top-left at (x, y), width w, height h) with RGB color.
    #[expect(
        dead_code,
        reason = "reserved for the v0.9 template backgrounds / fills"
    )]
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: RgbColor) {
        self.write_operator(&format!("{} {} {} rg\n", color.r, color.g, color.b));
        self.write_operator(&format!("{} {} {} {} re\n", x, y, w, h));
        self.write_operator("f\n");
    }

    /// Emit text at (x, y) with given font, size (pt), and string.
    /// Font name should match the resource dictionary (e.g. "F1" for the first registered font).
    pub fn text_at(&mut self, x: f32, y: f32, font_name: &str, size_pt: f32, text: &str) {
        let shown = self.fonts.encode(font_for_alias(font_name), text);
        self.write_operator("BT\n");
        self.write_operator(&format!("/{} {} Tf\n", font_name, size_pt));
        self.write_operator(&format!("{} {} Td\n", x, y));
        self.write_operator(&format!("{} Tj\n", shown));
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
        let shown = self.fonts.encode(font_for_alias(font_name), text);
        let rad = rotation_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();

        self.write_operator("BT\n");
        self.write_operator(&format!("/{} {} Tf\n", font_name, size_pt));
        self.write_operator(&format!(
            "{} {} {} {} {} {} Tm\n",
            cos, sin, -sin, cos, x, y
        ));
        self.write_operator(&format!("{} Tj\n", shown));
        self.write_operator("ET\n");
    }

    /// Write raw PDF operator bytes.
    fn write_operator(&mut self, op: &str) {
        self.bytes.extend_from_slice(op.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_shown_as_two_byte_glyph_ids() {
        let mut fonts = FontUsage::new();
        let mut surf = PdfSurface::new(&mut fonts);
        surf.text_at(0.0, 0.0, "F1", 9.0, "a(b)");
        let ops = String::from_utf8(surf.finish()).unwrap();
        assert!(ops.contains("<0001000200030004> Tj"), "{ops}");
    }

    #[test]
    fn surface_tracks_stroke_color() {
        let mut fonts = FontUsage::new();
        let mut surf = PdfSurface::new(&mut fonts);
        surf.set_stroke_color(1.0, 0.0, 0.0);
        // Verify internal state is updated.
        assert!((surf.current_stroke_r - 1.0).abs() < 1e-5);
        assert!((surf.current_stroke_g - 0.0).abs() < 1e-5);
        assert!((surf.current_stroke_b - 0.0).abs() < 1e-5);
    }

    #[test]
    fn stroke_alpha_switches_graphics_state_only_on_change() {
        let mut fonts = FontUsage::new();
        let mut surf = PdfSurface::new(&mut fonts);
        surf.set_stroke_alpha(1.0);
        surf.set_stroke_alpha(128.0 / 255.0);
        surf.set_stroke_alpha(128.0 / 255.0);
        surf.set_stroke_alpha(1.0);
        let page = surf.finish_page();
        let ops = String::from_utf8(page.bytes).unwrap();
        assert_eq!(ops, "/CA128 gs\n/CA255 gs\n", "opaque is the default state");
        assert_eq!(
            page.stroke_alphas.into_iter().collect::<Vec<_>>(),
            [128, 255]
        );
    }

    #[test]
    fn fill_alpha_names_its_own_graphics_state() {
        let mut fonts = FontUsage::new();
        let mut surf = PdfSurface::new(&mut fonts);
        surf.set_fill_alpha(0.18);
        surf.set_fill_alpha(0.18);
        let page = surf.finish_page();
        assert_eq!(String::from_utf8(page.bytes).unwrap(), "/ca46 gs\n");
        assert_eq!(page.fill_alphas.into_iter().collect::<Vec<_>>(), [46]);
        assert!(page.stroke_alphas.is_empty());
    }

    #[test]
    fn an_opaque_page_defines_no_graphics_state() {
        let mut fonts = FontUsage::new();
        let mut surf = PdfSurface::new(&mut fonts);
        surf.set_stroke_alpha(1.0);
        let page = surf.finish_page();
        assert!(page.bytes.is_empty());
        assert!(page.stroke_alphas.is_empty());
    }
}
