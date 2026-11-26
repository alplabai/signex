//! The parametric part search of `search_parts` (#1297): the rows of every
//! open library that match the Components panel's query (the same words,
//! read the same way), narrowed by value, package, tolerance, rating and
//! stock, each with its lifecycle state and its cached supplier offers.
//! Nothing is fetched: the offers are the cached prices the BOM prices from
//! (`bom_prices::row_quotes`). Shape: `docs/CLI.md`, "BOM and part search".

use serde_json::{Map, Value, json};
use signex_i18n::I18n;
use signex_library::{ComponentRow, ParamValue};
use signex_output::bom::PriceQuote;

use super::bom_prices::row_quotes;
use super::state::LibraryState;
use crate::panels::components_panel::query::{PanelQuery, SearchKeys, ValueWord};

/// The parameters a `rating` is read from, in order: the generic rating,
/// a voltage, a power and a current rating.
const RATING_PARAMETERS: [&str; 4] = ["rating", "voltage_max", "power", "current_max"];

/// A `value` filter: a number with a prefix or unit, else text.
#[derive(Debug, Clone)]
pub(crate) enum ValueFilter {
    Value(ValueWord),
    Text(String),
}

/// What a search asks.
#[derive(Debug, Clone)]
pub(crate) struct PartSearch {
    pub(crate) query: PanelQuery,
    pub(crate) value: Option<ValueFilter>,
    pub(crate) package: Option<String>,
    /// At most this tolerance.
    pub(crate) tolerance: Option<ValueWord>,
    /// At least this rating.
    pub(crate) rating: Option<ValueWord>,
    pub(crate) min_stock: Option<u32>,
    /// The quantity a unit price is read at.
    pub(crate) quantity: u32,
    pub(crate) limit: usize,
}

fn quantity_of(row: &ComponentRow, name: &str) -> Option<signex_types::quantity::Quantity> {
    row.parameters.get(name).and_then(ParamValue::as_quantity)
}

impl PartSearch {
    /// Whether `row` matches every word and every parameter asked.
    fn matches(&self, i18n: &I18n, row: &ComponentRow) -> bool {
        let keys = SearchKeys::of(i18n, row);
        if !self.query.words().iter().all(|word| keys.matches(word)) {
            return false;
        }
        let value_ok = match &self.value {
            None => true,
            Some(ValueFilter::Value(word)) => {
                quantity_of(row, "value").is_some_and(|q| word.equals(&q))
            }
            Some(ValueFilter::Text(text)) => row
                .parameters
                .get("value")
                .is_some_and(|v| v.display().trim().eq_ignore_ascii_case(text)),
        };
        let package_ok = self.package.as_deref().is_none_or(|package| {
            row.parameters
                .get("package")
                .is_some_and(|v| v.display().trim().eq_ignore_ascii_case(package))
        });
        let tolerance_ok = self.tolerance.as_ref().is_none_or(|most| {
            quantity_of(row, "tolerance")
                .is_some_and(|q| most.same_unit(&q) && q.base().abs() <= most.base.abs())
        });
        let rating_ok = self.rating.as_ref().is_none_or(|least| {
            RATING_PARAMETERS.iter().any(|name| {
                quantity_of(row, name)
                    .is_some_and(|q| least.same_unit(&q) && q.base() >= least.base)
            })
        });
        value_ok && package_ok && tolerance_ok && rating_ok
    }
}

/// One cached offer as the answer gives it, its unit price read at
/// `quantity` (the minimum order's below it).
fn offer_json(quote: &PriceQuote, quantity: u32) -> Value {
    json!({
        "distributor": quote.distributor,
        "currency": quote.currency,
        "stock": quote.stock,
        "breaks": quote.breaks,
        "unit_price": quote.unit_price_at(quantity),
        "age": quote.age,
    })
}

/// Every open library's rows that match `search`: libraries in the order
/// they were opened, tables by name, rows in table order; at most
/// `search.limit` are listed and `count` says how many matched.
pub(crate) fn search_parts(library: &LibraryState, i18n: &I18n, search: &PartSearch) -> Value {
    let mut matches = Vec::new();
    let mut count = 0usize;
    for open in &library.open_libraries {
        let mut tables: Vec<_> = open.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        for row in tables.into_iter().flat_map(|(_, rows)| rows) {
            if !search.matches(i18n, row) {
                continue;
            }
            let quotes = row_quotes(library, open, row);
            let in_stock = |least: u32| quotes.iter().any(|q| q.stock.is_some_and(|s| s >= least));
            if search.min_stock.is_some_and(|least| !in_stock(least)) {
                continue;
            }
            count += 1;
            if matches.len() >= search.limit {
                continue;
            }
            let parameters: Map<String, Value> = row
                .parameters
                .iter()
                .map(|(name, value)| (name.clone(), json!(value.display())))
                .collect();
            matches.push(json!({
                "library": open.library_id,
                "library_name": open.display_name,
                "part": row.row_id,
                "internal_pn": row.internal_pn.as_str(),
                "mpn": row.primary_mpn.mpn,
                "manufacturer": row.primary_mpn.manufacturer,
                "description": row.description,
                "lifecycle": row.state.as_str(),
                "parameters": parameters,
                "stock": quotes.iter().filter_map(|q| q.stock).max(),
                "offers": quotes.iter().map(|q| offer_json(q, search.quantity)).collect::<Vec<_>>(),
            }));
        }
    }
    json!({
        "source": "cache",
        "count": count,
        "truncated": count > matches.len(),
        "matches": matches,
    })
}
