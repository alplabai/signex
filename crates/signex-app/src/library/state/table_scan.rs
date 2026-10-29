//! The per-table read loop shared by `OpenLibrary::reload_tables` and
//! `LibrarySnapshot::read`: visit every table the adapter lists, load its
//! cached prices and its rows, and tolerate a failed read of either.

use super::*;

/// Everything one pass over an adapter's tables read.
pub(super) struct TableScan {
    pub tables: HashMap<String, Vec<ComponentRow>>,
    pub summaries: Vec<ComponentSummary>,
    pub pricing: HashMap<Uuid, signex_library::RowPricing>,
}

/// Read every table in `names`. A failed price read is handed to
/// `on_pricing_err` and the table's rows are still read; a failed row read
/// is handed to `on_read_err` and the table is kept empty. Each caller owns
/// its logging sink and its summary shape (`summarize`).
pub(super) fn scan_tables(
    adapter: &dyn LibraryAdapter,
    names: Vec<String>,
    summarize: fn(&ComponentRow) -> ComponentSummary,
    mut on_pricing_err: impl FnMut(&str, &LibraryError),
    mut on_read_err: impl FnMut(&str, &LibraryError),
) -> TableScan {
    let mut scan = TableScan {
        tables: HashMap::new(),
        summaries: Vec::new(),
        pricing: HashMap::new(),
    };
    for name in names {
        match adapter.table_pricing(&name) {
            Ok(cached) => scan.pricing.extend(cached),
            Err(e) => on_pricing_err(&name, &e),
        }
        match adapter.read_table(&name) {
            Ok(rows) => {
                scan.summaries.extend(rows.iter().map(summarize));
                scan.tables.insert(name, rows);
            }
            Err(e) => {
                on_read_err(&name, &e);
                scan.tables.insert(name, Vec::new());
            }
        }
    }
    scan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_adapters::{FailingListingAdapter, PricedAdapter};

    fn summarize(row: &ComponentRow) -> ComponentSummary {
        super::super::methods::summarize_row(row)
    }

    #[test]
    fn scan_visits_every_table_and_loads_prices() {
        let adapter = PricedAdapter::new(Uuid::now_v7());
        let scan = scan_tables(
            &adapter,
            vec!["R".into(), "C".into()],
            summarize,
            |_, _| panic!("no price error expected"),
            |_, _| panic!("no read error expected"),
        );
        assert_eq!(scan.tables.len(), 2);
        assert!(scan.pricing.contains_key(&adapter.priced_row));
    }

    #[test]
    fn scan_keeps_a_table_empty_when_its_read_fails() {
        let adapter = FailingListingAdapter::new(Uuid::now_v7());
        let mut read_errs = Vec::new();
        let scan = scan_tables(
            &adapter,
            vec!["R".into()],
            summarize,
            |_, _| {},
            |name, _| read_errs.push(name.to_string()),
        );
        assert_eq!(read_errs, ["R"]);
        assert!(scan.tables["R"].is_empty());
    }
}
