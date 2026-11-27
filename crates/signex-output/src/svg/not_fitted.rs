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
    LibSymbol, NOT_FITTED_CROSS_WIDTH_SCALE, Point, SchematicDefaults, Symbol, greyed_ink,
    not_fitted_cross,
};

/// The symbols of one page whose part is not fitted in the printed variant
/// (`opts.variant`), read by one rule for both uses.
#[derive(Debug, Default)]
pub(super) struct NotFitted {
    /// They print in the Not-fitted parts PDF layer (#2610).
    pub in_variant: HashSet<uuid::Uuid>,
    /// They draw greyed and crossed out (#1294): `in_variant`, or none
    /// while `opts.draw_not_fitted` is off.
    pub drawn: HashSet<uuid::Uuid>,
}

/// The not-fitted symbols of `sheet`. Read only when something uses them:
/// a layered PDF or the not-fitted look.
pub(super) fn not_fitted_on_page(
    sheet: &SheetSnapshot,
    opts: &PdfOptions,
    inputs: Option<&SvgEvaluatorInputs<'_>>,
) -> NotFitted {
    if !opts.draw_not_fitted && opts.layers.is_none() {
        return NotFitted::default();
    }
    let in_variant = variant_not_fitted(sheet, opts, inputs);
    let drawn = if opts.draw_not_fitted {
        in_variant.clone()
    } else {
        HashSet::new()
    };
    NotFitted { in_variant, drawn }
}

/// The symbols of `sheet` not fitted in the printed variant.
fn variant_not_fitted(
    sheet: &SheetSnapshot,
    opts: &PdfOptions,
    inputs: Option<&SvgEvaluatorInputs<'_>>,
) -> HashSet<uuid::Uuid> {
    let sheets_only = VariantSettings::default();
    let settings = inputs
        .and_then(|inputs| inputs.variant_settings)
        .unwrap_or(&sheets_only);
    let variant = opts.variant.as_deref();
    match inputs.and_then(|inputs| inputs.placed_fits) {
        Some(fits) => fits.not_fitted_on(&sheet.schematic, &sheet.placement, variant, settings),
        None => crate::variant_fit::not_fitted_symbols(
            &sheet.schematic,
            &sheet.placement,
            variant,
            settings,
        ),
    }
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

/// Half the side of the placeholder box a symbol whose library is missing
/// draws, centred on its position, in mm.
pub(super) const MISSING_LIBRARY_BOX_HALF_MM: f64 = 5.0;

/// The cross over `sym`'s body, in the theme's error ink, not greyed. With no
/// library (`lib` is `None`) it crosses the placeholder box instead, as the
/// canvas does.
pub(super) fn push_not_fitted_cross(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: Option<&LibSymbol>,
    xform: &PageTransform,
    palette: &SchematicPalette,
    defaults: &SchematicDefaults,
) {
    let strokes = match lib {
        Some(lib) => not_fitted_cross(lib, sym),
        None => Some(missing_library_box_cross(sym)),
    };
    let Some(strokes) = strokes else {
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
                stroke_rgb: Some(palette.error),
                fill_rgb: None,
                stroke_width: width as f32,
            },
        });
    }
}

/// The two corner-to-corner strokes of `sym`'s missing-library box.
fn missing_library_box_cross(sym: &Symbol) -> [(Point, Point); 2] {
    let half = MISSING_LIBRARY_BOX_HALF_MM;
    let (x, y) = (sym.position.x, sym.position.y);
    [
        (
            Point::new(x - half, y - half),
            Point::new(x + half, y + half),
        ),
        (
            Point::new(x - half, y + half),
            Point::new(x + half, y - half),
        ),
    ]
}

#[cfg(test)]
#[path = "not_fitted_tests.rs"]
mod tests;
