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
/// numeric (XLSX): the quantity, the prices and the stock.
pub fn cell_number(row: &BomRow, column: &BomColumn) -> Option<f64> {
    let price = row.price.as_ref();
    match column {
        BomColumn::Qty => Some(f64::from(row.qty)),
        BomColumn::UnitPrice => price.map(|p| p.unit_price),
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

/// An amount: two decimals.
pub fn money(value: f64) -> String {
    format!("{value:.2}")
}

/// One line under the rows for each currency the BOM is priced in, when the
/// extended price is among `columns`: "Total" in the first column, the sum in
/// the extended-price column and the code in the currency column. Each line
/// is `(text cells, numeric cells)`, one entry per column. Empty when no row
/// is priced or the extended price is not shown.
pub fn total_lines(table: &BomTable, columns: &[BomColumn]) -> Vec<Vec<(String, Option<f64>)>> {
    if !columns.contains(&BomColumn::ExtendedPrice) {
        return Vec::new();
    }
    signex_bom::price_totals(table)
        .into_iter()
        .map(|(currency, total)| {
            columns
                .iter()
                .enumerate()
                .map(|(index, column)| match column {
                    BomColumn::ExtendedPrice => (money(total), Some(total)),
                    BomColumn::Currency => (currency.clone(), None),
                    _ if index == 0 => ("Total".to_string(), None),
                    _ => (String::new(), None),
                })
                .collect()
        })
        .collect()
}
