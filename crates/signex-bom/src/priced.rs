//! The priced BOM (#2938): every line of a grouped BOM priced for a build of
//! `quantity` boards from cached prices only, as `bom_cost` prices it, with
//! the line's alternates and, when its part has no price or too little
//! stock, the alternate that would be bought instead. A line with no price
//! in the asked currency is reported and left out of the total, never
//! counted as zero. Shape: `docs/CLI.md`, "BOM".

use std::collections::BTreeMap;

use serde::Serialize;
use signex_types::project::{BomLineAlternates, MPN_LINE_PREFIX, bom_line_key, bom_row_key};

use crate::pricing::{price_of, row_mpn};
use crate::{BomPrice, BomRow, BomTable, PriceBook, round_cents};

/// The key a line's alternates are stored under, as the project stores it
/// (`bom_row_key`, `bom_line_key`): the library row its parts were placed
/// from, else `mpn:` and its MPN; `None` for a line with neither. A row
/// that is not `<uuid>/<uuid>` is its own key, as written.
pub fn line_key(row: &BomRow) -> Option<String> {
    let by_row = (row.library_row.as_deref()).map(|r| bom_row_key(r).unwrap_or_else(|_| r.into()));
    by_row.or_else(|| {
        row_mpn(row).and_then(|mpn| bom_line_key(&format!("{MPN_LINE_PREFIX}{mpn}")).ok())
    })
}

/// What the build asks: how many boards, and the currency the total is in
/// (`None`: each line in its own, totalled per currency).
#[derive(Debug, Clone, Copy)]
pub struct BuildOrder<'a> {
    pub quantity: u32,
    pub currency: Option<&'a str>,
}

/// Why a part is not the one bought.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shortfall {
    /// No cached price.
    NoPrice,
    /// Priced only in another currency than the one asked.
    OtherCurrency,
    /// Fewer in stock than the build needs.
    NoStock,
}

/// One alternate of a line, priced for the build when a price is cached.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PricedAlternate {
    pub row: String,
    pub price: Option<BomPrice>,
}

/// The alternate bought in place of a line's part, and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ServedBy {
    pub row: String,
    pub reason: Shortfall,
    pub price: BomPrice,
}

/// One priced line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PricedLine {
    pub key: Option<String>,
    pub designators: Vec<String>,
    /// Fitted parts per board.
    pub quantity: u32,
    /// Fitted parts for the whole build.
    pub build_quantity: u32,
    pub name: String,
    pub value: String,
    pub footprint: String,
    pub description: String,
    pub library_row: Option<String>,
    pub mpn: Option<String>,
    /// The line's part priced for the build, in whatever currency it is
    /// cached in.
    pub price: Option<BomPrice>,
    /// Why the line is left out of the total; `None` when it is counted.
    pub unpriced: Option<Shortfall>,
    pub alternates: Vec<PricedAlternate>,
    pub served_by: Option<ServedBy>,
}

/// The priced BOM of one build.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PricedBom {
    pub quantity: u32,
    pub currency: Option<String>,
    pub lines: Vec<PricedLine>,
    /// What the counted lines cost for the whole build, per currency.
    pub totals: BTreeMap<String, f64>,
    /// The build's cost and the cost per board, when every line is counted
    /// in one currency; `None` otherwise.
    pub total: Option<f64>,
    pub unit_cost: Option<f64>,
    /// The counted lines' cost in the asked (or only) currency when some
    /// line is not counted, as `bom_cost` reports a known unit cost.
    pub known_total: Option<f64>,
    /// The designators of the lines left out of the total.
    pub unpriced: Vec<String>,
}

fn same_currency(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Why `price` does not serve a build of `needed` in `currency`.
fn shortfall(price: Option<&BomPrice>, currency: Option<&str>, needed: u32) -> Option<Shortfall> {
    let Some(price) = price else {
        return Some(Shortfall::NoPrice);
    };
    if currency.is_some_and(|wanted| !same_currency(&price.currency, wanted)) {
        return Some(Shortfall::OtherCurrency);
    }
    price
        .stock
        .is_some_and(|stock| stock < needed)
        .then_some(Shortfall::NoStock)
}

/// Price `table` (grouped, unpriced) for `order` from `book`, with the
/// alternates `alternates` lists per line.
pub fn priced_bom(
    table: &BomTable,
    book: &PriceBook,
    alternates: &[BomLineAlternates],
    order: BuildOrder,
) -> PricedBom {
    let currency = order.currency.map(str::trim).filter(|c| !c.is_empty());
    let lines: Vec<PricedLine> = table
        .rows
        .iter()
        .map(|row| priced_line(row, book, alternates, order.quantity, currency))
        .collect();
    let mut totals: BTreeMap<String, f64> = BTreeMap::new();
    let mut unpriced = Vec::new();
    for line in lines.iter().filter(|line| line.build_quantity > 0) {
        match (&line.unpriced, &line.price) {
            (None, Some(price)) => {
                let key = currency.unwrap_or(&price.currency).to_string();
                *totals.entry(key).or_insert(0.0) += price.extended_price;
            }
            _ => unpriced.push(line.designators.join(",")),
        }
    }
    totals.values_mut().for_each(|t| *t = round_cents(*t));
    let single = match currency {
        Some(wanted) => Some(totals.get(wanted).copied().unwrap_or(0.0)),
        None if totals.len() <= 1 => Some(totals.values().next().copied().unwrap_or(0.0)),
        None => None,
    };
    let complete = unpriced.is_empty();
    let boards = f64::from(order.quantity.max(1));
    PricedBom {
        quantity: order.quantity,
        currency: currency.map(str::to_string),
        lines,
        total: single.filter(|_| complete),
        unit_cost: single.filter(|_| complete).map(|t| round_cents(t / boards)),
        known_total: single.filter(|_| !complete),
        totals,
        unpriced,
    }
}

fn priced_line(
    row: &BomRow,
    book: &PriceBook,
    alternates: &[BomLineAlternates],
    quantity: u32,
    currency: Option<&str>,
) -> PricedLine {
    let build_quantity = row.fitted_qty.saturating_mul(quantity);
    let key = line_key(row);
    let price = book
        .quote_for(row)
        .and_then(|quote| price_of(quote, build_quantity));
    let priced_alternates: Vec<PricedAlternate> = listed(key.as_deref(), alternates)
        .iter()
        .map(|alt| PricedAlternate {
            row: alt.clone(),
            price: book
                .by_row
                .get(alt)
                .and_then(|quotes| quotes.first())
                .and_then(|quote| price_of(quote, build_quantity)),
        })
        .collect();
    // Stock never leaves a line out of the total: zero needed is in stock.
    let unpriced = shortfall(price.as_ref(), currency, 0);
    let served_by = shortfall(price.as_ref(), currency, build_quantity)
        .filter(|_| build_quantity > 0)
        .and_then(|reason| {
            priced_alternates.iter().find_map(|alt| {
                let price = alt.price.as_ref()?;
                shortfall(Some(price), currency, build_quantity)
                    .is_none()
                    .then(|| ServedBy {
                        row: alt.row.clone(),
                        reason,
                        price: price.clone(),
                    })
            })
        });
    PricedLine {
        key,
        designators: row.references.clone(),
        quantity: row.fitted_qty,
        build_quantity,
        name: row.name.clone(),
        value: row.value.clone(),
        footprint: row.footprint.clone(),
        description: row.description.clone(),
        library_row: row.library_row.clone(),
        mpn: row_mpn(row).map(str::to_string),
        price,
        unpriced,
        alternates: priced_alternates,
        served_by,
    }
}

/// The alternates `alternates` lists for the line keyed `key`.
fn listed<'a>(key: Option<&str>, alternates: &'a [BomLineAlternates]) -> &'a [String] {
    key.and_then(|key| alternates.iter().find(|entry| entry.line == key))
        .map_or(&[][..], |entry| entry.rows.as_slice())
}

/// Give each row of `table` its line's alternates, for the BOM export's
/// Alternates column.
pub fn apply_alternates(table: &mut BomTable, alternates: &[BomLineAlternates]) {
    for row in &mut table.rows {
        row.alternates = listed(line_key(row).as_deref(), alternates).to_vec();
    }
}

#[cfg(test)]
#[path = "priced_tests.rs"]
mod tests;
