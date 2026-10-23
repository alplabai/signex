//! Field-level corruption: a single bad cell in an otherwise valid row must
//! surface as `FormatError::TsvFieldParse` naming that field, never a panic
//! or a silently defaulted value. Float cells reject empty and non-finite
//! (`NaN`, `inf`, `infinity`) input; integer and uuid cells reject empty.

use signex_types::format::{FormatError, SchJunctionRow, SchWireRow, SnxTable};

const UUID: &str = "11111111-2222-3333-4444-555555555555";

fn wire_cells() -> Vec<&'static str> {
    vec![UUID, "N1", "10", "20", "30", "40", "0.25"]
}

fn assert_field_error<R: SnxTable + std::fmt::Debug>(
    cells: &[&str],
    column: usize,
    field: &str,
    bad: &str,
    block: &str,
    row: usize,
) {
    let mut cells = cells.to_vec();
    cells[column] = bad;
    match R::from_row(&cells, block, row) {
        Err(FormatError::TsvFieldParse {
            block: got_block,
            row: got_row,
            field: got,
            ..
        }) => {
            assert_eq!(got_block, block);
            assert_eq!(got_row, row);
            assert_eq!(got, field);
        }
        other => panic!("{field}={bad:?}: expected TsvFieldParse, got {other:?}"),
    }
}

fn assert_wire_error(cells: &[&str], column: usize, field: &str, bad: &str) {
    assert_field_error::<SchWireRow>(cells, column, field, bad, "wires", 3);
}

#[test]
fn valid_wire_row_parses() {
    let row = SchWireRow::from_row(&wire_cells(), "wires", 0).expect("valid row");
    assert_eq!((row.start_x, row.end_y), (10, 40));
}

#[test]
fn wire_bad_uuid_is_reported() {
    for bad in ["", "not-a-uuid", "1111"] {
        assert_wire_error(&wire_cells(), 0, "uuid", bad);
    }
}

#[test]
fn wire_bad_integer_is_reported() {
    for (col, field) in [(2, "start_x"), (3, "start_y"), (4, "end_x"), (5, "end_y")] {
        for bad in ["", "abc", "1.5", "99999999999999999999"] {
            assert_wire_error(&wire_cells(), col, field, bad);
        }
    }
}

#[test]
fn wire_bad_float_is_reported() {
    for bad in [
        "wide",
        "",
        "NaN",
        "nan",
        "inf",
        "-inf",
        "infinity",
        "-Infinity",
    ] {
        assert_wire_error(&wire_cells(), 6, "stroke_width", bad);
    }
}

#[test]
fn junction_bad_cells_are_reported() {
    let ok = [UUID, "1", "2", "0.5"];
    assert!(SchJunctionRow::from_row(&ok, "junctions", 0).is_ok());
    for (col, field) in [(0, "uuid"), (1, "pos_x"), (2, "pos_y"), (3, "diameter")] {
        for bad in ["zzz", "abc", "1.2.3"] {
            assert_field_error::<SchJunctionRow>(&ok, col, field, bad, "junctions", 0);
        }
    }
    for bad in ["", "NaN", "inf", "infinity"] {
        assert_field_error::<SchJunctionRow>(&ok, 3, "diameter", bad, "junctions", 0);
    }
}
