//! TSV wire format for `alternate_symbols`: optional 22nd column.

use chrono::Utc;
use signex_library::*;
use uuid::Uuid;

fn row(pn: &str) -> ComponentRow {
    let lib = Uuid::new_v4();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url(""),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn with_alternate(mut r: ComponentRow) -> ComponentRow {
    r.alternate_symbols.push(AlternateSymbol::new(
        PrimitiveRef::new(r.symbol_ref.library_id, Uuid::new_v4()),
        "0.0.2",
        "IEC",
    ));
    r
}

#[test]
fn table_without_alternates_keeps_the_legacy_21_column_layout() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_table(tmp.path(), &[row("A"), row("B")]).unwrap();
    let text = std::fs::read_to_string(tmp.path()).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next().unwrap(), TABLE_HEADER.join("\t"));
    for line in lines {
        assert_eq!(line.split('\t').count(), TABLE_HEADER.len());
    }
}

#[test]
fn legacy_21_column_file_reads_with_no_alternates() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let original = row("A");
    write_table(tmp.path(), std::slice::from_ref(&original)).unwrap();
    let back = read_table(tmp.path()).unwrap();
    assert_eq!(back, vec![original]);
    assert!(back[0].alternate_symbols.is_empty());
}

#[test]
fn alternates_round_trip_through_the_22_column_layout() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let rows = vec![with_alternate(row("A")), row("B")];
    write_table(tmp.path(), &rows).unwrap();
    let text = std::fs::read_to_string(tmp.path()).unwrap();
    let header = text.lines().next().unwrap();
    assert!(header.ends_with(ALTERNATE_SYMBOLS_COLUMN));
    assert_eq!(header.split('\t').count(), TABLE_HEADER.len() + 1);
    assert_eq!(read_table(tmp.path()).unwrap(), rows);
}

#[test]
fn any_other_column_layout_still_errors() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let header = format!("{}\textra\tmore\n", TABLE_HEADER.join("\t"));
    std::fs::write(tmp.path(), header).unwrap();
    assert!(read_table(tmp.path()).is_err());

    let wrong_name = format!("{}\tnot_alternates\n", TABLE_HEADER.join("\t"));
    std::fs::write(tmp.path(), wrong_name).unwrap();
    assert!(read_table(tmp.path()).is_err());
}

#[test]
fn write_refuses_an_alternate_equal_to_the_default_symbol() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut r = row("A");
    r.alternate_symbols
        .push(AlternateSymbol::new(r.symbol_ref, "0.0.1", "same"));
    assert!(write_table(tmp.path(), &[r]).is_err());
}
