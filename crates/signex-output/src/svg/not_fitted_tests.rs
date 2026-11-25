//! #1294 — a not-fitted part on the page: greyed, with a cross over its
//! body, while the preference is on.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::project::VariantPart;
use signex_types::schematic::{FillType, Graphic, InstancePath, LibGraphic, Point, SchematicSheet};

use super::*;

/// One part, U1, whose body is a 10 x 5 mm rectangle.
fn page(dnp: bool) -> SheetSnapshot {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:BODY" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 5.0),
            width: 0.25,
            fill: FillType::None,
        },
    });
    let mut schematic = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    schematic.lib_symbols.insert(lib.id.clone(), lib);
    schematic.symbols.push(Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "L:BODY".into(),
        reference: "U1".into(),
        position: Point::new(100.0, 100.0),
        dnp,
        ..Symbol::default()
    });
    SheetSnapshot {
        path: PathBuf::from("test.snxsch"),
        schematic,
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: InstancePath::root(),
    }
}

fn options(draw_not_fitted: bool, variant: Option<&str>) -> PdfOptions {
    PdfOptions {
        draw_not_fitted,
        variant: variant.map(str::to_string),
        // One page unit per mm: the body stroke stays under the border's.
        scale: crate::pdf::PdfScale::OneToOne,
        palette: SchematicPalette {
            symbol_body_tint: Some((0.9, 0.9, 0.8)),
            ..SchematicPalette::classic()
        },
        ..PdfOptions::default()
    }
}

/// What the part draws: the body outline's stroke, its body fill, and how
/// many crosses (open two-point strokes in the symbol ink) the page has.
struct Drawn {
    outline: (f32, f32, f32),
    fill: (f32, f32, f32),
    crosses: usize,
}

fn drawn(sheet: &SheetSnapshot, opts: &PdfOptions, settings: Option<&VariantSettings>) -> Drawn {
    let (refdes, nets) = (HashMap::new(), HashMap::new());
    let inputs = SvgEvaluatorInputs {
        special_strings: None,
        global_refdes: &refdes,
        net_name_by_symbol_pin: &nets,
        cell: "",
        variant_settings: settings,
    };
    let page = SvgRenderContext::from_sheet(sheet, opts, 297.0, 210.0, 1.0, Some(&inputs));
    // Each path as (closed, command count, style).
    let paths: Vec<(bool, usize, SvgStyle)> = page
        .elements
        .iter()
        .filter_map(|e| match e {
            SvgElement::Path { commands, style } => Some((
                commands.iter().any(|c| matches!(c, SvgPathCommand::Close)),
                commands.len(),
                *style,
            )),
            SvgElement::Text { .. } => None,
        })
        .collect();
    let outline = paths
        .iter()
        .find(|(closed, _, style)| *closed && style.fill_rgb.is_none() && style.stroke_width < 1.0)
        .and_then(|(_, _, style)| style.stroke_rgb)
        .expect("the body outline");
    // The first outline-less fill is the page's, the second the body's.
    let fill = paths
        .iter()
        .filter(|(closed, _, style)| {
            *closed && style.stroke_rgb.is_none() && style.stroke_width == 0.0
        })
        .nth(1)
        .and_then(|(_, _, style)| style.fill_rgb)
        .expect("the body fill after the page fill");
    let ink = opts.palette.symbol_stroke;
    let crosses = paths
        .iter()
        .filter(|(closed, count, style)| *count == 2 && !*closed && style.stroke_rgb == Some(ink))
        .count();
    Drawn {
        outline,
        fill,
        crosses,
    }
}

#[test]
fn a_dnp_part_prints_greyed_with_a_cross_and_a_fitted_one_as_before() {
    let opts = options(true, None);
    let palette = opts.palette;
    let fitted = drawn(&page(false), &opts, None);
    assert_eq!(fitted.outline, palette.symbol_stroke);
    assert_eq!(fitted.fill, (0.9, 0.9, 0.8));
    assert_eq!(fitted.crosses, 0);

    let dnp = drawn(&page(true), &opts, None);
    assert_eq!(
        dnp.outline,
        greyed_rgb(palette.symbol_stroke, palette.paper)
    );
    assert_eq!(dnp.fill, greyed_rgb((0.9, 0.9, 0.8), palette.paper));
    assert_eq!(dnp.crosses, 2, "two strokes, corner to corner");
}

#[test]
fn with_the_preference_off_a_dnp_part_prints_as_fitted() {
    let opts = options(false, None);
    let dnp = drawn(&page(true), &opts, None);
    assert_eq!(dnp.outline, opts.palette.symbol_stroke);
    assert_eq!(dnp.crosses, 0);
}

#[test]
fn the_printed_variant_and_the_project_settings_decide() {
    let settings = VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            designator: "U1".into(),
            fitted: Some(false),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    };
    let sheet = page(false);
    let lite = drawn(&sheet, &options(true, Some("Lite")), Some(&settings));
    assert_eq!(lite.crosses, 2, "not fitted in Lite");
    let base = drawn(&sheet, &options(true, Some("Base")), Some(&settings));
    assert_eq!(base.crosses, 0, "fitted in Base");
}
