//! Nested sort of the Components panel table.

use std::cmp::Ordering;

use signex_types::quantity::{compare_for_sort, compare_ranges};

use super::cell::CellValue;
use crate::templates::{ColumnKey, SortKey};

/// Order of two cells in a column sorted `descending` or not. "Not set"
/// stays last in both directions; measurements order by unit family and
/// base value, a min-to-max value by its `sort_by` end then the other end,
/// text by `text_order` (the panel's display collation). Cells of
/// different kinds (a column mixing text and measurements) order
/// measurements, then numbers, then booleans, then text.
pub fn compare_cells(
    a: &CellValue,
    b: &CellValue,
    descending: bool,
    text_order: &dyn Fn(&str, &str) -> Ordering,
) -> Ordering {
    match (a.is_missing(), b.is_missing()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        (false, false) => {}
    }
    let ascending = match (a, b) {
        (CellValue::Quantity { value: x, .. }, CellValue::Quantity { value: y, .. }) => {
            compare_for_sort(x, y)
        }
        (CellValue::Range { range: x, sort_by }, CellValue::Range { range: y, .. }) => {
            compare_ranges(x, y, *sort_by)
        }
        (CellValue::Number(x), CellValue::Number(y)) => x.total_cmp(y),
        (CellValue::Bool(x), CellValue::Bool(y)) => x.cmp(y),
        (CellValue::Text(x), CellValue::Text(y)) => text_order(x, y),
        _ => kind_rank(a).cmp(&kind_rank(b)),
    };
    if descending {
        ascending.reverse()
    } else {
        ascending
    }
}

fn kind_rank(cell: &CellValue) -> u8 {
    match cell {
        CellValue::Quantity { .. } => 0,
        CellValue::Range { .. } => 1,
        CellValue::Number(_) => 2,
        CellValue::Bool(_) => 3,
        CellValue::Text(_) => 4,
        CellValue::Missing => 5,
    }
}

/// The cell of a row or column the cache does not hold.
static MISSING: CellValue = CellValue::Missing;

/// Cells of every row, read once per column the sort or grouping needs.
pub(super) struct CellCache {
    columns: Vec<(ColumnKey, Vec<CellValue>)>,
}

impl CellCache {
    pub(super) fn new<'k>(
        count: usize,
        columns: impl IntoIterator<Item = &'k ColumnKey>,
        cell: &dyn Fn(usize, &ColumnKey) -> CellValue,
    ) -> CellCache {
        let mut cache = CellCache {
            columns: Vec::new(),
        };
        for column in columns {
            if cache.column(column).is_none() {
                let cells = (0..count).map(|i| cell(i, column)).collect();
                cache.columns.push((column.clone(), cells));
            }
        }
        cache
    }

    pub(super) fn column(&self, column: &ColumnKey) -> Option<&[CellValue]> {
        self.columns
            .iter()
            .find(|(c, _)| c == column)
            .map(|(_, cells)| cells.as_slice())
    }

    pub(super) fn get(&self, column: &ColumnKey, row: usize) -> &CellValue {
        self.column(column)
            .and_then(|cells| cells.get(row))
            .unwrap_or(&MISSING)
    }
}

/// Sort `rows` (indices into the cache) by `keys`, outermost first; ties
/// fall to the part number, then to the given order.
pub(super) fn sort_indices(
    rows: &mut [usize],
    keys: &[SortKey],
    cache: &CellCache,
    text_order: &dyn Fn(&str, &str) -> Ordering,
) {
    rows.sort_by(|&a, &b| {
        keys.iter()
            .map(|k| {
                compare_cells(
                    cache.get(&k.column, a),
                    cache.get(&k.column, b),
                    k.descending,
                    text_order,
                )
            })
            .find(|o| o.is_ne())
            .unwrap_or_else(|| {
                let pn = &ColumnKey::PartNumber;
                compare_cells(cache.get(pn, a), cache.get(pn, b), false, text_order)
            })
    });
}

/// Row indices `0..count` in the order `keys` sort them (stable; ties go
/// to the part number). `cell(i, column)` is row `i`'s value in `column`,
/// read once per row and column.
pub fn sorted_rows(
    count: usize,
    keys: &[SortKey],
    cell: impl Fn(usize, &ColumnKey) -> CellValue,
    text_order: impl Fn(&str, &str) -> Ordering,
) -> Vec<usize> {
    let columns = keys
        .iter()
        .map(|k| &k.column)
        .chain([&ColumnKey::PartNumber]);
    let cache = CellCache::new(count, columns, &cell);
    let mut rows: Vec<usize> = (0..count).collect();
    sort_indices(&mut rows, keys, &cache, &text_order);
    rows
}
