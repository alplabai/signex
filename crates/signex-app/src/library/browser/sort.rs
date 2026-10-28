//! Row sorting for the library browser grid.
//!
//! Each row's sort key (the `ParamValue`'s quantity, or the numeric parse
//! and lowercase form of a plain cell) is derived exactly once, then the rows are stably sorted on
//! the cached keys. Deriving keys inside the comparator would repeat
//! that allocation and parsing work O(n log n) times per sort.

use std::cmp::Ordering;

use signex_library::ParamValue;
use signex_types::quantity::{Quantity, compare_for_sort};

/// What a cell is, for ordering. The variants rank in this order.
enum SortKind {
    /// A plain number.
    Number(f64),
    /// A typed measurement, read from the `ParamValue`.
    Measured(Quantity),
    /// Anything else.
    Text,
}

impl SortKind {
    fn rank(&self) -> u8 {
        match self {
            SortKind::Number(_) => 0,
            SortKind::Measured(_) => 1,
            SortKind::Text => 2,
        }
    }
}

/// Precomputed, comparison-ready form of one cell.
///
/// Ordering is a strict total order. Plain numbers (any cell that parses
/// as a non-NaN `f64` after trimming) rank first and compare with
/// `f64::total_cmp`. Typed measurements follow, by `(unit family, base
/// value)`, so a column of mixed families clusters by family and a legacy
/// `{4.7, "kΩ"}` lands beside a canonical `{4700, "ohm"}`. Text, empty
/// cells and a literal `NaN` rank last, in natural order
/// (`R2` before `R10`), case-insensitively. Ranking by kind first is
/// what keeps mixed columns transitive, which `slice::sort_by` requires.
pub(super) struct CellSortKey {
    kind: SortKind,
    lowercase: String,
}

impl CellSortKey {
    /// Key for a cell known only by its string (a column that is not a
    /// parameter, or a price).
    pub(super) fn from_cell(cell: &str) -> Self {
        let kind = match cell.trim().parse::<f64>().ok().filter(|n| !n.is_nan()) {
            Some(n) => SortKind::Number(n),
            None => SortKind::Text,
        };
        Self {
            kind,
            lowercase: cell.to_lowercase(),
        }
    }

    /// Key for a parameter value. A measurement is ordered by its
    /// base-unit quantity, never by its display text.
    pub(super) fn from_param(value: &ParamValue) -> Self {
        match value.as_quantity() {
            Some(q) => Self {
                kind: SortKind::Measured(q),
                lowercase: value.display().to_lowercase(),
            },
            None => match value {
                ParamValue::Number(n) if !n.is_nan() => Self {
                    kind: SortKind::Number(*n),
                    lowercase: value.display().to_lowercase(),
                },
                other => Self::from_cell(&other.display()),
            },
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (&self.kind, &other.kind) {
            (SortKind::Number(a), SortKind::Number(b)) => a.total_cmp(b),
            (SortKind::Measured(a), SortKind::Measured(b)) => compare_for_sort(a, b),
            (SortKind::Text, SortKind::Text) => {
                signex_types::designator::compare_references(&self.lowercase, &other.lowercase)
            }
            (a, b) => a.rank().cmp(&b.rank()),
        }
    }
}

/// Stable-sort `items` by the key `key_of` yields, computing each item's
/// key exactly once.
pub(super) fn sort_by_cell_key<T>(
    items: &mut Vec<T>,
    mut key_of: impl FnMut(&T) -> CellSortKey,
    descending: bool,
) {
    let mut keyed: Vec<(CellSortKey, T)> =
        items.drain(..).map(|item| (key_of(&item), item)).collect();
    keyed.sort_by(|(a, _), (b, _)| {
        let ord = a.compare(b);
        if descending { ord.reverse() } else { ord }
    });
    items.extend(keyed.into_iter().map(|(_, item)| item));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cells chosen to hit numbers, units, mixed case, empties, ties,
    /// whitespace-padded numbers, NaN and duplicates.
    fn fixture() -> Vec<String> {
        [
            "10", "9", "2.5", "  7 ", "1e2", "-3", "10k", "4.7k", "100nF", "100NF", "abc", "ABC",
            "Abc", "", "", " ", "0", "-0", "0.0", "1.0.10", "1.0.2", "Zeta", "alpha", "10", "—",
            "R0603", "r0603", "10 ", "inf", "-inf", "x", "X", "NaN", "nan", "-NaN",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
    }

    /// Antisymmetry and transitivity over every pair and triple: the
    /// contract `slice::sort_by` may panic on.
    #[test]
    fn comparator_is_a_total_order() {
        let keys: Vec<CellSortKey> = fixture()
            .iter()
            .map(|c| CellSortKey::from_cell(c))
            .collect();
        for a in &keys {
            for b in &keys {
                assert_eq!(a.compare(b), b.compare(a).reverse());
                for c in &keys {
                    if a.compare(b).is_le() && b.compare(c).is_le() {
                        assert!(a.compare(c).is_le());
                    }
                }
            }
        }
    }

    /// A long mixed column sorts without tripping std's total-order
    /// check, in both directions, with numbers ahead of text.
    #[test]
    fn long_mixed_column_sorts_without_panic() {
        let base = fixture();
        let mut items: Vec<String> = (0..64).flat_map(|_| base.iter().cloned()).collect();
        for descending in [true, false] {
            sort_by_cell_key(&mut items, |s| CellSortKey::from_cell(s), descending);
        }
        let first_text = items
            .iter()
            .position(|c| matches!(CellSortKey::from_cell(c).kind, SortKind::Text));
        let last_number = items
            .iter()
            .rposition(|c| matches!(CellSortKey::from_cell(c).kind, SortKind::Number(_)));
        assert!(last_number < first_text, "numbers precede text");
    }

    #[test]
    fn mixed_numbers_and_text_have_a_defined_order() {
        let mut items: Vec<String> = ["10k", "NaN", "10", "abc", "9", "", "-3", "inf"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        sort_by_cell_key(&mut items, |s| CellSortKey::from_cell(s), false);
        assert_eq!(items, ["-3", "9", "10", "inf", "", "10k", "abc", "NaN"]);
    }

    #[test]
    fn ties_keep_original_order() {
        let mut items: Vec<(usize, String)> = ["b", "A", "a", "B", "a"]
            .iter()
            .enumerate()
            .map(|(i, s)| (i, (*s).to_string()))
            .collect();
        sort_by_cell_key(&mut items, |(_, cell)| CellSortKey::from_cell(cell), false);
        let order: Vec<usize> = items.iter().map(|(i, _)| *i).collect();
        assert_eq!(order, vec![1, 2, 4, 0, 3]);
    }

    #[test]
    fn numbers_sort_numerically_not_lexically() {
        let mut items: Vec<String> = ["10", "9", "100"].iter().map(|s| s.to_string()).collect();
        sort_by_cell_key(&mut items, |s| CellSortKey::from_cell(s), false);
        assert_eq!(items, ["9", "10", "100"]);
    }

    #[test]
    fn key_computation_is_linear_in_row_count() {
        for n in [16usize, 256, 4096] {
            let mut items: Vec<usize> = (0..n).map(|i| (i * 7919) % n).collect();
            let mut calls = 0usize;
            sort_by_cell_key(
                &mut items,
                |v| {
                    calls += 1;
                    CellSortKey::from_cell(&v.to_string())
                },
                false,
            );
            assert_eq!(calls, n, "one key derivation per row, not per comparison");
        }
    }
}
