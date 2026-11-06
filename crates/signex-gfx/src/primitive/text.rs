//! Text primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

// [`TextItem::size_mm`] is a **world** quantity — the height the glyph should
// actually measure on the sheet (the schematic default is 1.27 mm = 50 mils =
// 10 Altium pt). Text renderers are sized in em, which is taller than the
// glyphs it carries, so a renderer divides by the face's cap height per em
// (`signex_types::font::mm_per_em`, #1229) or every label comes out the wrong
// height and an imported 10 pt label stops measuring 50 mils. [`text_px_in`]
// is the one place the canvas does it; never write a second copy of the
// ratio: that is how the two replay paths and the symbol editor's hit-test
// drifted apart.

/// Per-surface readability limits on rendered text, in **logical** pixels.
///
/// Not scene data: the same [`crate::scene::Scene`] is drawn by surfaces that
/// disagree about how small text may get before it stops being worth drawing
/// and how large it may grow before it stops being a schematic. The floor and
/// ceiling are a view decision, so the surface supplies them.
///
/// Apply these **before** multiplying by the display's DPI factor — clamping
/// after would move both thresholds by the scale factor on a HiDPI screen and
/// silently disagree with a surface that clamped first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSizePolicy {
    pub min_px: f32,
    pub max_px: f32,
    /// Largest em, in the policy's own pixels, the atlas pipeline draws; text
    /// past it is left to the canvas (see [`text_raster`]). Infinite: no
    /// limit, for surfaces whose [`Self::max_px`] already bounds the size.
    pub atlas_limit_px: f32,
}

impl TextSizePolicy {
    /// True size at every zoom: no floor, no ceiling.
    pub const UNCLAMPED: Self = Self::new(0.0, f32::INFINITY);

    pub const fn new(min_px: f32, max_px: f32) -> Self {
        Self {
            min_px,
            max_px,
            atlas_limit_px: f32::INFINITY,
        }
    }

    /// This policy with the atlas limit set.
    pub const fn with_atlas_limit(mut self, atlas_limit_px: f32) -> Self {
        self.atlas_limit_px = atlas_limit_px;
        self
    }
}

/// Text whose em is under this many device pixels is not drawn: it cannot be
/// seen and a zero-size glyph run is not worth shaping.
pub const MIN_VISIBLE_TEXT_DEVICE_PX: f32 = 0.5;

// shortcut: sanity ceiling on drawn text, derived from the largest real case
// rather than picked: the biggest text a sheet keeps on load
// (`MAX_TEXT_SIZE_MM`), over the smallest cap-height ratio `mm_per_em` can
// return (`MIN_MM_PER_EM`), at the canvas zoom limit and on a display scale of
// 4, doubled for headroom. Real text can therefore never reach it; past it the
// size is hostile data, not drawing, and the item is skipped.
pub const MAX_DRAWN_TEXT_DEVICE_PX: f32 = 2.0
    * (signex_types::format::MAX_TEXT_SIZE_MM as f32 / signex_types::font::MIN_MM_PER_EM)
    * (signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE * 16.0)
    * 4.0;

/// How a text of a given em size is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRaster {
    /// Not drawn: non-finite, invisibly small, or absurdly large.
    Skip,
    /// Drawn at true size from the glyph atlas (signex's GPU pipeline) or the
    /// canvas text path.
    Bitmap,
    /// Drawn at true size, but past the atlas: only the canvas draws it, and
    /// iced turns it into vector outlines.
    Oversize,
}

/// The one rule of the CPU canvas and the GPU glyph pipeline for an em of
/// `device_px`, with `atlas_limit_device_px` the largest em the atlas draws
/// (iced's `MAX_ATLAS_TEXT_SIZE` times the display scale). It never resizes
/// text, it only picks how (or whether) to draw.
pub fn text_raster(device_px: f32, atlas_limit_device_px: f32) -> TextRaster {
    if !device_px.is_finite()
        || !(MIN_VISIBLE_TEXT_DEVICE_PX..=MAX_DRAWN_TEXT_DEVICE_PX).contains(&device_px)
    {
        TextRaster::Skip
    } else if device_px > atlas_limit_device_px {
        TextRaster::Oversize
    } else {
        TextRaster::Bitmap
    }
}

/// Rendered em size in logical pixels for `size_mm` at `scale_px_per_mm`.
///
/// The single sizing rule: convert world millimetres to em through the
/// family's cap height ([`signex_types::font::mm_per_em`]), scale to pixels,
/// then clamp to the surface's readability policy. `family` `None` is the
/// canvas default face.
pub fn text_px_in(
    size_mm: f32,
    scale_px_per_mm: f32,
    policy: TextSizePolicy,
    family: Option<&str>,
) -> f32 {
    // `f32::max` swallows NaN; keep it so the raster guard culls the item.
    let size_mm = if size_mm.is_nan() {
        size_mm
    } else {
        size_mm.max(0.1)
    };
    let em_mm = size_mm / signex_types::font::mm_per_em(family);
    (em_mm * scale_px_per_mm).clamp(policy.min_px, policy.max_px)
}

/// [`text_px_in`] for the canvas default face.
pub fn text_px(size_mm: f32, scale_px_per_mm: f32, policy: TextSizePolicy) -> f32 {
    text_px_in(size_mm, scale_px_per_mm, policy, None)
}

/// Text item to be consumed by a text pipeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextHAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextVAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

#[cfg(test)]
mod size_tests {
    use super::*;

    /// The schematic's own numbers: 1.27 mm is 50 mils / 10 Altium pt
    /// (`signex_types::schematic::SCHEMATIC_TEXT_MM`) and 3.0 px/mm is 100%
    /// zoom (`SCHEMATIC_ZOOM_100_SCALE`).
    const SCHEMATIC: TextSizePolicy = TextSizePolicy::new(6.0, 64.0);

    #[test]
    fn default_schematic_text_sits_on_the_floor_at_100_percent() {
        // 1.27 / 0.735 = 1.728 em, x3.0 = 5.18 px — below the floor, so the
        // readability minimum is what the user actually sees.
        assert_eq!(text_px(1.27, 3.0, SCHEMATIC), 6.0);
    }

    #[test]
    fn zooming_out_cannot_shrink_text_below_the_floor() {
        // 25% zoom. Without the floor this would be 1.32 px — the smear the
        // GPU path used to render because it clamped at 1.0 instead.
        assert_eq!(text_px(1.27, 0.75, SCHEMATIC), 6.0);
    }

    #[test]
    fn between_the_bounds_the_size_is_the_em_conversion() {
        // 6.0 mm at 3.0 px/mm: 6.0 / 0.735 * 3.0 = 24.49 px, untouched by
        // either bound.
        assert!((text_px(6.0, 3.0, SCHEMATIC) - 24.489_796).abs() < 1e-3);
    }

    #[test]
    fn zooming_in_cannot_grow_text_past_the_ceiling() {
        assert_eq!(text_px(6.0, 100.0, SCHEMATIC), 64.0);
    }

    /// Treating `size_mm` as the em size — what the GPU pipeline did — is
    /// short by exactly the face's cap-height ratio, so a 10 pt import stopped measuring
    /// 50 mils.
    #[test]
    fn taking_size_mm_as_em_renders_short_by_the_ratio() {
        let policy = TextSizePolicy::new(0.0, f32::INFINITY);
        let correct = text_px(1.27, 3.0, policy);
        let as_if_em = 1.27 * 3.0;
        assert!((as_if_em / correct - signex_types::font::mm_per_em(None)).abs() < 1e-5);
    }

    /// The clamp is in logical pixels, so a surface working in physical
    /// pixels must scale both bounds rather than the result — the identity
    /// `scene_shader` relies on.
    #[test]
    fn scaling_both_bounds_matches_scaling_the_clamped_result() {
        let dpi = 2.0;
        let physical = text_px(
            1.27,
            3.0 * dpi,
            TextSizePolicy::new(SCHEMATIC.min_px * dpi, SCHEMATIC.max_px * dpi),
        );
        assert!((physical - text_px(1.27, 3.0, SCHEMATIC) * dpi).abs() < 1e-4);
    }

    /// #1229 — the same glyph height is a different em in a face with a
    /// different cap height, so capitals measure the same in both.
    #[test]
    fn capitals_measure_the_requested_height_in_every_bundled_face() {
        let policy = TextSizePolicy::new(0.0, f32::INFINITY);
        for family in ["Iosevka", "Roboto"] {
            let em_px = text_px_in(1.27, 10.0, policy, Some(family));
            let cap_px = em_px * signex_types::font::bundled_cap_height_per_em(family).unwrap();
            assert!((cap_px - 12.7).abs() < 0.127, "{family}: {cap_px}");
        }
        assert_ne!(
            text_px_in(1.27, 10.0, policy, Some("Iosevka")),
            text_px_in(1.27, 10.0, policy, Some("Roboto"))
        );
    }

    /// The atlas/CPU split: anything rotated, even a quarter turn written as
    /// a negative angle, is the CPU layer's.
    #[test]
    fn only_unrotated_items_are_axis_aligned() {
        let at = |rotation| super::TextItem {
            rotation,
            ..Default::default()
        };
        assert!(at(0.0).is_axis_aligned());
        assert!(at(-0.0).is_axis_aligned());
        assert!(!at(std::f32::consts::FRAC_PI_2).is_axis_aligned());
        assert!(!at(-std::f32::consts::FRAC_PI_2).is_axis_aligned());
    }

    /// The atlas limit a test passes: iced's own constant is imported in the
    /// app crate, where iced is available.
    const ATLAS: f32 = 1024.0;

    #[test]
    fn the_raster_rule_skips_only_unreachable_sizes() {
        assert_eq!(text_raster(1.8, ATLAS), TextRaster::Bitmap);
        assert_eq!(text_raster(ATLAS, ATLAS), TextRaster::Bitmap);
        assert_eq!(text_raster(ATLAS + 1.0, ATLAS), TextRaster::Oversize);
        assert_eq!(text_raster(5224.0, ATLAS), TextRaster::Oversize);
        assert_eq!(text_raster(f32::NAN, ATLAS), TextRaster::Skip);
        assert_eq!(text_raster(f32::INFINITY, ATLAS), TextRaster::Skip);
        assert_eq!(text_raster(0.1, ATLAS), TextRaster::Skip);
        assert_eq!(
            text_raster(MAX_DRAWN_TEXT_DEVICE_PX, ATLAS),
            TextRaster::Oversize
        );
        assert_eq!(
            text_raster(MAX_DRAWN_TEXT_DEVICE_PX * 1.01, ATLAS),
            TextRaster::Skip
        );
        assert_eq!(text_raster(1.0e9, ATLAS), TextRaster::Skip);
    }

    /// 40 mm text at 96 px/mm is about 5224 px: past the atlas, still drawn at
    /// that size (never clamped to the ceiling, never culled).
    #[test]
    fn text_past_the_atlas_ceiling_is_drawn_at_true_size_not_clamped_or_culled() {
        let px = text_px_in(40.0, 96.0, TextSizePolicy::UNCLAMPED, None);
        assert!((px - 5224.0).abs() < 5.0, "{px}");
        assert_eq!(text_raster(px, ATLAS), TextRaster::Oversize);
    }

    #[test]
    fn a_hostile_file_size_is_culled_and_normal_zoom_is_untouched() {
        let policy = TextSizePolicy::UNCLAMPED;
        for size_mm in [f32::NAN, f32::INFINITY, 1.0e30] {
            let px = text_px_in(size_mm, 48.0, policy, None);
            assert_eq!(
                text_raster(px, ATLAS),
                TextRaster::Skip,
                "{size_mm} -> {px}"
            );
        }
        // 1.27 mm text from fit-all to the zoom limit is always drawn.
        for scale in [0.75_f32, 1.75, 3.0, 48.0] {
            assert_eq!(
                text_raster(text_px_in(1.27, scale, policy, None), ATLAS),
                TextRaster::Bitmap
            );
        }
    }

    /// Many absurd file-supplied sizes at the zoom limit on a 3x display cost
    /// nothing: every one is skipped before any shaping or tessellation, and
    /// the largest real text still draws.
    #[test]
    fn a_hostile_file_of_huge_texts_is_skipped_at_max_zoom() {
        let (zoom_limit, dpr) = (48.0, 3.0);
        let hostile = (0..10_000)
            .filter(|_| {
                text_raster(
                    text_px_in(1.0e5, zoom_limit, TextSizePolicy::UNCLAMPED, None) * dpr,
                    ATLAS,
                ) != TextRaster::Skip
            })
            .count();
        assert_eq!(hostile, 0);
        let largest_real = text_px_in(100.0, zoom_limit, TextSizePolicy::UNCLAMPED, None) * dpr;
        assert_eq!(
            text_raster(largest_real, ATLAS),
            TextRaster::Oversize,
            "{largest_real}"
        );
    }
}

/// Line height as a multiple of the em size, for every text replay.
///
/// 1.2 is `canvas::Text`'s own default (`iced_graphics` 0.14,
/// `geometry/text.rs`), which the CPU replay used implicitly — not iced's
/// widget-level `LineHeight` default of 1.3. The GPU pipeline hardcoded 1.35,
/// so multiline notes spread 12.5% taller there. Both now read this.
pub const LINE_HEIGHT_EM: f32 = signex_types::font::TEXT_LINE_HEIGHT_EM;

#[derive(Clone, Debug, Default)]
pub struct TextItem {
    pub content: String,
    pub position: [f32; 2],
    pub size_mm: f32,
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    /// Font family for this item, already checked against the installed
    /// fonts by the caller. `None` draws in the canvas default.
    ///
    /// Both replays read it through [`TextItem::family_or`], so the CPU
    /// canvas and the GPU pipeline cannot pick different faces for the
    /// same item.
    pub family: Option<std::sync::Arc<str>>,
    pub rotation: f32,
    pub h_align: TextHAlign,
    pub v_align: TextVAlign,
}

impl TextItem {
    /// Whether signex's GPU glyph pipeline stages this item, drawn at an em of
    /// `device_px` with `atlas_limit_device_px` the largest the atlas takes:
    /// axis-aligned, and a size [`text_raster`] draws as a bitmap. The one
    /// rule the pipeline stages by and the canvas overlay leaves to it.
    pub fn gpu_stages(&self, device_px: f32, atlas_limit_device_px: f32) -> bool {
        self.is_axis_aligned()
            && text_raster(device_px, atlas_limit_device_px) == TextRaster::Bitmap
    }

    /// The family this item draws in: its own, else `default`.
    pub fn family_or<'a>(&'a self, default: &'a str) -> &'a str {
        self.family.as_deref().unwrap_or(default)
    }

    /// Whether this item can go through a glyph atlas at all.
    ///
    /// A cryoglyph `TextArea` is `{left, top, scale}` — it cannot rotate, so
    /// the GPU pipeline used to rotate only the alignment offset and draw the
    /// glyph run axis-aligned. Rotated items belong on the CPU canvas layer
    /// instead, and that costs no parity: `iced_wgpu` 0.14 sends any text
    /// under a non scale-translation transform through outline tessellation,
    /// never its atlas, so a rotated label was always vector fill (#645).
    ///
    /// The one predicate both replays use to decide which side draws an item.
    pub fn is_axis_aligned(&self) -> bool {
        self.rotation.abs() < f32::EPSILON
    }
}
