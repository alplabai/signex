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
}

impl TextSizePolicy {
    pub const fn new(min_px: f32, max_px: f32) -> Self {
        Self { min_px, max_px }
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
    let em_mm = size_mm.max(0.1) / signex_types::font::mm_per_em(family);
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
}

/// Line height as a multiple of the em size, for every text replay.
///
/// 1.2 is `canvas::Text`'s own default (`iced_graphics` 0.14,
/// `geometry/text.rs`), which the CPU replay used implicitly — not iced's
/// widget-level `LineHeight` default of 1.3. The GPU pipeline hardcoded 1.35,
/// so multiline notes spread 12.5% taller there. Both now read this.
pub const LINE_HEIGHT_EM: f32 = 1.2;

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
