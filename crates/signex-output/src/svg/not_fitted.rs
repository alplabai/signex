//! Not-fitted parts on the page (#1294): which parts the page greys and
//! crosses out, the grey and the cross. The look is the one rule in
//! `signex_types::schematic::not_fitted`, the canvas's too.

use std::collections::HashSet;

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;
use signex_types::project::VariantSettings;
use signex_types::schematic::{
    LibSymbol, NOT_FITTED_CROSS_WIDTH_SCALE, SchematicDefaults, Symbol, greyed_ink,
    not_fitted_cross,
};

/// The symbols of `sheet` drawn as not fitted in the printed variant
/// (`opts.variant`); none while `opts.draw_not_fitted` is off.
pub(super) fn drawn_not_fitted(
    sheet: &SheetSnapshot,
    opts: &PdfOptions,
    inputs: Option<&SvgEvaluatorInputs<'_>>,
) -> HashSet<uuid::Uuid> {
    if !opts.draw_not_fitted {
        return HashSet::new();
    }
    let sheets_only = VariantSettings::default();
    let settings = inputs
        .and_then(|inputs| inputs.variant_settings)
        .unwrap_or(&sheets_only);
    crate::variant_fit::not_fitted_symbols(
        &sheet.schematic,
        &sheet.placement,
        opts.variant.as_deref(),
        settings,
    )
}

/// `rgb` greyed toward `paper`.
pub(super) fn greyed_rgb(rgb: (f32, f32, f32), paper: (f32, f32, f32)) -> (f32, f32, f32) {
    let [r, g, b] = greyed_ink([rgb.0, rgb.1, rgb.2], [paper.0, paper.1, paper.2]);
    (r, g, b)
}

/// Grey every stroke, fill and text of `elements` (one part's drawing).
pub(super) fn grey_elements(elements: &mut [SvgElement], paper: (f32, f32, f32)) {
    for element in elements {
        match element {
            SvgElement::Path { style, .. } => {
                for rgb in [style.stroke_rgb.as_mut(), style.fill_rgb.as_mut()]
                    .into_iter()
                    .flatten()
                {
                    *rgb = greyed_rgb(*rgb, paper);
                }
            }
            SvgElement::Text { fill_rgb, .. } => *fill_rgb = greyed_rgb(*fill_rgb, paper),
        }
    }
}

/// The cross over `sym`'s body, in the symbol ink, not greyed.
pub(super) fn push_not_fitted_cross(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: &LibSymbol,
    xform: &PageTransform,
    palette: &SchematicPalette,
    defaults: &SchematicDefaults,
) {
    let Some(strokes) = not_fitted_cross(lib, sym) else {
        return;
    };
    let width = defaults.symbol_line_mm(0.0) * NOT_FITTED_CROSS_WIDTH_SCALE * xform.mm_to_unit;
    for (a, b) in strokes {
        out.push(SvgElement::Path {
            commands: vec![
                SvgPathCommand::MoveTo(pt(xform.x(a.x), xform.px_y(a.y))),
                SvgPathCommand::LineTo(pt(xform.x(b.x), xform.px_y(b.y))),
            ],
            style: SvgStyle {
                stroke_alpha: 1.0,
                stroke_rgb: Some(palette.symbol_stroke),
                fill_rgb: None,
                stroke_width: width as f32,
            },
        });
    }
}

#[cfg(test)]
#[path = "not_fitted_tests.rs"]
mod tests;
