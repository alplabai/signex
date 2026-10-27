//! #416: single-row edits copy untouched rows through as raw bytes.

use super::*;

fn three_rows(path: &std::path::Path) -> [ComponentRow; 3] {
    let rows = [
        mk_row("R1", "resistor"),
        mk_row("R2", "resistor"),
        mk_row("R3", "resistor"),
    ];
    write_table(path, &rows).unwrap();
    rows
}

/// Rewrite the first data record's terminator as CRLF, a byte-level detail a
/// parse + re-serialise would normalise away.
fn crlf_first_record(path: &std::path::Path) {
    let text = std::fs::read_to_string(path).unwrap();
    let first_nl = text.find('\n').unwrap();
    let second_nl = first_nl + 1 + text[first_nl + 1..].find('\n').unwrap();
    let mut out = text.clone();
    out.insert(second_nl, '\r');
    std::fs::write(path, out).unwrap();
}

#[test]
fn update_leaves_other_rows_byte_identical() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let [r1, r2, mut r3] = three_rows(tmp.path());
    crlf_first_record(tmp.path());
    r3.internal_pn = InternalPn::new("R3_EDIT");
    update_row(tmp.path(), &r3).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert!(bytes.windows(2).any(|w| w == b"\r\n"));
    assert_eq!(read_table(tmp.path()).unwrap(), vec![r1, r2, r3]);
}

#[test]
fn delete_leaves_other_rows_byte_identical() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let [r1, r2, r3] = three_rows(tmp.path());
    crlf_first_record(tmp.path());
    delete_row(tmp.path(), RowId::from(r2.row_id)).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert!(bytes.windows(2).any(|w| w == b"\r\n"));
    assert_eq!(read_table(tmp.path()).unwrap(), vec![r1, r3]);
}

#[test]
fn append_after_unterminated_last_record() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let [r1, r2, r3] = three_rows(tmp.path());
    let mut bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(bytes.pop(), Some(b'\n'));
    std::fs::write(tmp.path(), &bytes).unwrap();
    let r4 = mk_row("R4", "resistor");
    append_row(tmp.path(), &r4).unwrap();
    assert_eq!(read_table(tmp.path()).unwrap(), vec![r1, r2, r3, r4]);
}

#[test]
fn quoted_multiline_cell_does_not_confuse_record_boundaries() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut a = mk_row("R1", "resistor");
    a.parameters
        .insert("note".into(), ParamValue::Text("line1\nline2\tx".into()));
    let b = mk_row("R2", "resistor");
    write_table(tmp.path(), &[a.clone(), b.clone()]).unwrap();
    let mut b2 = b.clone();
    b2.version = "0.0.2".into();
    update_row(tmp.path(), &b2).unwrap();
    delete_row(tmp.path(), RowId::from(b.row_id)).unwrap();
    assert_eq!(read_table(tmp.path()).unwrap(), vec![a]);
}

#[test]
fn append_with_alternates_to_plain_table_widens_header() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let r1 = mk_row("R1", "resistor");
    write_table(tmp.path(), std::slice::from_ref(&r1)).unwrap();
    let mut r2 = mk_row("R2", "resistor");
    r2.alternate_symbols = vec![crate::alternate_symbol::AlternateSymbol::new(
        PrimitiveRef::new(Uuid::nil(), Uuid::new_v4()),
        "0.0.2",
        "IEC",
    )];
    append_row(tmp.path(), &r2).unwrap();
    assert_eq!(read_table(tmp.path()).unwrap(), vec![r1, r2]);
}
