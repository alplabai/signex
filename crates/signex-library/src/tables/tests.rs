use super::*;
use crate::manufacturer::ManufacturerPart;
use crate::param::ParamValue;
use chrono::{Duration, TimeZone};
use uuid::Uuid;

/// True if `dir` contains a leftover atomic-write temp sibling
/// (`*.tmp`). `atomic_write` picks a unique per-writer name (#416),
/// so tests must scan for any match instead of a fixed name.
fn has_stray_tmp(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
}

/// RAII guard: while alive, `dir` rejects new-file creation, so
/// `atomic_write`'s `File::create(&tmp)` fails deterministically
/// regardless of the unique per-writer temp name it picks (#416).
/// Restores permissions on drop.
///
/// On Windows, `icacls`'s exit code is not proof the deny is enforced
/// yet — under full-workspace parallel test load its write to the
/// directory's security descriptor can lose the race against the very
/// next `File::create` in the same directory (#482), so `on` settles
/// the deny with a real probe write before returning.
struct DenyNewFiles {
    dir: std::path::PathBuf,
}

impl DenyNewFiles {
    fn on(dir: &std::path::Path) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555))
                .expect("chmod dir read-only for test");
        }
        #[cfg(windows)]
        {
            let user = std::env::var("USERNAME").unwrap_or_else(|_| "Everyone".into());
            let status = std::process::Command::new("icacls")
                .arg(dir)
                .arg("/deny")
                .arg(format!("{user}:(WD)"))
                .status()
                .expect("run icacls /deny for test");
            assert!(status.success(), "icacls /deny failed");
            Self::settle_deny(dir);
        }
        Self {
            dir: dir.to_path_buf(),
        }
    }

    /// Poll with a real probe write instead of trusting `icacls`'s exit
    /// code, so the caller never proceeds through a window where the
    /// directory still silently accepts new files (#482).
    #[cfg(windows)]
    fn settle_deny(dir: &std::path::Path) {
        let probe = dir.join(format!(".icacls-settle-probe-{}", std::process::id()));
        for _ in 0..100 {
            match std::fs::File::create(&probe) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&probe);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => return,
            }
        }
        panic!("icacls /deny on {dir:?} did not take effect within 1s (#482)");
    }
}

impl Drop for DenyNewFiles {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o755));
        }
        #[cfg(windows)]
        {
            let user = std::env::var("USERNAME").unwrap_or_else(|_| "Everyone".into());
            let _ = std::process::Command::new("icacls")
                .arg(&self.dir)
                .arg("/remove:d")
                .arg(&user)
                .status();
        }
    }
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

/// Empty file (no header) reads back as empty vec.
#[test]
fn read_missing_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("does-not-exist.tsv");
    let rows = read_table(&path).unwrap();
    assert!(rows.is_empty());
}

/// `append_row` honours an empty file, then keeps growing it — and
/// leaves no stranded `.tmp` sibling behind.
#[test]
fn append_row_grows_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("table.tsv");
    let a = mk_row("R1", "resistor");
    let b = mk_row("R2", "resistor");
    append_row(&path, &a).unwrap();
    append_row(&path, &b).unwrap();
    let back = read_table(&path).unwrap();
    assert_eq!(back, vec![a, b]);
    assert!(!has_stray_tmp(dir.path()));
}

/// `write_table` must go through `atomic_write`, not `fs::write`:
/// a failed save leaves the previous table fully intact.
///
/// Discriminator: denying new-file creation in the destination's
/// parent directory makes `atomic_write`'s `File::create(&tmp)` fail
/// before it can touch the destination, regardless of the unique
/// per-writer temp name it picks (#416). A plain `fs::write` would
/// ignore that and clobber the old rows — so this test fails on a
/// revert.
#[test]
fn write_table_is_atomic_and_preserves_original_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("table.tsv");
    let original = vec![mk_row("R1", "resistor"), mk_row("R2", "resistor")];
    write_table(&path, &original).unwrap();

    let _deny = DenyNewFiles::on(dir.path());
    let err = write_table(&path, &[mk_row("C1", "capacitor")]).unwrap_err();
    assert!(matches!(err, LibraryError::Io(_)), "got {err:?}");

    assert_eq!(read_table(&path).unwrap(), original);
}

/// #1056: a NaN parameter is refused, the error names the row and the
/// key, and the file on disk is byte-identical afterwards.
#[test]
fn write_table_refuses_nan_parameter_and_keeps_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("table.tsv");
    write_table(&path, &[mk_row("R1", "resistor")]).unwrap();
    let before = std::fs::read(&path).unwrap();

    let mut bad = mk_row("R_BAD", "resistor");
    bad.parameters
        .insert("tolerance".into(), ParamValue::Number(f64::NAN));
    let err = write_table(&path, &[mk_row("R1", "resistor"), bad.clone()]).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("R_BAD"), "row not named: {msg}");
    assert!(msg.contains(&bad.row_id.to_string()), "row id: {msg}");
    assert!(msg.contains("tolerance"), "key not named: {msg}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(read_table(&path).is_ok());
}

/// #1056: an infinite Measurement is refused too.
#[test]
fn write_table_refuses_infinite_measurement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("table.tsv");
    write_table(&path, &[mk_row("C0", "capacitor")]).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut bad = mk_row("C1", "capacitor");
    bad.parameters.insert(
        "voltage".into(),
        ParamValue::Measurement {
            value: f64::NEG_INFINITY,
            unit: "V".into(),
        },
    );
    let err = write_table(&path, &[bad]).unwrap_err();
    assert!(err.to_string().contains("voltage"), "got {err}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
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

/// `delete_row` removes the matching id, leaves others alone.
#[test]
fn delete_row_removes_only_matching_id() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let a = mk_row("R1", "resistor");
    let b = mk_row("R2", "resistor");
    write_table(tmp.path(), &[a.clone(), b.clone()]).unwrap();
    delete_row(tmp.path(), RowId::from_uuid(a.row_id)).unwrap();
    let back = read_table(tmp.path()).unwrap();
    assert_eq!(back, vec![b]);
}

#[test]
fn delete_row_missing_returns_not_found() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let a = mk_row("R1", "resistor");
    write_table(tmp.path(), &[a]).unwrap();
    let err = delete_row(tmp.path(), RowId::new()).unwrap_err();
    assert!(matches!(err, LibraryError::NotFound(_)));
}

/// `update_row` replaces a row in-place by id.
#[test]
fn update_row_replaces_in_place() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut a = mk_row("R1", "resistor");
    let b = mk_row("R2", "resistor");
    write_table(tmp.path(), &[a.clone(), b.clone()]).unwrap();
    a.internal_pn = InternalPn::new("R1_RENAMED");
    update_row(tmp.path(), &a).unwrap();
    let back = read_table(tmp.path()).unwrap();
    assert_eq!(back, vec![a, b]);
}

#[test]
fn update_row_missing_returns_not_found() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let r = mk_row("R1", "resistor");
    let err = update_row(tmp.path(), &r).unwrap_err();
    assert!(matches!(err, LibraryError::NotFound(_)));
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
        Err(LibraryError::Backend(_))
    ));
    assert!(matches!(
        hash_from_cell(&"z".repeat(64)),
        Err(LibraryError::Backend(_))
    ));
}

#[test]
fn schema_constant_matches_header_length() {
    assert_eq!(TABLE_HEADER.len(), TableSchema::ROW.columns.len());
}
