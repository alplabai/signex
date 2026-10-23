//! Field-level corruption: a single bad cell in an otherwise valid row must
//! surface as `FormatError::TsvFieldParse` naming that field, never a panic
//! or a silently defaulted value.

use signex_types::format::{FormatError, SchJunctionRow, SchWireRow, SnxTable};

const UUID: &str = "11111111-2222-3333-4444-555555555555";

fn wire_cells() -> Vec<&'static str> {
    vec![UUID, "N1", "10", "20", "30", "40", "0.25"]
}

fn assert_field_error(cells: &[&str], column: usize, field: &str, bad: &str) {
    let mut cells = cells.to_vec();
    cells[column] = bad;
    match SchWireRow::from_row(&cells, "wires", 3) {
        Err(FormatError::TsvFieldParse {
            block,
            row,
            field: got,
            ..
        }) => {
            assert_eq!(block, "wires");
            assert_eq!(row, 3);
            assert_eq!(got, field);
        }
        other => panic!("{field}={bad:?}: expected TsvFieldParse, got {other:?}"),
    }
}

#[test]
fn valid_wire_row_parses() {
    let row = SchWireRow::from_row(&wire_cells(), "wires", 0).expect("valid row");
    assert_eq!((row.start_x, row.end_y), (10, 40));
}

#[test]
fn wire_bad_uuid_is_reported() {
    for bad in ["", "not-a-uuid", "1111"] {
        assert_field_error(&wire_cells(), 0, "uuid", bad);
    }
}

#[test]
fn wire_bad_integer_is_reported() {
    for (col, field) in [(2, "start_x"), (3, "start_y"), (4, "end_x"), (5, "end_y")] {
        for bad in ["", "abc", "1.5", "99999999999999999999"] {
            assert_field_error(&wire_cells(), col, field, bad);
        }
    }
}

#[test]
fn wire_bad_float_is_reported() {
    assert_field_error(&wire_cells(), 6, "stroke_width", "wide");
}

#[test]
fn junction_bad_cells_are_reported() {
    let ok = [UUID, "1", "2", "0.5"];
    assert!(SchJunctionRow::from_row(&ok, "junctions", 0).is_ok());
    for (col, field) in [(0, "uuid"), (1, "pos_x"), (2, "pos_y"), (3, "diameter")] {
        for bad in ["zzz", "abc", "1.2.3"] {
            let mut cells = ok;
            cells[col] = bad;
            match SchJunctionRow::from_row(&cells, "junctions", 0) {
                Err(FormatError::TsvFieldParse {
                    block,
                    row,
                    field: got,
                    ..
                }) => {
                    assert_eq!(block, "junctions");
                    assert_eq!(row, 0);
                    assert_eq!(got, field);
                }
                other => panic!("{field}={bad:?}: expected TsvFieldParse, got {other:?}"),
            }
        }
    }
}
