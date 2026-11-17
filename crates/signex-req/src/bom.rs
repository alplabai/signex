//! `bom_cost`: what the BOM costs per board when `quantity` boards are
//! built, from cached prices only. Each line buys its fitted quantity times
//! the build quantity (at least the distributor's minimum order) at that
//! quantity's price break. Prices in another currency are not converted, and
//! a line with no price makes the cost unknown: both fail the requirement.

use serde_json::json;
use signex_bom::{BomTable, PriceBook, round_cents};

use crate::Miss;

/// What a `bom_cost` requirement asks.
pub(crate) struct Budget<'a> {
    pub(crate) quantity: u32,
    pub(crate) currency: &'a str,
    pub(crate) max_unit_cost: f64,
}

/// The lines (their designators) a price could not be found for, and what
/// the priced lines cost together for the whole build.
struct Priced {
    total: f64,
    lines: usize,
    missing: Vec<String>,
    other_currency: Vec<(String, String)>,
}

/// `bom_cost`. `bom` is the grouped, unpriced BOM.
pub(crate) fn bom_cost(
    bom: Option<&Result<BomTable, String>>,
    prices: &PriceBook,
    budget: &Budget,
) -> Vec<Miss> {
    let currency = budget.currency.trim();
    if budget.quantity == 0 || currency.is_empty() {
        return vec![Miss::new(
            "`quantity` must be at least 1 and `currency` must be named, so the requirement was not checked",
        )];
    }
    if !budget.max_unit_cost.is_finite() || budget.max_unit_cost < 0.0 {
        return vec![Miss::new(
            "`max_unit_cost` is no amount, so the requirement was not checked",
        )];
    }
    let table = match bom {
        None => return vec![Miss::new("The BOM was not prepared")],
        Some(Err(why)) => {
            let miss = Miss::new(format!("The BOM could not be built: {why}"));
            return vec![miss.with("reason", json!(why))];
        }
        Some(Ok(table)) => table,
    };
    let priced = price(table, prices, budget.quantity, currency);
    let quantity = f64::from(budget.quantity);
    let unit_cost = round_cents(priced.total / quantity);
    let evidence = |miss: Miss| {
        miss.with("quantity", json!(budget.quantity))
            .with("currency", json!(currency))
            .with("max_unit_cost", json!(budget.max_unit_cost))
            .with("lines", json!(priced.lines))
    };
    if !priced.missing.is_empty() || !priced.other_currency.is_empty() {
        let unpriced = priced.missing.len() + priced.other_currency.len();
        let miss = Miss::new(format!(
            "The BOM cost is not known: {unpriced} line(s) have no cached price in {currency}"
        ))
        .with("missing", json!(priced.missing))
        .with(
            "other_currency",
            json!(
                priced
                    .other_currency
                    .iter()
                    .map(|(line, cur)| json!({"line": line, "currency": cur}))
                    .collect::<Vec<_>>()
            ),
        )
        .with("known_unit_cost", json!(unit_cost));
        return vec![evidence(miss)];
    }
    if unit_cost <= budget.max_unit_cost + f64::EPSILON {
        return Vec::new();
    }
    let miss = Miss::new(format!(
        "The BOM costs {unit_cost:.2} {currency} per board for {} boards, over the budget of {:.2} {currency}",
        budget.quantity, budget.max_unit_cost
    ))
    .with("unit_cost", json!(unit_cost))
    .with("total_cost", json!(round_cents(priced.total)));
    vec![evidence(miss)]
}

/// Price every line of `table` that fits a part, for `quantity` boards.
fn price(table: &BomTable, prices: &PriceBook, quantity: u32, currency: &str) -> Priced {
    let mut priced = Priced {
        total: 0.0,
        lines: 0,
        missing: Vec::new(),
        other_currency: Vec::new(),
    };
    for row in table.rows.iter().filter(|row| row.fitted_qty > 0) {
        priced.lines += 1;
        let line = row.references.join(",");
        let Some(quote) = prices.quote_for(row) else {
            priced.missing.push(line);
            continue;
        };
        if !quote.currency.trim().eq_ignore_ascii_case(currency) {
            priced.other_currency.push((line, quote.currency.clone()));
            continue;
        }
        let wanted = row.fitted_qty.saturating_mul(quantity);
        let buy = wanted.max(quote.minimum_order().unwrap_or(1));
        let Some(unit) = quote.unit_price_at(buy) else {
            priced.missing.push(line);
            continue;
        };
        priced.total += round_cents(unit * f64::from(buy));
    }
    priced
}
