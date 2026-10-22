//! Row sorting for the library browser grid.
//!
//! Each row's sort key (numeric parse and lowercase form of the display
//! string) is derived exactly once, then the rows are stably sorted on
//! the cached keys. Deriving keys inside the comparator would repeat
//! that allocation and parsing work O(n log n) times per sort.

use std::cmp::Ordering;

/// Precomputed, comparison-ready form of one cell.
///
/// Ordering: if both cells parse as `f64` (after trimming) they compare
/// numerically, with NaN comparing equal to everything; otherwise they
/// compare case-insensitively. This avoids the "lexical sort on numeric
/// columns" problem without needing a typed schema at compare time.
struct CellSortKey {
    number: Option<f64>,
    lowercase: String,
}

impl CellSortKey {
    fn new(cell: &str) -> Self {
        Self {
            number: cell.trim().parse::<f64>().ok(),
            lowercase: cell.to_lowercase(),
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        if let (Some(a), Some(b)) = (self.number, other.number) {
            return a.partial_cmp(&b).unwrap_or(Ordering::Equal);
        }
        self.lowercase.cmp(&other.lowercase)
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

    /// The pre-#151 comparator, kept verbatim as the reference.
    fn old_compare_cells(a: &str, b: &str) -> Ordering {
        if let (Ok(na), Ok(nb)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            return na.partial_cmp(&nb).unwrap_or(Ordering::Equal);
        }
        a.to_lowercase().cmp(&b.to_lowercase())
    }

    /// Cells chosen to hit numbers, units, mixed case, empties, ties,
    /// whitespace-padded numbers and duplicates. Each carries its
    /// original index so stability is observable.
    fn fixture() -> Vec<(usize, String)> {
        [
            "10", "9", "2.5", "  7 ", "1e2", "-3", "10k", "4.7k", "100nF", "100NF", "abc", "ABC",
            "Abc", "", "", " ", "0", "-0", "0.0", "1.0.10", "1.0.2", "Zeta", "alpha", "10", "—",
            "R0603", "r0603", "10 ", "inf", "-inf", "x", "X",
        ]
        .iter()
        .enumerate()
        .map(|(i, s)| (i, (*s).to_string()))
        .collect()
    }

    /// The mixed number/text ordering is not a total order (9 < 10 <
    /// 10k < 9), so std's sort may panic on a long mixed input - a
    /// pre-existing hazard, kept identical here. Equivalence is pinned
    /// two ways that cannot trip that check: every pairwise comparison
    /// agrees, and short chunks (insertion-sorted, no detection)
    /// produce the same order.
    #[test]
    fn pairwise_comparison_matches_old_comparator() {
        let cells = fixture();
        for (_, a) in &cells {
            for (_, b) in &cells {
                let new = CellSortKey::new(a).compare(&CellSortKey::new(b));
                assert_eq!(new, old_compare_cells(a, b), "{a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn matches_old_comparator_order_ascending_and_descending() {
        for descending in [false, true] {
            for chunk in fixture().chunks(16) {
                let mut expected = chunk.to_vec();
                expected.sort_by(|a, b| {
                    let ord = old_compare_cells(&a.1, &b.1);
                    if descending { ord.reverse() } else { ord }
                });
                let mut actual = chunk.to_vec();
                sort_by_cell_key(&mut actual, |(_, cell)| cell.clone(), descending);
                assert_eq!(actual, expected, "descending = {descending}");
            }
        }
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
