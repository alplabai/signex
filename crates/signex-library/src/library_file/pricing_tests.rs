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
            .entry(distributor.into())
            .or_default()
            .insert(cached.sku.clone(), cached);
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
    let got = &back.tables["resistors"].pricing[ROW_ID]["Weird Name \"1\""]["311-10.0KHRCT-ND"];
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

#[test]
fn one_damaged_record_is_dropped_and_the_library_still_opens() {
    let mut lib = library(Some(("DigiKey", priced(0.1))));
    let other = CachedPricing {
        sku: "TR-ND".into(),
        ..priced(0.2)
    };
    lib.tables
        .get_mut("resistors")
        .unwrap()
        .pricing
        .get_mut(ROW_ID)
        .unwrap()
        .get_mut("DigiKey")
        .unwrap()
        .insert("TR-ND".into(), other);
    let text = lib.write().unwrap();
    // Damage only the reel record: an impossible currency.
    let (head, tail) = text.split_once("[tables.resistors.pricing.").unwrap();
    let damaged = format!(
        "{head}[tables.resistors.pricing.{}",
        tail.replacen("currency = \"USD\"", "currency = \"dollars\"", 1)
    );
    assert_ne!(damaged, text);
    let back = LibraryFile::parse(&damaged).expect("a bad cache record must not block the library");
    let per_sku = &back.tables["resistors"].pricing[ROW_ID]["DigiKey"];
    assert_eq!(per_sku.len(), 1, "only the damaged record is gone");
}

#[test]
fn a_record_missing_a_field_is_dropped_not_fatal() {
    let text = library(Some(("DigiKey", priced(0.1)))).write().unwrap();
    let damaged = text.replace("fetched_at = ", "fetched_when = ");
    let back = LibraryFile::parse(&damaged).expect("opens");
    assert!(back.tables["resistors"].pricing.is_empty());
    assert_eq!(
        back.tables["resistors"].rows.len(),
        1,
        "the rows are untouched"
    );
}

#[test]
fn two_listings_of_one_distributor_round_trip_as_separate_records() {
    let mut lib = library(Some(("DigiKey", priced(0.1))));
    lib.tables
        .get_mut("resistors")
        .unwrap()
        .pricing
        .get_mut(ROW_ID)
        .unwrap()
        .get_mut("DigiKey")
        .unwrap()
        .insert(
            "TR-ND".into(),
            CachedPricing {
                sku: "TR-ND".into(),
                ..priced(0.01)
            },
        );
    let text = lib.write().unwrap();
    let back = LibraryFile::parse(&text).unwrap();
    assert_eq!(back, lib);
    assert_eq!(back.write().unwrap(), text);
}

#[test]
fn a_cache_entry_that_is_not_a_table_is_dropped_at_any_level() {
    let good = library(Some(("DigiKey", priced(0.1)))).write().unwrap();
    // A scalar where a row table belongs, and a scalar where a distributor
    // table belongs; the intact record next to them survives.
    let damaged = format!(
        "{good}\n[tables.resistors.pricing]\nstray-row = 1\n\n\
         [tables.resistors.pricing.{ROW_ID}]\nMouser = \"x\"\n"
    );
    let back = LibraryFile::parse(&damaged).expect("a damaged cache must not lock the library out");
    let pricing = &back.tables["resistors"].pricing;
    assert!(!pricing.contains_key("stray-row"));
    assert!(!pricing[ROW_ID].contains_key("Mouser"));
    assert_eq!(pricing[ROW_ID]["DigiKey"].len(), 1);
    assert_eq!(back.tables["resistors"].rows.len(), 1);
}

#[test]
fn a_non_finite_price_refuses_the_load_even_when_the_record_is_also_damaged() {
    let good = library(Some(("DigiKey", priced(0.1)))).write().unwrap();
    let bad = good
        .replace("unit_price = 0.1", "unit_price = nan")
        .replace("fetched_at = ", "fetched_when = ");
    assert!(matches!(
        LibraryFile::parse(&bad).unwrap_err(),
        LibraryFileError::InvalidPricing { .. }
    ));
}

#[test]
fn two_spellings_of_one_part_number_on_disk_load_as_one_record() {
    let text = library(Some(("DigiKey", priced(0.1)))).write().unwrap();
    let start = text.find("[tables.resistors.pricing.").unwrap();
    let block = &text[start..];
    let lower = block
        .replace("\"311-10.0KHRCT-ND\"]", "\"311-10.0khrct-nd\"]")
        .replace("sku = \"311-10.0KHRCT-ND\"", "sku = \"311-10.0khrct-nd\"");
    assert_ne!(lower, block);
    let doubled = format!(
        "{text}
{lower}"
    );
    let back = LibraryFile::parse(&doubled).unwrap();
    assert_eq!(back.tables["resistors"].pricing[ROW_ID]["DigiKey"].len(), 1);
}
