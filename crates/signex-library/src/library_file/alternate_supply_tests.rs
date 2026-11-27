//! Tests for #3046 in a `.snxlib`: the `snxlib/2.1` token a hand-entered
//! alternate stock or lifecycle selects, and the fetched `alternate_supply`
//! sidecar.
use super::*;
use crate::alternate_supply::FetchedSupply;
use crate::manufacturer::{ManufacturerLifecycle, ManufacturerPart};
use chrono::TimeZone;
use signex_types::format_version::FormatVersionError;

const ROW_ID: &str = "0192a8c0-1111-7000-8000-000000000001";

fn manifest() -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::parse_str("0192a8c0-0000-7000-8000-000000000000").unwrap(),
        library: LibrarySection {
            name: "MyLib".into(),
            description: None,
            ..Default::default()
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    }
}

/// One row whose `alternates` cell holds `alternate`.
fn library(alternate: &ManufacturerPart) -> LibraryFile {
    let columns = vec!["row_id".to_string(), "alternates".to_string()];
    let mut row = LibraryRow::default();
    row.cells.insert("row_id".into(), ROW_ID.into());
    let cell = serde_json::to_string(&vec![alternate.clone()]).unwrap();
    row.cells.insert("alternates".into(), cell);
    let table = LibraryTable {
        columns,
        rows: vec![Arc::new(row)],
        ..Default::default()
    };
    LibraryFile {
        layout: Default::default(),
        manifest: manifest(),
        tables: BTreeMap::from([("parts".to_string(), table)]),
    }
}

fn plain() -> ManufacturerPart {
    ManufacturerPart::draft("Sample Inc", "ALT-1")
}

fn manual() -> ManufacturerPart {
    ManufacturerPart {
        manual_stock: Some(1200),
        manual_lifecycle: Some(ManufacturerLifecycle::Eol),
        ..plain()
    }
}

fn fetched() -> FetchedSupply {
    FetchedSupply {
        stock: Some(86_400),
        lifecycle: Some(ManufacturerLifecycle::Active),
        fetched_at: chrono::Utc.with_ymd_and_hms(2026, 1, 3, 10, 0, 0).unwrap(),
    }
}

#[test]
fn a_library_without_manual_supply_stays_snxlib_2_byte_for_byte() {
    let text = library(&plain()).write().unwrap();
    assert!(text.starts_with("format = \"snxlib/2\"\n"), "{text}");
    let reread = LibraryFile::parse(&text).unwrap();
    assert_eq!(reread.write().unwrap(), text);
}

#[test]
fn a_manual_stock_or_lifecycle_writes_snxlib_2_1_and_survives_a_reload() {
    for part in [
        ManufacturerPart {
            manual_stock: Some(5),
            ..plain()
        },
        ManufacturerPart {
            manual_lifecycle: Some(ManufacturerLifecycle::Nrnd),
            ..plain()
        },
        manual(),
    ] {
        let text = library(&part).write().unwrap();
        assert!(text.starts_with("format = \"snxlib/2.1\"\n"), "{text}");
        let reread = LibraryFile::parse(&text).unwrap();
        assert_eq!(reread.manifest.format, FORMAT_TOKEN_MANUAL_SUPPLY);
        let cell = &reread.tables["parts"].rows[0].cells["alternates"];
        let back: Vec<ManufacturerPart> = serde_json::from_str(cell).unwrap();
        assert_eq!(back, vec![part]);
        assert_eq!(reread.write().unwrap(), text, "re-save is byte for byte");
    }
}

#[test]
fn clearing_the_last_manual_value_writes_snxlib_2_again() {
    let text = library(&manual()).write().unwrap();
    let mut reread = LibraryFile::parse(&text).unwrap();
    let cell = serde_json::to_string(&vec![plain()]).unwrap();
    Arc::make_mut(&mut reread.tables.get_mut("parts").unwrap().rows[0])
        .cells
        .insert("alternates".into(), cell);
    let lowered = reread.write().unwrap();
    assert_eq!(lowered, library(&plain()).write().unwrap());
}

#[test]
fn a_build_that_reads_only_snxlib_2_refuses_2_1_by_name() {
    use signex_types::format_version::{FormatVersion, MissingToken, NewerMinor};
    // The policy of builds before #3046.
    let older = FormatVersion {
        current: FORMAT_TOKEN,
        readable: &[FORMAT_TOKEN],
        newer_minor: NewerMinor::Refused,
        missing: MissingToken::Refused,
    };
    let err = older.read(Some(FORMAT_TOKEN_MANUAL_SUPPLY)).unwrap_err();
    assert!(matches!(err, FormatVersionError::Newer { .. }), "{err:?}");
    assert!(err.to_string().contains("snxlib/2.1"), "{err}");
    // This build reads both, and refuses the next minor by name.
    assert!(FORMAT_VERSION.read(Some(FORMAT_TOKEN)).is_ok());
    assert!(
        FORMAT_VERSION
            .read(Some(FORMAT_TOKEN_MANUAL_SUPPLY))
            .is_ok()
    );
    let err = FORMAT_VERSION.read(Some("snxlib/2.2")).unwrap_err();
    assert!(err.to_string().contains("snxlib/2.2"), "{err}");
}

#[test]
fn a_note_quoting_the_key_does_not_raise_the_token() {
    let part = ManufacturerPart {
        notes: Some("see \"manual_stock\": in the spec".into()),
        ..plain()
    };
    let text = library(&part).write().unwrap();
    assert!(text.starts_with("format = \"snxlib/2\"\n"), "{text}");
}

#[test]
fn fetched_alternate_supply_round_trips_and_keeps_the_token() {
    let mut file = library(&plain());
    file.tables
        .get_mut("parts")
        .unwrap()
        .alternate_supply
        .entry(ROW_ID.into())
        .or_default()
        .entry("Mouser".into())
        .or_default()
        .insert("ALT-1".into(), fetched());
    let text = file.write().unwrap();
    assert!(text.starts_with("format = \"snxlib/2\"\n"), "{text}");
    assert!(
        text.ends_with(
            "\n[tables.parts.alternate_supply.0192a8c0-1111-7000-8000-000000000001.Mouser.ALT-1]\n\
             stock = 86400\n\
             lifecycle = \"Active\"\n\
             fetched_at = \"2026-01-03T10:00:00Z\"\n"
        ),
        "{text}"
    );
    let reread = LibraryFile::parse(&text).unwrap();
    assert_eq!(reread, file);
    assert_eq!(reread.write().unwrap(), text);
}

#[test]
fn a_record_with_unknown_values_keeps_only_what_it_knows() {
    let mut file = library(&plain());
    let only_time = FetchedSupply {
        stock: None,
        lifecycle: None,
        ..fetched()
    };
    file.tables
        .get_mut("parts")
        .unwrap()
        .alternate_supply
        .entry(ROW_ID.into())
        .or_default()
        .entry("DigiKey".into())
        .or_default()
        .insert("ALT-1".into(), only_time);
    let text = file.write().unwrap();
    assert!(!text.contains("stock ="), "{text}");
    assert!(!text.contains("lifecycle ="), "{text}");
    assert_eq!(LibraryFile::parse(&text).unwrap(), file);
}

#[test]
fn a_damaged_record_is_dropped_and_the_library_opens() {
    let good = library(&plain()).write().unwrap();
    let text = format!(
        "{good}\n[tables.parts.alternate_supply.{ROW_ID}.Mouser.ALT-1]\n\
         stock = -4\nfetched_at = \"2026-01-03T10:00:00Z\"\n\
         \n[tables.parts.alternate_supply.{ROW_ID}.Mouser.ALT-2]\n\
         lifecycle = \"Withdrawn\"\nfetched_at = \"2026-01-03T10:00:00Z\"\n"
    );
    let file = LibraryFile::parse(&text).unwrap();
    assert!(file.tables["parts"].alternate_supply.is_empty());
}

#[test]
fn a_kept_text_library_takes_the_new_token_when_a_manual_value_arrives() {
    // An unknown key makes the writer edit the read text in place (#1435).
    let text = library(&plain()).write().unwrap().replacen(
        "[library]\n",
        "[library]\nfuture_key = 1\n",
        1,
    );
    let mut file = LibraryFile::parse(&text).unwrap();
    assert!(file.layout.keeps_text());
    let cell = serde_json::to_string(&vec![manual()]).unwrap();
    Arc::make_mut(&mut file.tables.get_mut("parts").unwrap().rows[0])
        .cells
        .insert("alternates".into(), cell);
    let written = file.write().unwrap();
    assert!(
        written.starts_with("format = \"snxlib/2.1\"\n"),
        "{written}"
    );
    assert!(written.contains("future_key = 1"), "{written}");
}
