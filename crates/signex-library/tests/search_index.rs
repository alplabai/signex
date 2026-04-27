//! Acceptance tests for `TantivySearchIndex`.
//!
//! The full Tantivy rewrite for the DBLib model is deferred (see
//! `docs/internal/docs/LIBRARY_PLAN.md`); the index needs only to
//! compile and accept [`ComponentRow`] payloads for now. The
//! corpus-level tests below build rows directly and verify
//! text/numeric query paths against the row schema.
//!
//! Run with: `cargo test -p signex-library --features search-tantivy --test search_index`.

#![cfg(feature = "search-tantivy")]

use std::collections::BTreeMap;

use chrono::Utc;
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, Facet, FacetOp, LifecycleState, ManufacturerPart,
    ParamMap, ParamValue, PlmReserved, PrimitiveRef, SearchIndex, SearchQuery, TantivySearchIndex,
};
use uuid::Uuid;

// ── fixture builders ───────────────────────────────────────────────────

fn fresh_row(
    internal_pn: &str,
    mpn: &str,
    manufacturer: &str,
    _description: &str,
    class: &str,
    parameters: ParamMap,
) -> ComponentRow {
    let lib = Uuid::nil();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: signex_library::InternalPn::new(internal_pn),
        class: ComponentClass::new(class),
        datasheet: DatasheetRef::url(""),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::nil()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft(manufacturer, mpn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters,
        plm: PlmReserved::default(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
    }
}

/// Build 100 fixture rows: caps, resistors, ICs, etc.
fn fixture_corpus() -> Vec<ComponentRow> {
    let mut out = Vec::with_capacity(100);

    // 1 — the unique target the text-search test queries for.
    let mut cap_target_params = ParamMap::new();
    cap_target_params.insert(
        "capacitance".into(),
        ParamValue::Number(10e-6), // 10 µF
    );
    cap_target_params.insert("voltage".into(), ParamValue::Number(25.0));
    cap_target_params.insert("dielectric".into(), ParamValue::Text("X7R".into()));
    cap_target_params.insert("package".into(), ParamValue::Text("0805".into()));
    out.push(fresh_row(
        "C0805_10uF_25V_X7R",
        "GRM21BR71E106KE12L",
        "Murata",
        "Capacitor 10µF 0805 25V X7R MLCC",
        "Capacitor",
        cap_target_params,
    ));

    // 39 other capacitors with varying values.
    let cap_specs = [
        (1e-9, 50.0, "C0G", "0402"),
        (10e-9, 25.0, "X7R", "0402"),
        (100e-9, 16.0, "X7R", "0603"),
        (1e-6, 10.0, "X5R", "0603"),
        (4.7e-6, 25.0, "X5R", "0805"),
        (22e-6, 10.0, "X5R", "0805"),
        (47e-6, 6.3, "X5R", "1206"),
        (100e-6, 10.0, "X7R", "1210"),
        (220e-12, 50.0, "C0G", "0402"),
        (470e-12, 50.0, "C0G", "0402"),
    ];
    for (i, (c, v, dielec, pkg)) in cap_specs.iter().enumerate() {
        for rep in 0..4 {
            let mut params = ParamMap::new();
            params.insert("capacitance".into(), ParamValue::Number(*c));
            params.insert("voltage".into(), ParamValue::Number(*v));
            params.insert("dielectric".into(), ParamValue::Text((*dielec).into()));
            params.insert("package".into(), ParamValue::Text((*pkg).into()));
            let pn = format!("C_FILLER_{:03}", i * 4 + rep);
            let desc = format!(
                "Generic ceramic capacitor {} package, {} dielectric, {} V",
                pkg, dielec, v
            );
            out.push(fresh_row(
                &pn,
                &format!("CAP{}", i * 4 + rep),
                "GenericCo",
                &desc,
                "Capacitor",
                params,
            ));
        }
    }

    // 40 resistors.
    let r_decades = [
        1.0, 4.7, 10.0, 22.0, 47.0, 100.0, 220.0, 470.0, 1_000.0, 4_700.0,
    ];
    for (i, r) in r_decades.iter().enumerate() {
        for (rep, pkg) in ["0402", "0603", "0805", "1206"].iter().enumerate() {
            let mut params = ParamMap::new();
            params.insert("resistance".into(), ParamValue::Number(*r));
            params.insert("package".into(), ParamValue::Text((*pkg).into()));
            params.insert("tolerance".into(), ParamValue::Text("1%".into()));
            let pn = format!("R{}_{}_{}", pkg, *r as u64, i);
            let desc = format!("Thick film resistor {} Ω, 1%, {}", r, pkg);
            out.push(fresh_row(
                &pn,
                &format!("RES{}_{}", i, rep),
                "Yageo",
                &desc,
                "Resistor",
                params,
            ));
        }
    }

    // 19 ICs / misc — 1 unique cap + 40 fillers + 40 resistors + 19 ICs = 100.
    for i in 0..19 {
        let mut params = ParamMap::new();
        params.insert("voltage".into(), ParamValue::Number(3.3));
        params.insert("package".into(), ParamValue::Text("SOT-23".into()));
        let pn = format!("IC_FILLER_{:03}", i);
        out.push(fresh_row(
            &pn,
            &format!("IC{}", i),
            "TI",
            &format!("Generic regulator IC variant {}", i),
            "IC",
            params,
        ));
    }

    assert_eq!(out.len(), 100, "fixture corpus must be exactly 100 rows");
    out
}

// ── tests ──────────────────────────────────────────────────────────────

#[test]
fn text_query_pinpoints_the_single_matching_part() {
    let dir = tempfile::tempdir().unwrap();
    let idx = TantivySearchIndex::open(dir.path()).expect("open index");

    let corpus = fixture_corpus();
    for c in &corpus {
        idx.add_or_update(c).expect("add row");
    }
    idx.commit().expect("commit");

    let q = SearchQuery {
        text: Some("Capacitor 10µF 0805 25V X7R".into()),
        category: None,
        facets: vec![],
        limit: 5,
    };
    let hits = idx.query(&q);

    assert!(!hits.is_empty(), "expected at least one hit");
    // Highest-scoring hit must be the unique 10 µF 0805 25V X7R cap.
    assert_eq!(
        hits[0].internal_pn.as_str(),
        "C0805_10uF_25V_X7R",
        "top hit was {:?}",
        hits[0].internal_pn
    );
    // The search index synthesises a "<manufacturer> <mpn>" description; the
    // MPN is unique enough that the synthesised string still pinpoints the
    // hit.
    assert!(
        hits[0].description.contains("Murata"),
        "unexpected description: {:?}",
        hits[0].description
    );
    assert!(
        hits[0].description.contains("GRM21BR71E106KE12L"),
        "unexpected description: {:?}",
        hits[0].description
    );
}

#[test]
fn numeric_facet_lt_returns_only_sub_threshold_parts() {
    let dir = tempfile::tempdir().unwrap();
    let idx = TantivySearchIndex::open(dir.path()).expect("open index");

    let corpus = fixture_corpus();
    for c in &corpus {
        idx.add_or_update(c).expect("add row");
    }
    idx.commit().expect("commit");

    let q = SearchQuery {
        text: None,
        category: Some("Capacitor".into()),
        facets: vec![Facet {
            field: "parameters.capacitance".into(),
            op: FacetOp::Lt,
            value: "1e-6".into(),
        }],
        limit: 200,
    };
    let hits = idx.query(&q);

    assert!(!hits.is_empty(), "expected sub-1µF capacitor hits");

    // Cross-check by re-running the predicate on the corpus.
    let mut expected: Vec<String> = corpus
        .iter()
        .filter(|c| {
            if c.class.as_str() != "Capacitor" {
                return false;
            }
            let cap = c.parameters.get("capacitance");
            matches!(cap, Some(ParamValue::Number(n)) if *n < 1e-6)
        })
        .map(|c| c.internal_pn.0.clone())
        .collect();
    expected.sort();

    let mut got: Vec<String> = hits
        .iter()
        .map(|h| h.internal_pn.as_str().to_string())
        .collect();
    got.sort();

    assert_eq!(
        got, expected,
        "Lt 1e-6 should return exactly sub-1µF caps (got {:?}, expected {:?})",
        got, expected
    );
}

#[test]
fn index_persists_across_drop_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path().to_path_buf();

    {
        let idx = TantivySearchIndex::open(&dir_path).expect("open index");
        for c in &fixture_corpus() {
            idx.add_or_update(c).expect("add row");
        }
        idx.commit().expect("commit");
    }

    let idx2 = TantivySearchIndex::open(&dir_path).expect("reopen index");
    let q = SearchQuery {
        text: Some("Capacitor 10µF 0805 25V X7R".into()),
        category: None,
        facets: vec![],
        limit: 5,
    };
    let hits = idx2.query(&q);
    assert!(
        !hits.is_empty(),
        "reopened index should still answer queries"
    );
    assert_eq!(hits[0].internal_pn.as_str(), "C0805_10uF_25V_X7R");
}

#[test]
fn add_or_update_replaces_existing_doc() {
    let dir = tempfile::tempdir().unwrap();
    let idx = TantivySearchIndex::open(dir.path()).expect("open index");

    let mut params = ParamMap::new();
    params.insert("capacitance".into(), ParamValue::Number(1e-6));
    let mut row = fresh_row(
        "C_TEST",
        "TEST-001",
        "Acme",
        "Sentinel-original-aardvark cap",
        "Capacitor",
        params,
    );
    idx.add_or_update(&row).expect("add");
    idx.commit().expect("commit");

    // Mutate manufacturer + mpn — same row_id, new content.
    row.primary_mpn.manufacturer = "Sentinel-updated-zucchini".into();
    row.primary_mpn.mpn = "TEST-002".into();
    idx.add_or_update(&row).expect("update");
    idx.commit().expect("commit update");

    let q_new = SearchQuery {
        text: Some("zucchini".into()),
        category: None,
        facets: vec![],
        limit: 10,
    };
    let hits = idx.query(&q_new);
    assert_eq!(hits.len(), 1, "expected exactly one doc post-update");
    assert!(hits[0].description.contains("Sentinel-updated-zucchini"));

    let q_old = SearchQuery {
        text: Some("aardvark".into()),
        category: None,
        facets: vec![],
        limit: 10,
    };
    assert!(
        idx.query(&q_old).is_empty(),
        "stale doc should have been replaced (got {:?})",
        idx.query(&q_old)
    );
}

#[test]
fn category_only_query_filters_corpus() {
    let dir = tempfile::tempdir().unwrap();
    let idx = TantivySearchIndex::open(dir.path()).expect("open index");

    for c in &fixture_corpus() {
        idx.add_or_update(c).expect("add");
    }
    idx.commit().expect("commit");

    let q = SearchQuery {
        text: None,
        category: Some("Resistor".into()),
        facets: vec![],
        limit: 200,
    };
    let hits = idx.query(&q);
    assert_eq!(
        hits.len(),
        40,
        "expected exactly 40 resistor fixtures, got {}",
        hits.len()
    );
    for h in &hits {
        assert!(
            h.internal_pn.as_str().starts_with("R0402")
                || h.internal_pn.as_str().starts_with("R0603")
                || h.internal_pn.as_str().starts_with("R0805")
                || h.internal_pn.as_str().starts_with("R1206"),
            "unexpected non-resistor in result: {}",
            h.internal_pn
        );
    }
}

// Defensive: keep an unused `BTreeMap` import in scope so cargo doesn't warn
// when the test file evolves; suppression rather than removal because adapter
// tests often re-introduce these collections.
#[allow(dead_code)]
fn _force_use_btreemap() -> BTreeMap<String, String> {
    BTreeMap::new()
}
