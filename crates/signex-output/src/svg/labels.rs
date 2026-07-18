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

pub(super) fn label_size_pt(font_size_mm: f64, mm_to_unit: f64, scale: &PdfScale) -> f32 {
    if font_size_mm > 0.0 {
        (font_size_mm * mm_to_unit) as f32
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

pub(super) fn schematic_text_offset_hier(text: &str, font_size_mm: f64, spin: SpinStyle) -> (f64, f64) {
    let dist = font_size_mm * 0.4
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
            * font_size_mm
            * 0.6;
    match spin {
        SpinStyle::Left => (-dist, 0.0),
        SpinStyle::Up => (0.0, -dist),
        SpinStyle::Right => (dist, 0.0),
        SpinStyle::Bottom => (0.0, dist),
    }
}

pub(super) fn schematic_text_offset_global(shape: &str, spin: SpinStyle) -> (f64, f64) {
    let mut horiz = signex_types::schematic::SCHEMATIC_TEXT_MM * 0.5;
    let vert = signex_types::schematic::SCHEMATIC_TEXT_MM * 0.0715;

    if matches!(shape, "input" | "bidirectional" | "tri_state") {
        horiz += signex_types::schematic::SCHEMATIC_TEXT_MM * 0.75;
    }

    match spin {
        SpinStyle::Left => (-horiz, vert),
        SpinStyle::Up => (vert, -horiz),
        SpinStyle::Right => (horiz, vert),
        SpinStyle::Bottom => (vert, horiz),
    }
}

pub(super) fn spin_text_style(spin: SpinStyle) -> (SvgTextAlign, SvgTextVAlign, f32) {
    let align = match spin {
        SpinStyle::Left | SpinStyle::Bottom => SvgTextAlign::Right,
        SpinStyle::Right | SpinStyle::Up => SvgTextAlign::Left,
    };
    let rotation = match spin {
        SpinStyle::Up => -90.0,
        SpinStyle::Bottom => 90.0,
        _ => 0.0,
    };
    (align, SvgTextVAlign::Bottom, rotation)
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
