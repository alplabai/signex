//! #678 — a non-finite number in a `.snxsch` / `.snxpcb` TSV cell must
//! fail the load with a field-level error instead of becoming NaN
//! geometry that later collapses to the sheet origin.

use super::pcb_rows::PcbFootprintRow;
use super::sch_rows::{SchComponentRow, SchJunctionRow, SchLabelRow, SchWireRow};
use super::tsv::{parse_f64, parse_tsv_block};
use super::units::mm_to_nm;
use super::*;
use uuid::Uuid;

const NON_FINITE_SPELLINGS: [&str; 6] = ["NaN", "nan", "inf", "-inf", "infinity", "1e999"];

/// Render `row` to TSV with `column` replaced by `bad`, then parse it back.
fn parse_with_cell<R: SnxTable>(row: &R, column: &str, bad: &str) -> Result<Vec<R>, FormatError> {
    let columns = R::columns();
    let idx = columns
        .iter()
        .position(|c| *c == column)
        .expect("column exists");
    let mut cells = row.to_row();
    cells[idx] = bad.to_string();
    let body = format!("{}\n{}\n", columns.join("\t"), cells.join("\t"));
    parse_tsv_block("test", &body)
}

fn assert_rejects_every_spelling<R: SnxTable + std::fmt::Debug>(row: &R, column: &str) {
    for bad in NON_FINITE_SPELLINGS {
        match parse_with_cell(row, column, bad) {
            Err(FormatError::TsvFieldParse { field, .. }) => assert_eq!(field, column),
            other => panic!("column {column:?} accepted {bad:?}: {other:?}"),
        }
    }
}

#[test]
fn parse_f64_rejects_non_finite_values() {
    for bad in NON_FINITE_SPELLINGS {
        assert!(
            parse_f64(bad, "b", 0, "f").is_err(),
            "{bad:?} must not parse"
        );
    }
    assert_eq!(parse_f64("1.5", "b", 0, "f").unwrap(), 1.5);
    assert_eq!(parse_f64("", "b", 0, "f").unwrap(), 0.0);
}

#[test]
fn component_rotation_rejects_non_finite() {
    let row = SchComponentRow {
        uuid: Uuid::nil(),
        ref_des: "R1".into(),
        library: "Device:R".into(),
        pos_x: 0,
        pos_y: 0,
        rotation: 0.0,
        value: "v".into(),
        mpn: "m".into(),
        unknown_columns: Default::default(),
    };
    assert_rejects_every_spelling(&row, "rotation");
}

#[test]
fn wire_stroke_width_rejects_non_finite() {
    let row = SchWireRow {
        uuid: Uuid::nil(),
        net: "n".into(),
        start_x: 0,
        start_y: 0,
        end_x: 1,
        end_y: 1,
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    };
    assert_rejects_every_spelling(&row, "stroke_width");
}

#[test]
fn junction_diameter_rejects_non_finite() {
    let row = SchJunctionRow {
        uuid: Uuid::nil(),
        pos_x: 0,
        pos_y: 0,
        diameter: 0.0,
        unknown_columns: Default::default(),
    };
    assert_rejects_every_spelling(&row, "diameter");
}

#[test]
fn label_rotation_and_font_size_reject_non_finite() {
    let row = SchLabelRow {
        uuid: Uuid::nil(),
        text: "N".into(),
        pos_x: 0,
        pos_y: 0,
        rotation: 0.0,
        kind: "net".into(),
        shape: "s".into(),
        font_size: 1.0,
        justify: "left".into(),
        justify_v: "bottom".into(),
        unknown_columns: Default::default(),
    };
    assert_rejects_every_spelling(&row, "rotation");
    assert_rejects_every_spelling(&row, "font_size");
}

#[test]
fn pcb_footprint_rotation_rejects_non_finite() {
    let row = PcbFootprintRow {
        uuid: Uuid::nil(),
        ref_des: "U1".into(),
        library: "L:F".into(),
        pos_x: 0,
        pos_y: 0,
        rotation: 0.0,
        layer: "F.Cu".into(),
        value: "v".into(),
        unknown_columns: Default::default(),
    };
    assert_rejects_every_spelling(&row, "rotation");
}

/// HI-12 documents that a non-finite coordinate saves as the largest
/// representable value ("visibly wrong"), never as the origin.
#[test]
fn mm_to_nm_never_maps_nan_to_the_origin() {
    assert_eq!(mm_to_nm(f64::NAN), i64::MAX);
    assert_eq!(mm_to_nm(f64::INFINITY), i64::MAX);
    assert_eq!(mm_to_nm(f64::NEG_INFINITY), i64::MIN);
}
