//! Curve drawings (#1267) in `.snxsch`: an ellipse, elliptical arc or round
//! rectangle round-trips byte for byte under `snxsch/1.5`, a sheet without
//! one keeps its lower token, and a 1.4 build opens the token read-only.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{CurveShape, FillType, Point, SchCurve, SchDrawing, SchematicSheet};

fn curve(n: u128, shape: CurveShape) -> SchDrawing {
    SchDrawing::Curve(SchCurve::new(Uuid::from_u128(n), shape, 0.25))
}

fn ellipse() -> CurveShape {
    CurveShape::Ellipse {
        center: Point::new(25.4, 25.4),
        radius_x: 7.62,
        radius_y: 5.08,
    }
}

fn arc() -> CurveShape {
    CurveShape::EllipticalArc {
        center: Point::new(50.8, 25.4),
        radius_x: 10.16,
        radius_y: 5.08,
        start_deg: 30.0,
        end_deg: 200.0,
    }
}

fn round_rect() -> CurveShape {
    CurveShape::round_rect(Point::new(10.16, 40.64), Point::new(30.48, 50.8))
}

fn sheet_of(drawings: Vec<SchDrawing>) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.drawings = drawings;
    sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn starts_with_token(text: &str, token: &str) -> bool {
    text.starts_with(&format!("format = \"{token}\""))
}

#[test]
fn every_curve_kind_round_trips_byte_identical_under_1_5() {
    let mut filled = SchCurve::new(Uuid::from_u128(4), ellipse(), 0.5);
    filled.fill = FillType::Background;
    let sheet = sheet_of(vec![
        curve(1, ellipse()),
        curve(2, arc()),
        curve(3, round_rect()),
        SchDrawing::Curve(filled),
    ]);
    let text = save(&sheet);
    assert!(starts_with_token(&text, SNXSCH_FORMAT_CURVES), "{text}");
    for tag in [
        "type = \"curve\"",
        "kind = \"ellipse\"",
        "kind = \"elliptical_arc\"",
        "kind = \"round_rect\"",
    ] {
        assert!(text.contains(tag), "{tag}: {text}");
    }
    assert!(
        !text.contains("locked = "),
        "an open lock is not written: {text}"
    );

    let back = load(&text);
    assert_eq!(back.drawings, sheet.drawings);
    assert_eq!(save(&back), text, "a second save is byte-identical");
}

#[test]
fn a_locked_curve_keeps_its_lock() {
    let mut locked = SchCurve::new(Uuid::from_u128(5), arc(), 0.0);
    locked.locked = true;
    let sheet = sheet_of(vec![SchDrawing::Curve(locked)]);
    let back = load(&save(&sheet));
    assert!(back.drawings[0].is_locked());
}

#[test]
fn a_sheet_without_a_curve_keeps_its_token_and_drops_back_when_the_last_goes() {
    let mut sheet = sheet_of(vec![curve(1, ellipse())]);
    assert!(starts_with_token(&save(&sheet), SNXSCH_FORMAT_CURVES));
    sheet.drawings.clear();
    assert!(starts_with_token(&save(&sheet), SNXSCH_FORMAT_V1));
}

/// A 1.4 build has no curve drawing kind, so it must see this token as a
/// newer minor and refuse the save instead of failing on the drawing.
#[test]
fn a_1_4_build_opens_a_curve_sheet_read_only() {
    let one_four = FormatVersion {
        current: SNXSCH_FORMAT_BUS_STYLE,
        readable: &[
            "snxsch/1",
            SNXSCH_FORMAT_V1,
            SNXSCH_FORMAT_NO_ERC_RULES,
            SNXSCH_FORMAT_REFINEMENT,
            SNXSCH_FORMAT_BUS_STYLE,
        ],
        ..SNXSCH_VERSION
    };
    assert!(one_four.is_newer_minor(SNXSCH_FORMAT_CURVES));
    assert!(one_four.refuse_save(SNXSCH_FORMAT_CURVES).is_err());
}
