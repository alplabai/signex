//! Label + field text placement helpers.
//!
//! Label font sizing, per-label colour, the spin/justify geometry that
//! positions net / global / hierarchical labels, and the `HAlign`/
//! `VAlign` → SVG alignment converters shared with note and field text.
//!
//! Extracted verbatim from the SVG exporter (`svg/mod.rs`); pure code
//! motion, zero behaviour change.

use super::*;
use crate::pdf::PdfScale;
use crate::pdf::palette::SchematicPalette;
use signex_types::markup::{RichSegment, parse_signex_markup};
use signex_types::schematic::{HAlign, LabelType, VAlign};

pub(super) fn label_size_pt(
    font_size_mm: f64,
    mm_to_unit: f64,
    scale: &PdfScale,
    font_alias: &str,
) -> f32 {
    if font_size_mm > 0.0 {
        super::text::em_for_glyph_height(font_alias, (font_size_mm * mm_to_unit) as f32)
    } else {
        match scale {
            PdfScale::OneToOne | PdfScale::FitToPage | PdfScale::Percent(_) => 9.0,
        }
    }
}

pub(super) fn label_colour(label_type: LabelType, palette: &SchematicPalette) -> (f32, f32, f32) {
    match label_type {
        LabelType::Net => palette.net_label,
        LabelType::Global => palette.global_label,
        LabelType::Hierarchical => palette.hier_label,
        LabelType::Power => palette.power_label,
    }
}

#[derive(Clone, Copy)]
pub(super) enum SpinStyle {
    Left,
    Right,
    Up,
    Bottom,
}

pub(super) fn label_spin_style(justify: HAlign, rotation: f64) -> SpinStyle {
    let rot = normalize_rotation(rotation);
    let vertical = rot == 90 || rot == 270;

    if vertical {
        if matches!(justify, HAlign::Right) {
            SpinStyle::Bottom
        } else {
            SpinStyle::Up
        }
    } else if matches!(justify, HAlign::Right) {
        SpinStyle::Left
    } else {
        SpinStyle::Right
    }
}

pub(super) fn schematic_text_offset_net(spin: SpinStyle) -> (f64, f64) {
    let dist = 0.15;
    match spin {
        SpinStyle::Up | SpinStyle::Bottom => (-dist, 0.0),
        SpinStyle::Left | SpinStyle::Right => (0.0, -dist),
    }
}

pub(super) fn schematic_text_offset_hier(
    text: &str,
    font_size_mm: f64,
    spin: SpinStyle,
) -> (f64, f64) {
    let unit = label_offset_unit_mm(font_size_mm);
    let dist = unit * 0.4
        + (parse_signex_markup(&normalize_standard_text(text))
            .iter()
            .map(|seg| match seg {
                RichSegment::Normal(t)
                | RichSegment::Bold(t)
                | RichSegment::Italic(t)
                | RichSegment::Strike(t)
                | RichSegment::Subscript(t)
                | RichSegment::Superscript(t)
                | RichSegment::Overbar(t) => t.chars().count(),
                RichSegment::Link { label, .. } => label.chars().count(),
            })
            .sum::<usize>() as f64)
            * unit
            * 0.6;
    match spin {
        SpinStyle::Left => (-dist, 0.0),
        SpinStyle::Up => (0.0, -dist),
        SpinStyle::Right => (dist, 0.0),
        SpinStyle::Bottom => (0.0, dist),
    }
}

/// The glyph height label offsets are measured in: the label's own size, and
/// never below the schematic default, as the canvas sizes its flags. The
/// offsets used to be fixed at the default, so a larger label's text ran
/// into the flag it sits beside (#1229).
pub(super) fn label_offset_unit_mm(font_size_mm: f64) -> f64 {
    font_size_mm.max(signex_types::schematic::SCHEMATIC_TEXT_MM)
}

pub(super) fn schematic_text_offset_global(
    shape: &str,
    spin: SpinStyle,
    font_size_mm: f64,
) -> (f64, f64) {
    let unit = label_offset_unit_mm(font_size_mm);
    let mut horiz = unit * 0.5;
    let vert = unit * 0.0715;

    if matches!(shape, "input" | "bidirectional" | "tri_state") {
        horiz += unit * 0.75;
    }

    match spin {
        SpinStyle::Left => (-horiz, vert),
        SpinStyle::Up => (vert, -horiz),
        SpinStyle::Right => (horiz, vert),
        SpinStyle::Bottom => (vert, horiz),
    }
}

/// Alignment and rotation of label text for `spin`. Vertical text always
/// reads bottom-to-top (-90° in the y-down page frame), as on the canvas
/// (#1111): a label that runs down from its anchor is drawn turned half a
/// revolution with both alignments swapped, which covers the same strip of
/// the page as top-to-bottom text would.
pub(super) fn spin_text_style(spin: SpinStyle) -> (SvgTextAlign, SvgTextVAlign, f32) {
    match spin {
        SpinStyle::Right => (SvgTextAlign::Left, SvgTextVAlign::Bottom, 0.0),
        SpinStyle::Left => (SvgTextAlign::Right, SvgTextVAlign::Bottom, 0.0),
        SpinStyle::Up => (SvgTextAlign::Left, SvgTextVAlign::Bottom, -90.0),
        SpinStyle::Bottom => (SvgTextAlign::Left, SvgTextVAlign::Top, -90.0),
    }
}

fn normalize_rotation(deg: f64) -> i32 {
    let r = (deg.round() as i32) % 360;
    if r < 0 { r + 360 } else { r }
}

pub(super) fn halign_to_svg(h: HAlign) -> SvgTextAlign {
    match h {
        HAlign::Left => SvgTextAlign::Left,
        HAlign::Center => SvgTextAlign::Center,
        HAlign::Right => SvgTextAlign::Right,
    }
}

pub(super) fn valign_to_svg(v: VAlign) -> SvgTextVAlign {
    match v {
        VAlign::Top => SvgTextVAlign::Top,
        VAlign::Center => SvgTextVAlign::Center,
        VAlign::Bottom => SvgTextVAlign::Bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1229 — label text grows with the label's size, so the distance it
    /// keeps from its anchor does too; the default size keeps the old numbers.
    #[test]
    fn label_offsets_scale_with_the_label_size_and_keep_the_default() {
        const DEFAULT: f64 = signex_types::schematic::SCHEMATIC_TEXT_MM;
        let (x, y) = schematic_text_offset_global("input", SpinStyle::Right, DEFAULT);
        assert!((x - DEFAULT * 1.25).abs() < 1e-9 && (y - DEFAULT * 0.0715).abs() < 1e-9);
        let (big_x, big_y) = schematic_text_offset_global("input", SpinStyle::Right, 2.0 * DEFAULT);
        assert!((big_x - 2.0 * x).abs() < 1e-9 && (big_y - 2.0 * y).abs() < 1e-9);

        let (hx, _) = schematic_text_offset_hier("ABCD", DEFAULT, SpinStyle::Right);
        assert!((hx - (DEFAULT * 0.4 + 4.0 * DEFAULT * 0.6)).abs() < 1e-9);
        let (big_hx, _) = schematic_text_offset_hier("ABCD", 2.0 * DEFAULT, SpinStyle::Right);
        assert!((big_hx - 2.0 * hx).abs() < 1e-9);

        // Smaller than the default is still the default, as on the canvas.
        let (small_x, _) = schematic_text_offset_global("output", SpinStyle::Right, 0.5);
        assert!((small_x - DEFAULT * 0.5).abs() < 1e-9);
    }

    /// Every label orientation exports upright or bottom-to-top text.
    #[test]
    fn label_text_is_never_upside_down_or_top_to_bottom() {
        for justify in [HAlign::Left, HAlign::Right, HAlign::Center] {
            for rotation in [0.0, 90.0, 180.0, 270.0] {
                let (_, _, rot) = spin_text_style(label_spin_style(justify, rotation));
                assert!(
                    rot == 0.0 || rot == -90.0,
                    "{justify:?} @ {rotation}: {rot}°"
                );
            }
        }
    }

    /// Turning top-to-bottom text half a revolution keeps it on the same
    /// strip of the page and the same side of its baseline.
    #[test]
    fn downward_label_text_keeps_its_place() {
        let (align, v_align, rot) = spin_text_style(SpinStyle::Bottom);
        // Was right-aligned at +90°: the text ends at the anchor, so it
        // covers the strip above it. Left-aligned at -90° starts at the
        // anchor and runs up the same strip. Top alignment at -90° puts the
        // glyphs on the same (+x) side of the baseline as Bottom did at +90°.
        assert_eq!(rot, -90.0);
        assert!(matches!(align, SvgTextAlign::Left));
        assert!(matches!(v_align, SvgTextVAlign::Top));
    }
}
