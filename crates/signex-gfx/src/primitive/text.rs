//! Text primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Millimetres of rendered glyph height per em.
///
/// [`TextItem::size_mm`] is a **world** quantity — the height the glyph should
/// actually measure on the sheet (the schematic default is 1.27 mm = 50 mils =
/// 10 Altium pt). Text renderers are sized in em, which is taller than the
/// glyphs it carries, so a renderer must divide by this ratio or every label
/// comes out short and an imported 10 pt label stops measuring 50 mils.
///
/// One constant, because a second copy is how the two replay paths and the
/// symbol editor's hit-test drifted apart.
pub const MM_PER_EM: f32 = 0.72;

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
/// The single sizing rule: convert world millimetres to em via [`MM_PER_EM`],
/// scale to pixels, then clamp to the surface's readability policy.
pub fn text_px(size_mm: f32, scale_px_per_mm: f32, policy: TextSizePolicy) -> f32 {
    let em_mm = size_mm.max(0.1) / MM_PER_EM;
    (em_mm * scale_px_per_mm).clamp(policy.min_px, policy.max_px)
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
        // 1.27 / 0.72 = 1.7639 em, x3.0 = 5.29 px — below the floor, so the
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
        // 6.0 mm at 3.0 px/mm: 6.0 / 0.72 * 3.0 = 25.0 px, untouched by
        // either bound.
        assert!((text_px(6.0, 3.0, SCHEMATIC) - 25.0).abs() < 1e-4);
    }

    #[test]
    fn zooming_in_cannot_grow_text_past_the_ceiling() {
        assert_eq!(text_px(6.0, 100.0, SCHEMATIC), 64.0);
    }

    /// Treating `size_mm` as the em size — what the GPU pipeline did — is
    /// short by exactly `MM_PER_EM`, so a 10 pt import stopped measuring
    /// 50 mils.
    #[test]
    fn taking_size_mm_as_em_renders_short_by_the_ratio() {
        let policy = TextSizePolicy::new(0.0, f32::INFINITY);
        let correct = text_px(1.27, 3.0, policy);
        let as_if_em = 1.27 * 3.0;
        assert!((as_if_em / correct - MM_PER_EM).abs() < 1e-5);
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
}

#[derive(Clone, Debug, Default)]
pub struct TextItem {
    pub content: String,
    pub position: [f32; 2],
    pub size_mm: f32,
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub rotation: f32,
    pub h_align: TextHAlign,
    pub v_align: TextVAlign,
}
