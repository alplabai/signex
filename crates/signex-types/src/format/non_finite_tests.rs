//! #678 — a non-finite number in a `.snxsch` / `.snxpcb` TSV cell must
//! fail the load with a field-level error instead of becoming NaN
//! geometry that later collapses to the sheet origin.

use super::pcb_rows::PcbFootprintRow;
use super::sch_rows::{SchComponentRow, SchJunctionRow, SchLabelRow, SchWireRow};
use super::tsv::format_f64;
use super::tsv::{parse_f64, parse_tsv_block};
use super::units::{NonFiniteField, mm_to_nm};
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
    let mut cells = row.to_row().expect("row under test is finite");
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

#[test]
fn mm_to_nm_refuses_values_the_wire_format_cannot_hold() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1.0e13, -1.0e13] {
        assert_eq!(
            mm_to_nm(bad, "pos_x"),
            Err(NonFiniteField("pos_x")),
            "{bad}"
        );
    }
    assert_eq!(mm_to_nm(1.5, "pos_x"), Ok(1_500_000));
    assert_eq!(mm_to_nm(-2.0, "pos_x"), Ok(-2_000_000));
}

#[test]
fn format_f64_refuses_non_finite_and_keeps_finite_output() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(format_f64(bad, "rotation"), Err(NonFiniteField("rotation")));
    }
    assert_eq!(format_f64(0.0, "f").unwrap(), "0");
    assert_eq!(format_f64(90.0, "f").unwrap(), "90");
    assert_eq!(format_f64(0.25, "f").unwrap(), "0.25");
}

#[test]
fn write_tsv_block_names_block_row_and_field() {
    let finite = SchWireRow {
        uuid: Uuid::nil(),
        net: "n".into(),
        start_x: 0,
        start_y: 0,
        end_x: 1,
        end_y: 1,
        stroke_width: 0.15,
        unknown_columns: Default::default(),
    };
    let bad = SchWireRow {
        stroke_width: f64::INFINITY,
        ..finite.clone()
    };
    match write_tsv_block("sheets.wires", &[finite, bad]) {
        Err(FormatError::NonFiniteField { block, row, field }) => {
            assert_eq!(
                (block.as_str(), row, field.as_str()),
                ("sheets.wires", 1, "stroke_width")
            );
        }
        other => panic!("expected NonFiniteField, got {other:?}"),
    }
}

#[test]
fn writing_a_sheet_with_a_nan_coordinate_fails_naming_the_row() {
    use crate::schematic::{SchematicSheet, Wire};
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire::default());
    sheet.wires.push(Wire {
        start: crate::schematic::Point {
            x: f64::NAN,
            y: 0.0,
        },
        ..Wire::default()
    });
    match SnxSchematic::new(sheet).write_string() {
        Err(FormatError::NonFiniteField { block, row, field }) => {
            assert_eq!(
                (block.as_str(), row, field.as_str()),
                ("sheets.wires", 1, "start_x")
            );
        }
        other => panic!("expected NonFiniteField, got {other:?}"),
    }
}
