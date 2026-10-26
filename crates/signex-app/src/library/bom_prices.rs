//! The BOM's cached prices (#1023), read from the open libraries.
//!
//! Every open library already holds its rows and their cached distributor
//! prices in memory (`OpenLibrary::tables` / `pricing`, refreshed with the
//! library), so building the BOM's [`PriceBook`] reads no file. A placed
//! symbol links to its row by `(library_id, row_id)`; a part placed without
//! that link is matched by MPN instead.

use signex_library::{age_label, current_pricing};
use signex_output::bom::{PriceBook, PriceQuote, library_row_key, mpn_key};

use super::state::LibraryState;

/// The cached quotes of every row of every open library, each row's quotes
/// in its listing order (the first is the one the BOM uses, as the library
/// browser shows it). A price whose SKU no longer matches the row's listing
/// is left out, as everywhere else. For an MPN listed by more than one row,
/// the first row in a fixed order wins: libraries in the order they were
/// opened, tables by name, rows in table order.
pub fn price_book(library: &LibraryState) -> PriceBook {
    let mut book = PriceBook::default();
    for open in &library.open_libraries {
        let mut tables: Vec<_> = open.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        for row in tables.into_iter().flat_map(|(_, rows)| rows) {
            let Some(cached) = open.pricing.get(&row.row_id) else {
                continue;
            };
            let quotes: Vec<PriceQuote> = current_pricing(row, cached)
                .into_iter()
                .map(|(distributor, price)| PriceQuote {
                    distributor: distributor.to_string(),
                    currency: price.currency.clone(),
                    stock: price.stock,
                    breaks: price.breaks.iter().map(|b| (b.qty, b.unit_price)).collect(),
                    age: age_label(price.age(library.clock)),
                })
                .collect();
            if quotes.is_empty() {
                continue;
            }
            let mpn = mpn_key(&row.primary_mpn.mpn);
            if !mpn.is_empty() {
                book.by_mpn.entry(mpn).or_insert_with(|| quotes.clone());
            }
            book.by_row.insert(
                library_row_key(&open.library_id.to_string(), &row.row_id.to_string()),
                quotes,
            );
        }
    }
    book
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::path::PathBuf;

    use chrono::{Duration, Utc};
    use signex_library::distributor::PriceBreak;
    use signex_library::{CachedPricing, ComponentRow, DistributorListing, RowPricing};
    use uuid::Uuid;

    use super::*;
    use crate::library::state::{LibraryDisplaySettings, OpenLibrary};

    fn row(mpn: &str, sku: &str) -> ComponentRow {
        let now = Utc::now();
        ComponentRow {
            row_id: Uuid::now_v7(),
            internal_pn: signex_library::InternalPn::new("R"),
            class: signex_library::ComponentClass::new("resistor"),
            datasheet: Default::default(),
            state: signex_library::LifecycleState::Draft,
            symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: signex_library::ManufacturerPart::draft("Yageo", mpn),
            alternates: Vec::new(),
            supply: vec![DistributorListing::new("DigiKey", sku)],
            parameters: Default::default(),
            plm: Default::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: now,
            updated: now,
            content_hash: [0; 32],
            server_revision: None,
        }
    }

    fn cached(sku: &str, fetched_days_ago: i64) -> RowPricing {
        RowPricing::from([(
            "DigiKey".to_string(),
            BTreeMap::from([(
                sku.to_string(),
                CachedPricing {
                    sku: sku.into(),
                    currency: "USD".into(),
                    stock: Some(42),
                    breaks: vec![PriceBreak {
                        qty: 1,
                        unit_price: 0.1,
                    }],
                    fetched_at: Utc::now() - Duration::days(fetched_days_ago),
                },
            )]),
        )])
    }

    fn library(
        rows: Vec<ComponentRow>,
        pricing: HashMap<Uuid, RowPricing>,
    ) -> (LibraryState, Uuid) {
        let library_id = Uuid::new_v4();
        let mut state = LibraryState {
            clock: Utc::now(),
            ..Default::default()
        };
        state.open_libraries.push(OpenLibrary {
            root: PathBuf::from("/tmp/prices/P.snxlib"),
            display_name: "P".into(),
            library_id,
            tables: HashMap::from([("Resistors".to_string(), rows)]),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing,
            own_edit_stamp: None,
            display: LibraryDisplaySettings::default(),
        });
        (state, library_id)
    }

    #[test]
    fn a_priced_row_is_found_by_its_library_row_and_its_mpn() {
        let priced = row("rc0603fr-0710kl", "DK-1");
        let unpriced = row("GRM188", "DK-2");
        let pricing = HashMap::from([(priced.row_id, cached("DK-1", 3))]);
        let priced_id = priced.row_id;
        let (state, library_id) = library(vec![priced, unpriced], pricing);

        let book = price_book(&state);

        let key = library_row_key(&library_id.to_string(), &priced_id.to_string());
        let quote = &book.by_row[&key][0];
        assert_eq!(quote.distributor, "DigiKey");
        assert_eq!(quote.breaks, vec![(1, 0.1)]);
        assert_eq!(quote.stock, Some(42));
        assert_eq!(quote.age, age_label(Duration::days(3)));
        assert!(book.by_mpn.contains_key("RC0603FR-0710KL"));
        assert_eq!(book.by_row.len(), 1, "an unpriced row adds nothing");
    }

    #[test]
    fn a_price_for_a_replaced_sku_is_not_used() {
        let r = row("RC0603", "DK-NEW");
        let pricing = HashMap::from([(r.row_id, cached("DK-OLD", 1))]);
        let (state, _) = library(vec![r], pricing);
        assert!(price_book(&state).is_empty());
    }
}
