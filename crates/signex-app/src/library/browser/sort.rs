//! Row sorting for the library browser grid.
//!
//! Each row's sort key (numeric parse and lowercase form of the display
//! string) is derived exactly once, then the rows are stably sorted on
//! the cached keys. Deriving keys inside the comparator would repeat
//! that allocation and parsing work O(n log n) times per sort.

use std::cmp::Ordering;

/// Precomputed, comparison-ready form of one cell.
///
/// Ordering is a strict total order: every cell that parses as a
/// non-NaN `f64` (after trimming) ranks before every other cell; numbers
/// compare with `f64::total_cmp`, everything else (text, empty cells and
/// a literal `NaN`) compares case-insensitively. Ranking by kind first is
/// what keeps mixed columns (`9`, `10`, `10k`) transitive, which
/// `slice::sort_by` requires. It also avoids the "lexical sort on numeric
/// columns" problem without needing a typed schema at compare time.
struct CellSortKey {
    number: Option<f64>,
    lowercase: String,
}

impl CellSortKey {
    fn new(cell: &str) -> Self {
        Self {
            number: cell.trim().parse::<f64>().ok().filter(|n| !n.is_nan()),
            lowercase: cell.to_lowercase(),
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (self.number, other.number) {
            (Some(a), Some(b)) => a.total_cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.lowercase.cmp(&other.lowercase),
        }
    }
}

/// Stable-sort `items` by the cell string `cell_of` yields, computing
/// each item's key exactly once.
pub(super) fn sort_by_cell_key<T>(
    items: &mut Vec<T>,
    mut cell_of: impl FnMut(&T) -> String,
    descending: bool,
) {
    let mut keyed: Vec<(CellSortKey, T)> = items
        .drain(..)
        .map(|item| (CellSortKey::new(&cell_of(&item)), item))
        .collect();
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
        let keys: Vec<CellSortKey> = fixture().iter().map(|c| CellSortKey::new(c)).collect();
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
            sort_by_cell_key(&mut items, |s| s.clone(), descending);
        }
        let first_text = items
            .iter()
            .position(|c| CellSortKey::new(c).number.is_none());
        let last_number = items
            .iter()
            .rposition(|c| CellSortKey::new(c).number.is_some());
        assert!(last_number < first_text, "numbers precede text");
    }

    #[test]
    fn mixed_numbers_and_text_have_a_defined_order() {
        let mut items: Vec<String> = ["10k", "NaN", "10", "abc", "9", "", "-3", "inf"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        sort_by_cell_key(&mut items, |s| s.clone(), false);
        assert_eq!(items, ["-3", "9", "10", "inf", "", "10k", "abc", "NaN"]);
    }

    #[test]
    fn ties_keep_original_order() {
        let mut items: Vec<(usize, String)> = ["b", "A", "a", "B", "a"]
            .iter()
            .enumerate()
            .map(|(i, s)| (i, (*s).to_string()))
            .collect();
        sort_by_cell_key(&mut items, |(_, cell)| cell.clone(), false);
        let order: Vec<usize> = items.iter().map(|(i, _)| *i).collect();
        assert_eq!(order, vec![1, 2, 4, 0, 3]);
    }

    #[test]
    fn numbers_sort_numerically_not_lexically() {
        let mut items: Vec<String> = ["10", "9", "100"].iter().map(|s| s.to_string()).collect();
        sort_by_cell_key(&mut items, |s| s.clone(), false);
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
                    v.to_string()
                },
                false,
            );
            assert_eq!(calls, n, "one key derivation per row, not per comparison");
        }
    }
}
