//! What each BOM column shows for a row, shared by every writer and the
//! preview, and the per-currency total lines under a priced BOM (#1023).

use signex_bom::{BomRow, BomTable};

use super::BomColumn;

/// The text `column` shows for `row`. Price columns are empty for a row
/// without a cached price.
pub fn cell_text(row: &BomRow, column: &BomColumn) -> String {
    let price = row.price.as_ref();
    match column {
        BomColumn::Name => row.name.clone(),
        BomColumn::Designator | BomColumn::Reference => row.references.join(", "),
        BomColumn::LibRef => row.lib_ref.clone(),
        BomColumn::Qty => row.qty.to_string(),
        BomColumn::Value => row.value.clone(),
        BomColumn::Footprint => row.footprint.clone(),
        BomColumn::Description => row.description.clone(),
        BomColumn::Custom(name) => row.custom.get(name).cloned().unwrap_or_default(),
        BomColumn::UnitPrice => price.map(|p| unit_money(p.unit_price)).unwrap_or_default(),
        BomColumn::BuyQty => price.map(|p| p.buy_qty.to_string()).unwrap_or_default(),
        BomColumn::ExtendedPrice => price.map(|p| money(p.extended_price)).unwrap_or_default(),
        BomColumn::Currency => price.map(|p| p.currency.clone()).unwrap_or_default(),
        BomColumn::Stock => price
            .and_then(|p| p.stock)
            .map(|stock| stock.to_string())
            .unwrap_or_default(),
        BomColumn::Distributor => price.map(|p| p.distributor.clone()).unwrap_or_default(),
        BomColumn::PriceAge => price.map(|p| p.age.clone()).unwrap_or_default(),
    }
}

/// The number `column` holds for `row`, for writers that keep numbers
/// numeric (XLSX): the quantities, the prices and the stock. An extended
/// price is already rounded to cents.
pub fn cell_number(row: &BomRow, column: &BomColumn) -> Option<f64> {
    let price = row.price.as_ref();
    match column {
        BomColumn::Qty => Some(f64::from(row.qty)),
        BomColumn::UnitPrice => price.map(|p| p.unit_price),
        BomColumn::BuyQty => price.map(|p| f64::from(p.buy_qty)),
        BomColumn::ExtendedPrice => price.map(|p| p.extended_price),
        BomColumn::Stock => price.and_then(|p| p.stock).map(f64::from),
        _ => None,
    }
}

/// A unit price: at least two decimals, up to four for sub-unit prices.
pub fn unit_money(value: f64) -> String {
    let text = format!("{value:.4}");
    let trimmed = text.trim_end_matches('0');
    let decimals = trimmed.split('.').nth(1).map_or(0, str::len);
    if decimals < 2 {
        format!("{value:.2}")
    } else {
        trimmed.to_string()
    }
}

/// An amount: two decimals, halves rounded away from zero like the
/// extended prices themselves.
pub fn money(value: f64) -> String {
    format!("{:.2}", signex_bom::round_cents(value))
}

/// One line under the rows for each currency the BOM is priced in, when the
/// extended price is among `columns`. The line always names its currency, so
/// totals in different currencies cannot be mistaken for one another:
/// - "Total" goes in the first column that holds neither the sum nor the
///   currency, and becomes "Total USD" when there is no currency column;
/// - the sum goes in the extended-price column, and the code in the currency
///   column;
/// - with no column free for the label, the label joins the currency cell
///   ("Total USD"), or the sum cell ("Total 1.00 USD") when there is no
///   currency column either.
///
/// Each line is `(text, number)` per column. Empty when no row is priced or
/// the extended price is not shown.
pub fn total_lines(table: &BomTable, columns: &[BomColumn]) -> Vec<Vec<(String, Option<f64>)>> {
    if !columns.contains(&BomColumn::ExtendedPrice) {
        return Vec::new();
    }
    let label_at = columns
        .iter()
        .position(|c| !matches!(c, BomColumn::ExtendedPrice | BomColumn::Currency));
    let has_currency = columns.contains(&BomColumn::Currency);
    signex_bom::price_totals(table)
        .into_iter()
        .map(|(currency, total)| {
            columns
                .iter()
                .enumerate()
                .map(|(index, column)| match column {
                    BomColumn::ExtendedPrice if label_at.is_none() && !has_currency => {
                        (format!("Total {} {currency}", money(total)), None)
                    }
                    BomColumn::ExtendedPrice => (money(total), Some(total)),
                    BomColumn::Currency if label_at.is_none() => {
                        (format!("Total {currency}"), None)
                    }
                    BomColumn::Currency => (currency.clone(), None),
                    _ if Some(index) == label_at && has_currency => ("Total".to_string(), None),
                    _ if Some(index) == label_at => (format!("Total {currency}"), None),
                    _ => (String::new(), None),
                })
                .collect()
        })
        .collect()
}
