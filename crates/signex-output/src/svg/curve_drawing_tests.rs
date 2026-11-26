//! Curve drawings (#1267) in the SVG page model, and so in the PDF and the
//! print preview: one path from the shared outline, closed and filled for a
//! closed shape, open and unfilled for an elliptical arc, in the curve's own
//! colour.

use std::path::PathBuf;

use signex_types::schematic::{
    CurveShape, FillType, Point, SchCurve, SchDrawing, SchematicSheet, StrokeColor,
};
use uuid::Uuid;

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;

/// A colour nothing else on the page uses, to find the curve's paths.
const TAG: StrokeColor = StrokeColor {
    r: 1,
    g: 2,
    b: 3,
    a: 255,
};

fn tag_rgb() -> (f32, f32, f32) {
    (1.0 / 255.0, 2.0 / 255.0, 3.0 / 255.0)
}

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

/// The paths `curve`, tagged with [`TAG`], exports to.
fn curve_paths(mut curve: SchCurve) -> Vec<(Vec<SvgPathCommand>, SvgStyle)> {
    curve.stroke_color = Some(TAG);
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        drawings: vec![SchDrawing::Curve(curve)],
        ..Default::default()
    };
    elements(sheet)
        .into_iter()
        .filter_map(|e| match e {
            SvgElement::Path { commands, style } if style.stroke_rgb == Some(tag_rgb()) => {
                Some((commands, style))
            }
            _ => None,
        })
        .collect()
}

fn ellipse() -> CurveShape {
    CurveShape::Ellipse {
        center: Point::new(50.0, 50.0),
        radius_x: 10.0,
        radius_y: 5.0,
    }
}

#[test]
fn a_filled_ellipse_is_one_closed_filled_path_of_its_outline() {
    let mut c = SchCurve::new(Uuid::from_u128(1), ellipse(), 0.25);
    c.fill = FillType::Background;
    let outline = c.flatten().len();
    let paths = curve_paths(c);
    assert_eq!(paths.len(), 1, "{paths:?}");
    let (commands, style) = &paths[0];
    assert_eq!(
        commands.len(),
        outline + 1,
        "every outline point, then close"
    );
    assert!(matches!(commands.last(), Some(SvgPathCommand::Close)));
    assert!(style.fill_rgb.is_some());
}

#[test]
fn an_elliptical_arc_is_open_and_never_filled() {
    let arc = CurveShape::EllipticalArc {
        center: Point::new(50.0, 50.0),
        radius_x: 10.0,
        radius_y: 5.0,
        start_deg: 0.0,
        end_deg: 180.0,
    };
    let mut c = SchCurve::new(Uuid::from_u128(2), arc, 0.25);
    c.fill = FillType::Background;
    let paths = curve_paths(c);
    assert_eq!(paths.len(), 1, "{paths:?}");
    let (commands, style) = &paths[0];
    assert!(!commands.iter().any(|c| matches!(c, SvgPathCommand::Close)));
    assert!(style.fill_rgb.is_none());
}

#[test]
fn a_round_rectangle_exports_in_its_own_colour() {
    let c = SchCurve::new(
        Uuid::from_u128(3),
        CurveShape::round_rect(Point::new(20.0, 20.0), Point::new(40.0, 30.0)),
        0.0,
    );
    assert_eq!(curve_paths(c).len(), 1);
}

#[test]
fn a_curve_widens_the_page_content_box() {
    let points = annotation_extent(&SchDrawing::Curve(SchCurve::new(
        Uuid::from_u128(4),
        ellipse(),
        0.0,
    )));
    let (lo_x, hi_x) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.x), hi.max(p.x))
    });
    assert!((lo_x - 40.0).abs() < 1e-9 && (hi_x - 60.0).abs() < 1e-9);
}
