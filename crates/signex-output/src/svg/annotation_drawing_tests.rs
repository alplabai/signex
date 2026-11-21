//! Bezier, dimension and table drawings (#1267, #1300) in the SVG page
//! model, and so in the PDF and the print preview.

use std::path::PathBuf;

use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{
    DimensionArrows, DimensionDirection, Point, SchBezier, SchDimension, SchDrawing, SchTable,
    SchematicSheet, Symbol, TableBorders, TableSource,
};
use uuid::Uuid;

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;

fn elements(sheet: SchematicSheet) -> Vec<SvgElement> {
    let snapshot = SheetSnapshot {
        path: PathBuf::from("test.snxsch"),
        schematic: sheet,
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    let opts = PdfOptions::default();
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    SvgRenderContext::from_sheet(&snapshot, &opts, w_mm, h_mm, 72.0 / 25.4, None).elements
}

fn sheet_with(drawing: SchDrawing) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.drawings.push(drawing);
    sheet
}

fn texts(elements: &[SvgElement]) -> Vec<String> {
    elements
        .iter()
        .filter_map(|e| match e {
            SvgElement::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn cubic_count(elements: &[SvgElement]) -> usize {
    elements
        .iter()
        .filter(|e| match e {
            SvgElement::Path { commands, .. } => commands
                .iter()
                .any(|c| matches!(c, SvgPathCommand::CubicTo(..))),
            _ => false,
        })
        .count()
}

fn bezier() -> SchBezier {
    SchBezier::new(
        Uuid::from_u128(1),
        Point::new(20.0, 40.0),
        Point::new(22.0, 70.0),
        Point::new(80.0, 70.0),
        Point::new(85.0, 40.0),
    )
}

#[test]
fn a_solid_bezier_is_one_cubic_path() {
    let before = elements(sheet_with(SchDrawing::Line {
        uuid: Uuid::from_u128(9),
        start: Point::new(10.0, 10.0),
        end: Point::new(20.0, 10.0),
        width: 0.0,
        stroke_color: None,
        locked: false,
    }));
    let with = elements(sheet_with(SchDrawing::Bezier(bezier())));
    assert_eq!(cubic_count(&with), cubic_count(&before) + 1);
}

#[test]
fn a_dashed_bezier_is_flattened_marks_and_ends_add_heads() {
    let mut b = bezier();
    b.line_style = NetLineStyle::Dashed;
    b.ends = signex_types::schematic::LineEnds::Both;
    let solid_count = elements(sheet_with(SchDrawing::Bezier(bezier()))).len();
    let out = elements(sheet_with(SchDrawing::Bezier(b)));
    let filled = out
        .iter()
        .filter(|e| matches!(e, SvgElement::Path { style, .. } if style.fill_rgb.is_some()))
        .count();
    assert!(filled >= 2, "two arrowheads");
    assert_eq!(out.len(), solid_count + 2, "one dash path and two heads");
}

#[test]
fn a_dimension_writes_its_measured_text() {
    let d = SchDimension::new(
        Uuid::from_u128(2),
        Point::new(30.0, 30.0),
        Point::new(42.7, 30.0),
        -5.0,
        DimensionDirection::Horizontal,
    );
    let out = elements(sheet_with(SchDrawing::Dimension(d.clone())));
    assert!(
        texts(&out).contains(&"12.70 mm".to_string()),
        "{:?}",
        texts(&out)
    );

    let mut open = d;
    open.arrows = DimensionArrows::Open;
    open.override_text = Some("pitch".into());
    let out = elements(sheet_with(SchDrawing::Dimension(open)));
    assert!(texts(&out).contains(&"pitch".to_string()));
}

#[test]
fn a_table_writes_its_header_and_cells_and_power_nets_in_the_power_colour() {
    let mut t = SchTable::new_typed(Uuid::from_u128(3), Point::new(40.0, 40.0));
    t.source = TableSource::ComponentPins {
        designator: "U1".into(),
        sort: Default::default(),
    };
    t.columns = SchTable::columns_for(&t.source);
    t.rows = vec![
        vec![
            "2".into(),
            "VIN".into(),
            "VIN_12V".into(),
            "power in".into(),
        ],
        vec!["3".into(), "EN".into(), "EN".into(), "input".into()],
    ];
    t.title = Some("U1 pin assignment".into());
    let mut sheet = sheet_with(SchDrawing::Table(t));
    sheet.symbols.push(Symbol {
        uuid: Uuid::from_u128(7),
        is_power: true,
        value: "VIN_12V".into(),
        position: Point::new(150.0, 150.0),
        ..Default::default()
    });
    let palette = PdfOptions::default().palette;
    let out = elements(sheet);
    let all = texts(&out);
    for want in [
        "U1 pin assignment",
        "Pin",
        "Net",
        "VIN_12V",
        "EN",
        "power in",
    ] {
        assert!(all.contains(&want.to_string()), "{want} in {all:?}");
    }
    assert!(
        !all.contains(&"Unit".to_string()),
        "hidden columns are not drawn"
    );
    let colour_of = |want: &str| {
        out.iter().find_map(|e| match e {
            SvgElement::Text { text, fill_rgb, .. } if text == want => Some(*fill_rgb),
            _ => None,
        })
    };
    assert_eq!(colour_of("VIN_12V"), Some(palette.power_label));
    assert_ne!(colour_of("EN"), Some(palette.power_label));
}

#[test]
fn table_borders_follow_the_style() {
    let mut t = SchTable::new_typed(Uuid::from_u128(4), Point::new(10.0, 10.0));
    let (all_lines, outline) = sch_table::border_segments(&t);
    assert_eq!(outline.len(), 4);
    // Header rule, two row rules, two column rules.
    assert_eq!(all_lines.len(), 5);
    t.style.borders = TableBorders::Outer;
    assert_eq!(sch_table::border_segments(&t).0.len(), 1);
    t.style.borders = TableBorders::None;
    let (lines, outline) = sch_table::border_segments(&t);
    assert!(lines.is_empty() && outline.is_empty());
}

#[test]
fn annotations_widen_the_content_box() {
    let t = SchTable::new_typed(Uuid::from_u128(5), Point::new(10.0, 10.0));
    let points = annotation_extent(&SchDrawing::Table(t.clone()));
    assert_eq!(points.len(), 2);
    assert_eq!(points[1], t.bounds().1);
    assert_eq!(
        annotation_extent(&SchDrawing::Bezier(bezier())).len(),
        signex_types::schematic::BEZIER_FLATTEN_SEGMENTS + 1
    );
}
