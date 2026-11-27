//! Mockup row 208: the picker's candidates are the library alternates of the
//! part's own row first, then the search results over the project
//! libraries (DECISIONS 2026-11-26); the compare strip's pins and footprint
//! checks; what the variant gets from a candidate.

use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::lifecycle::LifecycleState;
use signex_library::manufacturer::{AlternateStatus, ManufacturerPart};
use signex_library::param::{ParamMap, ParamValue};
use signex_library::primitive::PrimitiveRef;
use signex_library::{CachedPricing, RowPricing};
use uuid::Uuid;

use super::candidates::{
    Candidate, CandidateQuery, CandidateSource, CandidateStatus, Candidates, PinCompare,
    alternate_values, collect, compare_pins, row_matches, stock_of,
};
use crate::library::test_adapters::PricedAdapter;

fn row(pn: &str, mpn: &str, value: &str, lib: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    let mut parameters = ParamMap::new();
    parameters.insert("Value".to_string(), ParamValue::Text(value.to_string()));
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("capacitor"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: format!("{value} 10V X5R"),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Murata", mpn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters,
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn alternate(manufacturer: &str, mpn: &str, status: AlternateStatus) -> ManufacturerPart {
    ManufacturerPart {
        manufacturer: manufacturer.to_string(),
        mpn: mpn.to_string(),
        status,
        notes: None,
        manual_stock: None,
        manual_lifecycle: None,
    }
}

fn query(row: Option<(Uuid, Uuid)>, text: &str) -> CandidateQuery {
    CandidateQuery {
        row,
        base_mpn: "GRM21BR61A226ME44".to_string(),
        query: text.to_string(),
        project_libraries: Vec::new(),
        open_libraries: Vec::new(),
        sheet: None,
        units: Vec::new(),
    }
}

fn no_other(_: Uuid) -> Option<Box<dyn signex_library::LibraryAdapter>> {
    None
}

#[test]
fn the_rows_alternates_come_first_then_the_search_results() {
    let lib = Uuid::now_v7();
    let mut base = row("C-22U", "GRM21BR61A226ME44", "22µ", lib);
    base.alternates = vec![
        alternate(
            "Samsung Electro-Mechanics",
            "CL21A226MPQNNNE",
            AlternateStatus::Approved,
        ),
        alternate("TDK", "C2012X5R1A226M125AB", AlternateStatus::Conditional),
    ];
    let other = row("C-22U-1206", "GRM31CR61A226ME19L", "22µ", lib);
    let unrelated = row("R-10K", "RC0603FR-0710KL", "10k", lib);
    let base_id = base.row_id;
    let adapter = PricedAdapter::new(lib).with_rows(vec![base, other, unrelated]);
    let mut out = Candidates::default();
    collect(
        &adapter,
        "passives.snxlib",
        &query(Some((lib, base_id)), "22µ"),
        &no_other,
        &mut out,
    );
    assert_eq!(out.library.as_deref(), Some("passives.snxlib"));
    let alternates: Vec<&str> = out.alternates.iter().map(|c| c.mpn.as_str()).collect();
    assert_eq!(alternates, ["CL21A226MPQNNNE", "C2012X5R1A226M125AB"]);
    assert!(
        out.alternates
            .iter()
            .all(|c| c.source == CandidateSource::LibraryAlternate && c.value == "22µ")
    );
    assert_eq!(
        out.alternates[1].status,
        CandidateStatus::Alternate(AlternateStatus::Conditional)
    );
    // The part's own row and part number are not search results.
    let found: Vec<&str> = out.found.iter().map(|c| c.mpn.as_str()).collect();
    assert_eq!(found, ["GRM31CR61A226ME19L"]);
    assert_eq!(out.found_total, 1);
    assert_eq!(
        out.found[0].status,
        CandidateStatus::Lifecycle(LifecycleState::Released)
    );
    let all: Vec<&str> = out.all().map(|c| c.mpn.as_str()).collect();
    assert_eq!(all.len(), 3, "alternates first: {all:?}");
    assert_eq!(all[2], "GRM31CR61A226ME19L");
}

#[test]
fn an_empty_query_finds_nothing_and_every_word_must_match() {
    let lib = Uuid::now_v7();
    let cap = row("C-22U", "GRM31CR61A226ME19L", "22µ", lib);
    assert!(!row_matches(&cap, "  "));
    assert!(row_matches(&cap, "22µ murata"));
    assert!(row_matches(&cap, "grm31"));
    assert!(!row_matches(&cap, "22µ tdk"));
}

#[test]
fn stock_is_the_largest_any_listing_reports() {
    let cached = |stock: Option<u32>| CachedPricing {
        sku: "X".into(),
        currency: "EUR".into(),
        stock,
        breaks: Vec::new(),
        fetched_at: chrono::Utc::now(),
    };
    let mut pricing = RowPricing::new();
    pricing.insert(
        "DigiKey".into(),
        [("A".to_string(), cached(Some(23_900)))].into(),
    );
    pricing.insert(
        "Mouser".into(),
        [("B".to_string(), cached(Some(86_400)))].into(),
    );
    pricing.insert("LCSC".into(), [("C".to_string(), cached(None))].into());
    assert_eq!(stock_of(&pricing), Some(86_400));
    assert_eq!(stock_of(&RowPricing::new()), None);
}

#[test]
fn pins_compare_by_number_and_name() {
    let two = vec![
        ("1".to_string(), "~".to_string()),
        ("2".to_string(), "~".to_string()),
    ];
    let mut three = two.clone();
    three.push(("3".to_string(), "GND".to_string()));
    assert_eq!(
        compare_pins(&two, Some(&two)),
        PinCompare::Same { count: 2 }
    );
    assert_eq!(
        compare_pins(&two, Some(&three)),
        PinCompare::Differ { theirs: 3, ours: 2 }
    );
    assert_eq!(compare_pins(&two, None), PinCompare::Unknown);
    assert_eq!(compare_pins(&[], Some(&two)), PinCompare::Unknown);
}

/// The variant gets the part number always, and the value and footprint
/// only where they differ from the placed part's.
#[test]
fn the_variant_gets_what_differs() {
    let candidate = |value: &str, footprint: &str| Candidate {
        source: CandidateSource::Search,
        mpn: " NFM31PC276B0J3L ".to_string(),
        manufacturer: "Murata".to_string(),
        value: value.to_string(),
        footprint: footprint.to_string(),
        pins: None,
        stock: None,
        status: CandidateStatus::Lifecycle(LifecycleState::Released),
        supply: None,
    };
    assert_eq!(
        alternate_values(&candidate("22µ", "cap_0805"), "22µ", "CAP_0805"),
        ("NFM31PC276B0J3L".to_string(), None, None)
    );
    assert_eq!(
        alternate_values(&candidate("27µ 6.3V", "CAP_3T_1206"), "22µ", "CAP_0805"),
        (
            "NFM31PC276B0J3L".to_string(),
            Some("27µ 6.3V".to_string()),
            Some("CAP_3T_1206".to_string())
        )
    );
    assert_eq!(
        alternate_values(&candidate("", ""), "22µ", "CAP_0805"),
        ("NFM31PC276B0J3L".to_string(), None, None),
        "nothing read keeps the part's own"
    );
}

/// #3046: a library alternate carries its stock and lifecycle, the
/// hand-entered value over the fetched one, and the fetched one beside it.
#[test]
fn an_alternate_reads_its_stock_and_lifecycle_with_the_row() {
    use signex_library::{FetchedSupply, ManufacturerLifecycle, RowAlternateSupply, SupplySource};
    let lib = Uuid::now_v7();
    let mut base = row("C-22U", "GRM21BR61A226ME44", "22µ", lib);
    let mut tdk = alternate("TDK", "C2012X5R1A226M125AB", AlternateStatus::Approved);
    tdk.manual_lifecycle = Some(ManufacturerLifecycle::Nrnd);
    base.alternates = vec![
        alternate(
            "Samsung Electro-Mechanics",
            "CL21A226MPQNNNE",
            AlternateStatus::Approved,
        ),
        tdk,
        alternate("Yageo", "CC0805MKX5R6BB226", AlternateStatus::Approved),
    ];
    let at = chrono::Utc::now();
    let fetched = |stock, lifecycle| FetchedSupply {
        stock: Some(stock),
        lifecycle: Some(lifecycle),
        fetched_at: at,
    };
    let mut supply = RowAlternateSupply::new();
    supply.entry("Mouser".into()).or_default().extend([
        (
            "CL21A226MPQNNNE".to_string(),
            fetched(86_400, ManufacturerLifecycle::Active),
        ),
        (
            "C2012X5R1A226M125AB".to_string(),
            fetched(12_050, ManufacturerLifecycle::Active),
        ),
    ]);
    let base_id = base.row_id;
    let adapter = PricedAdapter::new(lib)
        .with_rows(vec![base])
        .with_alternate_supply([(base_id, supply)].into());
    let mut out = Candidates::default();
    collect(
        &adapter,
        "passives.snxlib",
        &query(Some((lib, base_id)), ""),
        &no_other,
        &mut out,
    );
    let [samsung, tdk, yageo] = &out.alternates[..] else {
        panic!("three alternates: {:?}", out.alternates);
    };
    assert_eq!(samsung.stock, Some(86_400));
    let samsung = samsung.supply.as_ref().unwrap();
    assert_eq!(samsung.lifecycle(), Some(ManufacturerLifecycle::Active));
    assert_eq!(
        samsung.stock_source(),
        Some(SupplySource::Fetched {
            distributor: "Mouser".into(),
            fetched_at: at
        })
    );
    // The hand-entered lifecycle wins; the fetched stock still shows.
    assert_eq!(tdk.stock, Some(12_050));
    let tdk = tdk.supply.as_ref().unwrap();
    assert_eq!(tdk.lifecycle(), Some(ManufacturerLifecycle::Nrnd));
    assert_eq!(tdk.lifecycle_source(), Some(SupplySource::Manual));
    // Nothing known: no stock, no lifecycle.
    assert_eq!(yageo.stock, None);
    assert_eq!(yageo.supply.as_ref().unwrap().lifecycle(), None);
}
