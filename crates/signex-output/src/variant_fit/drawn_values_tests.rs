//! #2948: a part is drawn with the active (or printed) variant's own value,
//! in SVG, PDF and print as on the canvas; Base and a cleared value draw the
//! sheet's value.

use std::collections::{BTreeMap, HashMap};

use signex_types::project::{VALUE_PARAMETER, VariantPart, VariantSettings};
use signex_types::schematic::{InstancePath, Point, SchematicSheet, Symbol, TextProp};
use uuid::Uuid;

use super::*;
use crate::pdf::PdfOptions;
use crate::svg::{SvgElement, SvgEvaluatorInputs, SvgRenderContext};
use crate::{ExportContext, PdfExporter, PreviewOptions, PreviewRasterizer, ProjectMetadata};
use crate::{Exporter, SheetSnapshot};

fn resistor(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        val_text: Some(TextProp {
            position: Point::new(40.0, 40.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: Default::default(),
            justify_v: Default::default(),
            hidden: false,
        }),
        ..Default::default()
    }
}

/// Lite gives R1 the value `value`.
fn lite_value(value: &str) -> VariantSettings {
    VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            designator: "R1".into(),
            parameters: BTreeMap::from([(VALUE_PARAMETER.to_string(), value.to_string())]),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    }
}

fn context(symbol: Symbol, settings: VariantSettings) -> ExportContext {
    context_of(vec![symbol], settings)
}

fn context_of(symbols: Vec<Symbol>, settings: VariantSettings) -> ExportContext {
    ExportContext {
        sheets: vec![SheetSnapshot {
            path: "main.snxsch".into(),
            schematic: SchematicSheet {
                symbols,
                ..Default::default()
            },
            sheet_name: "Main".to_string(),
            sheet_number: 1,
            sheet_count: 1,
            placement: InstancePath::root(),
        }],
        metadata: ProjectMetadata {
            variant_settings: settings,
            ..ProjectMetadata::default()
        },
        netlist: None,
        netlist_sheets: Vec::new(),
        netlist_placements: Vec::new(),
        current_sheet: None,
    }
}

fn values_of(ctx: &ExportContext, variant: Option<&str>) -> HashMap<Uuid, String> {
    DrawnValues::of_context(ctx, variant)
        .at(&InstancePath::root())
        .cloned()
        .unwrap_or_default()
}

#[test]
fn the_active_variant_gives_its_value_and_base_gives_none() {
    let r1 = resistor("R1", "10k");
    let ctx = context(r1.clone(), lite_value("47k"));
    assert_eq!(
        values_of(&ctx, Some("Lite")),
        HashMap::from([(r1.uuid, "47k".to_string())])
    );
    // Variant names compare without regard to ASCII case, as the BOM's do.
    assert_eq!(values_of(&ctx, Some("LITE")).len(), 1);
    assert!(values_of(&ctx, None).is_empty());
    assert!(values_of(&ctx, Some("Base")).is_empty());
    assert!(DrawnValues::of_context(&ctx, None).is_empty());
}

#[test]
fn a_cleared_value_draws_the_sheet_value_again() {
    let r1 = resistor("R1", "10k");
    let mut settings = lite_value("47k");
    settings.parts[0].parameters.clear();
    settings.parts[0].fitted = Some(true);
    let ctx = context(r1, settings);
    assert!(values_of(&ctx, Some("Lite")).is_empty());
}

#[test]
fn another_parameter_does_not_change_the_drawn_value() {
    let mut settings = lite_value("47k");
    settings.parts[0].parameters = BTreeMap::from([("Tolerance".to_string(), "1%".to_string())]);
    let ctx = context(resistor("R1", "10k"), settings);
    assert!(values_of(&ctx, Some("Lite")).is_empty());
}

#[test]
fn a_power_port_is_never_given_a_value() {
    let mut port = resistor("R1", "VCC");
    port.is_power = true;
    let ctx = context(port, lite_value("47k"));
    assert!(values_of(&ctx, Some("Lite")).is_empty());
}

#[test]
fn drawn_symbol_takes_the_value_and_borrows_otherwise() {
    let r1 = resistor("R1", "10k");
    let values = HashMap::from([(r1.uuid, "47k".to_string())]);
    assert_eq!(drawn_symbol(&r1, Some(&values)).value, "47k");
    assert!(matches!(
        drawn_symbol(&r1, None),
        std::borrow::Cow::Borrowed(_)
    ));
    let same = HashMap::from([(r1.uuid, "10k".to_string())]);
    assert!(matches!(
        drawn_symbol(&r1, Some(&same)),
        std::borrow::Cow::Borrowed(_)
    ));
}

/// The texts the SVG page of `ctx`'s first sheet prints in `variant`.
fn svg_texts(ctx: &ExportContext, variant: Option<&str>) -> Vec<String> {
    let values = DrawnValues::of_context(ctx, variant);
    let (refdes, nets) = (HashMap::new(), HashMap::new());
    let inputs = SvgEvaluatorInputs {
        special_strings: None,
        global_refdes: &refdes,
        net_name_by_symbol_pin: &nets,
        cell: "",
        variant_settings: None,
        placed_fits: None,
        drawn_values: Some(&values),
    };
    let page = SvgRenderContext::from_sheet(
        &ctx.sheets[0],
        &PdfOptions::default(),
        297.0,
        210.0,
        1.0,
        Some(&inputs),
    );
    page.elements
        .into_iter()
        .filter_map(|e| match e {
            SvgElement::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn the_svg_page_prints_the_variant_value() {
    let ctx = context(resistor("R1", "10k"), lite_value("47k"));
    let lite = svg_texts(&ctx, Some("Lite"));
    assert!(lite.iter().any(|t| t == "47k"), "{lite:?}");
    assert!(!lite.iter().any(|t| t == "10k"), "{lite:?}");
    let base = svg_texts(&ctx, None);
    assert!(base.iter().any(|t| t == "10k"), "{base:?}");
    assert!(!base.iter().any(|t| t == "47k"), "{base:?}");
}

/// An `=` variant value is evaluated as a sheet value is, over the other
/// parts of its sheet.
#[test]
fn an_expression_variant_value_prints_evaluated() {
    let ctx = context_of(
        vec![resistor("R1", "10k"), resistor("R2", "10kΩ")],
        lite_value("= 2 * R2.Value"),
    );
    let lite = svg_texts(&ctx, Some("Lite"));
    assert!(lite.iter().any(|t| t == "20kΩ"), "{lite:?}");
}

fn pdf_options(variant: Option<&str>) -> PdfOptions {
    PdfOptions {
        variant: variant.map(str::to_string),
        generate_nets_info: false,
        bookmark_sheets: false,
        bookmark_parts: false,
        ..PdfOptions::default()
    }
}

/// The PDF of a part given 47k by Lite is the PDF of the part whose sheet
/// says 47k; in Base it prints the sheet's 10k.
#[test]
fn the_pdf_prints_the_variant_value() {
    let given = context(resistor("R1", "10k"), lite_value("47k"));
    let mut written = given.clone();
    written.sheets[0].schematic.symbols[0].value = "47k".to_string();
    written.metadata.variant_settings = VariantSettings::default();
    let export = |ctx: &ExportContext, variant| {
        PdfExporter
            .export(ctx, &pdf_options(variant))
            .expect("export")
            .bytes
    };
    assert!(export(&given, Some("Lite")) == export(&written, Some("Lite")));
    let mut base_written = given.clone();
    base_written.metadata.variant_settings = VariantSettings::default();
    assert!(export(&given, None) == export(&base_written, None));
    assert!(export(&given, Some("Lite")) != export(&base_written, Some("Lite")));
}

/// Print and the print preview draw through the raster path: the same value.
#[test]
fn the_print_preview_draws_the_variant_value() {
    let given = context(resistor("R1", "10k"), lite_value("47k"));
    let mut written = given.clone();
    written.sheets[0].schematic.symbols[0].value = "47k".to_string();
    written.metadata.variant_settings = VariantSettings::default();
    let raster = |ctx: &ExportContext| {
        let opts = PreviewOptions {
            pdf: pdf_options(Some("Lite")),
            ..PreviewOptions::default()
        };
        PreviewRasterizer
            .rasterize(ctx, &opts)
            .into_iter()
            .map(|page| page.rgba)
            .collect::<Vec<_>>()
    };
    let drawn = raster(&given);
    assert!(!drawn.is_empty());
    assert!(drawn == raster(&written));
    let mut base = given.clone();
    base.metadata.variant_settings = VariantSettings::default();
    assert!(drawn != raster(&base));
}
