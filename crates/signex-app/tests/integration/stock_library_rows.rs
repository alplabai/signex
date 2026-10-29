//! #1653 — the bundled stock library ships placeable component rows.
//! Without them the Components panel showed `Stock (0)` and a new user
//! could place nothing. These tests open the committed `Stock.snxlib`
//! through the real adapter, exactly as the app mounts it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::{ComponentRow, LifecycleState, hash_row_content};
use signex_sketch::entity::EntityKind;

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn open_stock() -> LocalGitAdapter {
    LocalGitAdapter::open(stock_file()).expect("Stock.snxlib opens")
}

/// Every stock row. Asserts the table is not empty, so a check over the
/// rows can never pass vacuously against a library with no rows.
fn stock_rows(adapter: &LocalGitAdapter) -> Vec<ComponentRow> {
    let rows: Vec<ComponentRow> = adapter
        .iter_rows()
        .expect("read stock rows")
        .into_iter()
        .map(|(_, row)| row)
        .collect();
    assert!(!rows.is_empty(), "Stock.snxlib ships no component rows");
    rows
}

#[test]
fn stock_library_lists_one_placeable_row_per_non_power_symbol() {
    let rows = stock_rows(&open_stock());
    assert_eq!(rows.len(), 34, "34 non-power stock symbols, one row each");
    for pn in [
        "STOCK-R",
        "STOCK-C",
        "STOCK-L",
        "STOCK-D",
        "STOCK-LED",
        "STOCK-74HC00",
    ] {
        assert!(
            rows.iter().any(|r| r.internal_pn.as_str() == pn),
            "{pn} is missing"
        );
    }
    let pns: BTreeSet<_> = rows.iter().map(|r| r.internal_pn.as_str()).collect();
    assert_eq!(pns.len(), rows.len(), "internal_pn is unique");
    for row in &rows {
        assert_eq!(row.state, LifecycleState::Released, "{}", row.internal_pn);
        assert!(
            !row.description.is_empty(),
            "{} has no description",
            row.internal_pn
        );
        assert!(
            !row.is_symbol_unbound(),
            "{} has no symbol",
            row.internal_pn
        );
    }
}

#[test]
fn every_row_ref_resolves_to_a_bundled_file_and_pins_match_pads() {
    let adapter = open_stock();
    let library_id = adapter.library_id();
    for row in stock_rows(&adapter) {
        let pn = row.internal_pn.to_string();
        assert_eq!(row.symbol_ref.library_id, library_id, "{pn} symbol library");
        let symbol = adapter
            .get_symbol(row.symbol_ref.uuid)
            .unwrap_or_else(|e| panic!("{pn}: symbol does not resolve: {e}"));
        let Some(fp_ref) = row.footprint_ref else {
            assert!(row.footprint_version.is_empty(), "{pn}");
            continue;
        };
        assert_eq!(fp_ref.library_id, library_id, "{pn} footprint library");
        let footprint = adapter
            .get_footprint(fp_ref.uuid)
            .unwrap_or_else(|e| panic!("{pn}: footprint does not resolve: {e}"));
        let pads: BTreeSet<String> = footprint
            .sketch
            .iter()
            .flat_map(|s| &s.entities)
            .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
            .filter_map(|e| e.pad.as_ref().map(|p| p.number.clone()))
            .collect();
        let pins: BTreeSet<String> = symbol.pins.iter().map(|p| p.number.clone()).collect();
        assert_eq!(pins, pads, "{pn}: pin numbers must equal pad numbers");
        assert!(
            row.pin_map_overrides.is_empty(),
            "{pn}: 1:1 map needs no overrides"
        );
    }
}

#[test]
fn every_row_content_hash_verifies() {
    for row in stock_rows(&open_stock()) {
        assert_eq!(
            hash_row_content(&row).expect("hash"),
            row.content_hash,
            "{} content_hash is stale",
            row.internal_pn
        );
    }
}
