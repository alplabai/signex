use super::*;
use crate::manufacturer::ManufacturerPart;
use crate::param::ParamValue;
use chrono::{Duration, TimeZone};
use uuid::Uuid;

use crate::validate::validate_row_for_write;
use csv::{QuoteStyle, ReaderBuilder, WriterBuilder};

/// Parse a whole TSV table the way the `.snxlib` codec parses a table body.
fn read_table(path: &std::path::Path) -> Result<Vec<ComponentRow>, LibraryError> {
    let bytes = std::fs::read(path).map_err(LibraryError::Io)?;
    let mut rdr = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_reader(&bytes[..]);
    let headers = rdr
        .headers()
        .map_err(|e| LibraryError::Backend(format!("read headers: {e}")))?;
    let columns: Vec<&str> = headers.iter().collect();
    check_columns(&columns)
        .map_err(|e| LibraryError::Backend(format!("table schema mismatch: {e}")))?;
    let mut rows = Vec::new();
    for record in rdr.records() {
        let record = record.map_err(|e| LibraryError::Backend(format!("read row: {e}")))?;
        rows.push(record_to_row(&record)?);
    }
    Ok(rows)
}

/// Serialise `rows` as a TSV table, appending the alternate-symbols column
/// only when some row has one.
fn write_table(path: &std::path::Path, rows: &[ComponentRow]) -> Result<(), LibraryError> {
    let mut wtr = WriterBuilder::new()
        .delimiter(b'\t')
        .quote_style(QuoteStyle::Necessary)
        .from_writer(Vec::<u8>::new());
    let with_alternates = rows.iter().any(|r| !r.alternate_symbols.is_empty());
    let mut header: Vec<&str> = TABLE_HEADER.to_vec();
    if with_alternates {
        header.push(ALTERNATE_SYMBOLS_COLUMN);
    }
    wtr.write_record(&header)
        .map_err(|e| LibraryError::Backend(format!("write header: {e}")))?;
    for row in rows {
        let mut record = row_to_record(row)?;
        if with_alternates {
            record.push(alternate_symbols_cell(row)?.unwrap_or_default());
        }
        wtr.write_record(record)
            .map_err(|e| LibraryError::Backend(format!("write row: {e}")))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| LibraryError::Backend(format!("flush table: {e}")))?;
    std::fs::write(path, bytes).map_err(LibraryError::Io)
}

fn mk_row(internal_pn: &str, class: &str) -> ComponentRow {
    let lib = Uuid::nil();
    // Anchor timestamps so equality checks aren't flaky under
    // sub-millisecond drift between `Utc::now()` calls.
    let created = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
    let updated = created + Duration::hours(1);
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::new_v4(),
        internal_pn: InternalPn::new(internal_pn),
        class: ComponentClass::new(class),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
        sim_ref: None,
        pin_map_overrides: vec![PinPadOverride::new("EP", "EP1")],
        primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
        alternates: Vec::new(),
        supply: vec![DistributorListing::new("DigiKey", "DK-1")],
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created,
        updated,
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

/// Plan §6 step 1.3 — the foundational TSV round-trip.
#[test]
fn tsv_roundtrip_preserves_row() {
    let rows = vec![mk_row("R10K", "resistor"), mk_row("C100N", "capacitor")];
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_table(tmp.path(), &rows).unwrap();
    let back = read_table(tmp.path()).unwrap();
    assert_eq!(rows, back);
}

/// Several labelled documents survive a write and read, primary first
/// (#1231).
#[test]
fn tsv_roundtrip_preserves_every_document() {
    let mut row = mk_row("U1", "opamp");
    row.set_documents(vec![
        DatasheetDoc::new("TI", DatasheetRef::url("https://ti.example/u1.pdf")),
        DatasheetDoc::new("Errata", DatasheetRef::hash_pinned("ab12", "errata.pdf")),
        DatasheetDoc::new("", DatasheetRef::url("https://st.example/u1.pdf")),
    ]);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_table(tmp.path(), std::slice::from_ref(&row)).unwrap();
    let back = read_table(tmp.path()).unwrap();
    assert_eq!(back, vec![row.clone()]);
    assert_eq!(back[0].datasheet_label, "TI");
    assert_eq!(back[0].more_datasheets.len(), 2);
}

/// The description is a plain schema column and round-trips with quotes
/// and non-ASCII text.
#[test]
fn a_description_round_trips() {
    let mut row = mk_row("R1", "resistor");
    row.description = "100 nF, \"X7R\", 50 V µ".into();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_table(tmp.path(), std::slice::from_ref(&row)).unwrap();
    assert_eq!(read_table(tmp.path()).unwrap(), vec![row]);
}

/// A row with one unlabelled document keeps the single-object cell it
/// has always had, so existing tables are rewritten byte for byte.
#[test]
fn a_single_document_keeps_the_object_cell() {
    let row = mk_row("R1", "resistor");
    let cells = row_to_record(&row).unwrap();
    assert_eq!(
        cells[3],
        r#"{"kind":"url","url":"https://example.com/ds.pdf"}"#
    );
    let mut labelled = row.clone();
    labelled.datasheet_label = "Vishay".into();
    assert!(row_to_record(&labelled).unwrap()[3].starts_with('['));
}

/// An empty list cell reads as a row with no datasheet.
#[test]
fn an_empty_document_list_reads_as_no_datasheet() {
    assert!(datasheet_from_cell("[]").unwrap().is_empty());
    let mut row = mk_row("R1", "resistor");
    row.set_documents(Vec::new());
    assert!(row.datasheet.is_empty());
    assert!(row.datasheet_label.is_empty() && row.more_datasheets.is_empty());
}

/// #1056: finite numbers and measurements still round-trip unchanged.
#[test]
fn write_table_round_trips_finite_parameters() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("table.tsv");
    let mut row = mk_row("R1", "resistor");
    row.parameters.insert("n".into(), ParamValue::Number(-0.5));
    row.parameters.insert(
        "m".into(),
        ParamValue::Measurement {
            value: 10_000.0,
            unit: "ohm".into(),
        },
    );
    write_table(&path, std::slice::from_ref(&row)).unwrap();
    assert_eq!(read_table(&path).unwrap(), vec![row]);
}

/// `none` footprint and sim refs encode as empty strings; round-trip
/// preserves the `None` shape.
#[test]
fn none_primitive_refs_encode_empty() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut r = mk_row("RX", "resistor");
    r.footprint_ref = None;
    r.sim_ref = None;
    write_table(tmp.path(), &[r.clone()]).unwrap();
    let back = read_table(tmp.path()).unwrap();
    assert_eq!(back, vec![r]);
}

/// Hash hex encode/decode is bit-exact.
#[test]
fn hash_round_trip_preserves_bytes() {
    let mut h = [0u8; 32];
    for (i, b) in h.iter_mut().enumerate() {
        *b = i as u8;
    }
    let s = hash_to_cell(&h);
    let back = hash_from_cell(&s).unwrap();
    assert_eq!(h, back);
}

/// A 64-BYTE cell whose bytes are not all ASCII hex is a recoverable
/// error, never a char-boundary panic.
#[test]
fn hash_from_cell_rejects_multibyte_cell() {
    let cell = format!("{}{}", "a".repeat(61), '€');
    assert_eq!(cell.len(), 64, "cell must be 64 BYTES to reach the slicing");
    assert!(matches!(
        hash_from_cell(&cell),
        Err(LibraryError::InvalidData(_))
    ));
    assert!(matches!(
        hash_from_cell(&"z".repeat(64)),
        Err(LibraryError::InvalidData(_))
    ));
}

#[test]
fn schema_constant_matches_header_length() {
    assert_eq!(TABLE_HEADER.len(), TableSchema::ROW.columns.len());
}

/// #1056: a NaN parameter is refused at the write gate, naming the row and
/// the key.
#[test]
fn a_nan_parameter_is_refused_before_write() {
    let mut bad = mk_row("R_BAD", "resistor");
    bad.parameters
        .insert("tolerance".into(), ParamValue::Number(f64::NAN));
    let msg = validate_row_for_write(&bad).unwrap_err().to_string();
    assert!(msg.contains("R_BAD"), "row not named: {msg}");
    assert!(msg.contains("tolerance"), "key not named: {msg}");
}

/// #1056: an infinite Measurement is refused too.
#[test]
fn an_infinite_measurement_is_refused_before_write() {
    let mut bad = mk_row("C1", "capacitor");
    bad.parameters.insert(
        "voltage".into(),
        ParamValue::Measurement {
            value: f64::NEG_INFINITY,
            unit: "V".into(),
        },
    );
    let err = validate_row_for_write(&bad).unwrap_err();
    assert!(err.to_string().contains("voltage"), "got {err}");
}
