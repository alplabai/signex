//! Tests for the cached-pricing sidecar of a `.snxlib` table.
use super::*;
use crate::distributor::PriceBreak;
use crate::pricing::CachedPricing;
use chrono::TimeZone;

const ROW_ID: &str = "0192a8c0-1111-7000-8000-000000000001";

fn manifest() -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::parse_str("0192a8c0-0000-7000-8000-000000000000").unwrap(),
        library: LibrarySection {
            name: "MyLib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    }
}

fn table_with_row() -> LibraryTable {
    let columns = vec!["row_id".to_string(), "mpn".to_string()];
    let mut row = LibraryRow::default();
    row.cells.insert("row_id".into(), ROW_ID.into());
    row.cells.insert("mpn".into(), "RC0603".into());
    LibraryTable {
        columns,
        rows: vec![row],
        ..Default::default()
    }
}

fn priced(unit_price: f64) -> CachedPricing {
    CachedPricing {
        sku: "311-10.0KHRCT-ND".into(),
        currency: "USD".into(),
        stock: Some(50_000),
        breaks: vec![
            PriceBreak { qty: 1, unit_price },
            PriceBreak {
                qty: 100,
                unit_price: 1e-7,
            },
            PriceBreak {
                qty: 1000,
                unit_price: 2.0,
            },
        ],
        fetched_at: chrono::Utc
            .with_ymd_and_hms(2026, 10, 24, 9, 30, 15)
            .unwrap()
            + chrono::Duration::nanoseconds(123_456_789),
    }
}

fn library(pricing: Option<(&str, CachedPricing)>) -> LibraryFile {
    let mut table = table_with_row();
    if let Some((distributor, cached)) = pricing {
        table
            .pricing
            .entry(ROW_ID.into())
            .or_default()
            .insert(distributor.into(), cached);
    }
    LibraryFile {
        manifest: manifest(),
        tables: BTreeMap::from([("resistors".to_string(), table)]),
    }
}

#[test]
fn library_without_pricing_writes_no_pricing_section_and_is_byte_stable() {
    let text = library(None).write().unwrap();
    assert!(
        !text.contains("pricing"),
        "no sidecar when there is no price:\n{text}"
    );
    let reparsed = LibraryFile::parse(&text).unwrap();
    assert_eq!(
        reparsed.write().unwrap(),
        text,
        "re-save must be byte-identical"
    );
}

#[test]
fn pre_pricing_file_loads_and_resaves_unchanged() {
    // A file as written before the sidecar existed: no `pricing` anywhere.
    let legacy = library(None).write().unwrap();
    let lib = LibraryFile::parse(&legacy).unwrap();
    assert!(lib.tables["resistors"].pricing.is_empty());
    assert_eq!(lib.write().unwrap(), legacy);
}

#[test]
fn pricing_round_trips_exactly() {
    // A distributor name that needs quoting, whole-number and tiny prices,
    // and a timestamp with nanoseconds all have to survive.
    let lib = library(Some(("Weird Name \"1\"", priced(1.0))));
    let text = lib.write().unwrap();
    let back = LibraryFile::parse(&text).unwrap();
    assert_eq!(back, lib);
    assert_eq!(back.write().unwrap(), text);
    let got = &back.tables["resistors"].pricing[ROW_ID]["Weird Name \"1\""];
    assert_eq!(got.breaks[0].unit_price, 1.0);
    assert_eq!(got.breaks[1].unit_price, 1e-7);
    assert_eq!(got.fetched_at, priced(1.0).fetched_at);
}

#[test]
fn pricing_does_not_disturb_the_tsv_block() {
    let plain = library(None).write().unwrap();
    let with = library(Some(("DigiKey", priced(0.1)))).write().unwrap();
    assert!(
        with.starts_with(&plain),
        "pricing is appended after the existing text"
    );
}

#[test]
fn non_finite_price_is_refused_on_write() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = library(Some(("DigiKey", priced(bad)))).write().unwrap_err();
        assert!(
            matches!(err, LibraryFileError::InvalidPricing { .. }),
            "{bad}: {err}"
        );
    }
}

#[test]
fn non_finite_price_is_refused_on_parse() {
    let good = library(Some(("DigiKey", priced(0.1)))).write().unwrap();
    for literal in ["nan", "inf", "-inf"] {
        let bad = good.replace("unit_price = 0.1", &format!("unit_price = {literal}"));
        assert_ne!(bad, good, "fixture must contain the edited price");
        let err = LibraryFile::parse(&bad).unwrap_err();
        assert!(
            matches!(err, LibraryFileError::InvalidPricing { .. }),
            "{literal}: {err}"
        );
    }
}
