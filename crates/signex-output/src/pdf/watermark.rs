//! The PDF watermark: what it prints, where and how large.
//!
//! [`Watermark::placement`] is the one size and position rule; the export
//! and the dialog's live preview both draw from it. The rule is written up
//! in docs/PDF_EXPORT.md, "Watermark".

use super::MM_TO_PT;
use super::colour::ColourMap;
use super::font::{best_alias_for_text, font_for_alias, normalize_pdf_text, text_advance_pt};
use super::surface::PdfSurface;

/// The text a watermark prints.
#[derive(Debug, Clone, PartialEq)]
pub enum WatermarkText {
    Draft,
    Confidential,
    Custom(String),
}

impl WatermarkText {
    /// The printed words. The presets print in English whatever the UI
    /// language: PDF text is locale-invariant (docs/DECISIONS.md, 2026-11-07).
    pub fn printed(&self) -> &str {
        match self {
            WatermarkText::Draft => "DRAFT",
            WatermarkText::Confidential => "CONFIDENTIAL",
            WatermarkText::Custom(text) => text,
        }
    }
}

/// Which exported pages carry the watermark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatermarkPages {
    #[default]
    All,
    FirstOnly,
}

/// A text watermark drawn over every chosen page.
#[derive(Debug, Clone, PartialEq)]
pub struct Watermark {
    pub text: WatermarkText,
    /// 0 to 100; a larger value prints as 100.
    pub opacity_percent: u8,
    /// Counter-clockwise on the page.
    pub angle_deg: f64,
    pub colour_rgb: [u8; 3],
    pub pages: WatermarkPages,
    /// A smaller line under the text, e.g. "Rev B · 2026-11-18". The caller
    /// builds it: the exporter reads no clock.
    pub revision_line: Option<String>,
}

impl Default for Watermark {
    fn default() -> Self {
        Self {
            text: WatermarkText::Draft,
            opacity_percent: 18,
            angle_deg: 45.0,
            colour_rgb: [0xC0, 0x39, 0x2B],
            pages: WatermarkPages::All,
            revision_line: None,
        }
    }
}

/// Where and how large a watermark sits on one page, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WatermarkPlacement {
    /// The middle of the main text, from the page's top-left corner, y down.
    pub center_mm: (f64, f64),
    pub angle_deg: f64,
    /// Cap height of the main text.
    pub text_height_mm: f64,
    /// Cap height of the revision line.
    pub revision_height_mm: f64,
    /// Baseline-to-baseline distance from the main text to the revision line.
    pub revision_gap_mm: f64,
}

/// Largest main-text cap height, as a share of the page's short edge.
const MAX_TEXT_SHARE: f64 = 0.15;
/// Share of the line through the page centre, at the watermark angle,
/// the main text may span.
const LINE_SHARE: f64 = 0.8;
/// Revision cap height as a share of the main text's.
const REVISION_SHARE: f64 = 0.16;
/// Smallest revision cap height, so the line stays readable.
const MIN_REVISION_MM: f64 = 3.0;
/// Space between the main baseline and the revision line's cap top, as a
/// share of the main cap height.
const REVISION_LEADING: f64 = 0.3;
/// The face the watermark asks for first: the bold sans.
const PREFERRED_ALIAS: &str = "F2";
/// Cap height per em when a face does not state one.
const FALLBACK_CAP_RATIO: f64 = 0.7;

impl Watermark {
    /// Whether the page at `page_index` (0-based, among the exported pages)
    /// carries the watermark.
    pub fn applies_to_page(&self, page_index: usize) -> bool {
        match self.pages {
            WatermarkPages::All => true,
            WatermarkPages::FirstOnly => page_index == 0,
        }
    }

    /// Size and position on a `page_w_mm` x `page_h_mm` page: centred, as
    /// large as fits (see docs/PDF_EXPORT.md, "Watermark").
    pub fn placement(&self, page_w_mm: f64, page_h_mm: f64) -> WatermarkPlacement {
        let angle_deg = if self.angle_deg.is_finite() {
            self.angle_deg
        } else {
            0.0
        };
        let short_edge = page_w_mm.min(page_h_mm).max(0.0);
        let max_height = MAX_TEXT_SHARE * short_edge;
        let text = self.printed_text();
        let alias = face_alias(&text);
        // Width of the text per millimetre of cap height.
        let width_per_cap = em_advance(alias, &text) / cap_ratio(alias);
        let text_height_mm = if width_per_cap > 0.0 {
            max_height.min(LINE_SHARE * chord_mm(page_w_mm, page_h_mm, angle_deg) / width_per_cap)
        } else {
            max_height
        };
        let revision_height_mm = (text_height_mm * REVISION_SHARE)
            .max(MIN_REVISION_MM)
            .min(text_height_mm);
        WatermarkPlacement {
            center_mm: (page_w_mm / 2.0, page_h_mm / 2.0),
            angle_deg,
            text_height_mm,
            revision_height_mm,
            revision_gap_mm: text_height_mm * REVISION_LEADING + revision_height_mm,
        }
    }

    /// Fill opacity, 0.0 to 1.0.
    pub(super) fn alpha(&self) -> f32 {
        f32::from(self.opacity_percent.min(100)) / 100.0
    }

    /// The fill colour after the page's colour mode, mapped like any text.
    pub(super) fn ink(&self, colours: &ColourMap) -> (f32, f32, f32) {
        let [r, g, b] = self.colour_rgb.map(|c| f32::from(c) / 255.0);
        colours.map_stroke_bw(r, g, b)
    }

    fn printed_text(&self) -> String {
        normalize_pdf_text(self.text.printed())
    }

    /// Draw the watermark onto a page. It must be the page's last drawing:
    /// it changes the fill opacity inside `q ... Q`.
    pub(super) fn draw(
        &self,
        surface: &mut PdfSurface<'_>,
        page_w_mm: f64,
        page_h_mm: f64,
        colours: &ColourMap,
    ) {
        let placement = self.placement(page_w_mm, page_h_mm);
        let (r, g, b) = self.ink(colours);
        surface.raw_operator("q\n");
        surface.set_fill_alpha(self.alpha());
        surface.set_fill_color(r, g, b);
        let frame = TextFrame::new(&placement, page_h_mm);
        let main = self.printed_text();
        frame.line(
            surface,
            &main,
            placement.text_height_mm,
            placement.text_height_mm / 2.0,
        );
        if let Some(revision) = self.revision_line.as_deref() {
            let revision = normalize_pdf_text(revision);
            frame.line(
                surface,
                &revision,
                placement.revision_height_mm,
                placement.text_height_mm / 2.0 + placement.revision_gap_mm,
            );
        }
        surface.raw_operator("Q\n");
    }
}

/// The watermark's centre and direction in PDF points (y up).
struct TextFrame {
    centre: (f64, f64),
    angle_deg: f64,
    along: (f64, f64),
    up: (f64, f64),
}

impl TextFrame {
    fn new(placement: &WatermarkPlacement, page_h_mm: f64) -> Self {
        let (sin, cos) = placement.angle_deg.to_radians().sin_cos();
        Self {
            centre: (
                placement.center_mm.0 * MM_TO_PT,
                (page_h_mm - placement.center_mm.1) * MM_TO_PT,
            ),
            angle_deg: placement.angle_deg,
            along: (cos, sin),
            up: (-sin, cos),
        }
    }

    /// One line centred along the watermark, its baseline `below_mm` under
    /// the centre.
    fn line(&self, surface: &mut PdfSurface<'_>, text: &str, cap_mm: f64, below_mm: f64) {
        if text.is_empty() {
            return;
        }
        let alias = face_alias(text);
        let size_pt = cap_mm * MM_TO_PT / cap_ratio(alias);
        let half_width = f64::from(text_advance_pt(alias, text, size_pt as f32)) / 2.0;
        let below = below_mm * MM_TO_PT;
        let x = self.centre.0 - self.along.0 * half_width - self.up.0 * below;
        let y = self.centre.1 - self.along.1 * half_width - self.up.1 * below;
        surface.text_at_rotated(
            x as f32,
            y as f32,
            alias,
            size_pt as f32,
            text,
            self.angle_deg as f32,
        );
    }
}

/// The embedded face that shows `text`: the bold sans unless another face
/// covers more of it.
fn face_alias(text: &str) -> &'static str {
    best_alias_for_text(PREFERRED_ALIAS, text)
}

/// Advance of `text` per point of font size (em units).
fn em_advance(alias: &str, text: &str) -> f64 {
    const PROBE_PT: f32 = 1000.0;
    if text.is_empty() {
        return 0.0;
    }
    f64::from(text_advance_pt(alias, text, PROBE_PT)) / f64::from(PROBE_PT)
}

/// Cap height per em of the face behind `alias`.
fn cap_ratio(alias: &str) -> f64 {
    font_for_alias(alias)
        .face()
        .and_then(|face| {
            let upm = f64::from(face.units_per_em());
            let cap = f64::from(face.capital_height()?);
            (upm > 0.0 && cap > 0.0).then_some(cap / upm)
        })
        .unwrap_or(FALLBACK_CAP_RATIO)
}

/// Length of the line through the centre of a `w` x `h` page at
/// `angle_deg`, between the two edges it meets.
fn chord_mm(w: f64, h: f64, angle_deg: f64) -> f64 {
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    let across_width = if cos.abs() > 1e-9 {
        w / cos.abs()
    } else {
        f64::INFINITY
    };
    let across_height = if sin.abs() > 1e-9 {
        h / sin.abs()
    } else {
        f64::INFINITY
    };
    across_width.min(across_height).max(0.0)
}
