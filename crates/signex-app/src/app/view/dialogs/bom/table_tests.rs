use super::{flag_ink, is_mono, is_numeric, line_fill, sorted_row_order};
use signex_output::{BomColumn, BomIssueSeverity, BomRow};
use signex_types::theme::{ThemeId, theme_tokens};

fn row(references: &[&str], qty: u32, value: &str) -> BomRow {
    BomRow {
        references: references.iter().map(|r| r.to_string()).collect(),
        qty,
        value: value.to_string(),
        ..BomRow::default()
    }
}

/// The export pipeline orders rollup rows by their first designator with
/// `signex_types::designator::compare_references`, and the preview modal
/// opens with the Designator column sort pre-seeded. Sorting the joined
/// reference string with `str::cmp` here showed R1, R10, R2 in the modal
/// over a CSV/HTML/XLSX reading R1, R2, R10 — the preview has to agree
/// with the file the user exports one click later.
#[test]
fn designator_sort_matches_the_export_order() {
    let rows = vec![
        row(&["R10"], 1, "1k"),
        row(&["R2"], 1, "2k"),
        row(&["R1", "R11"], 2, "3k"),
    ];
    let columns = vec![BomColumn::Designator, BomColumn::Qty];

    let ascending = sorted_row_order(&rows, &columns, Some((0, true)));
    assert_eq!(ascending, vec![2, 1, 0], "R1 < R2 < R10");

    let descending = sorted_row_order(&rows, &columns, Some((0, false)));
    assert_eq!(descending, vec![0, 1, 2], "descending is the exact reverse");
}

/// Qty stays numeric and the text columns stay case-insensitive — the
/// natural-order change must not have leaked into the other branches.
#[test]
fn other_columns_keep_their_own_orders() {
    let rows = vec![
        row(&["R1"], 10, "beta"),
        row(&["R2"], 2, "Alpha"),
        row(&["R3"], 9, "gamma"),
    ];
    let columns = vec![BomColumn::Designator, BomColumn::Qty, BomColumn::Value];

    assert_eq!(
        sorted_row_order(&rows, &columns, Some((1, true))),
        vec![1, 2, 0]
    );
    assert_eq!(
        sorted_row_order(&rows, &columns, Some((2, true))),
        vec![1, 0, 2]
    );
}

/// No sort spec, or one pointing past the column set, leaves the rollup
/// order the export pipeline already produced.
#[test]
fn unsorted_and_out_of_range_keep_rollup_order() {
    let rows = vec![row(&["R10"], 1, "a"), row(&["R2"], 1, "b")];
    let columns = vec![BomColumn::Designator];

    assert_eq!(sorted_row_order(&rows, &columns, None), vec![0, 1]);
    assert_eq!(
        sorted_row_order(&rows, &columns, Some((7, true))),
        vec![0, 1]
    );
}

/// Pins behaviour (not a bug repro): descending sort reverses the comparison, not the slice, so rows with
/// equal keys keep rollup order in both directions (and the cached
/// lowercase key makes "Alpha"/"alpha" equal).
#[test]
fn descending_sort_is_stable_for_equal_keys() {
    let rows = vec![
        row(&["R1"], 1, "Alpha"),
        row(&["R2"], 1, "beta"),
        row(&["R3"], 1, "alpha"),
        row(&["R4"], 1, "BETA"),
    ];
    let columns = vec![BomColumn::Designator, BomColumn::Value];

    assert_eq!(
        sorted_row_order(&rows, &columns, Some((1, true))),
        vec![0, 2, 1, 3]
    );
    assert_eq!(
        sorted_row_order(&rows, &columns, Some((1, false))),
        vec![1, 3, 0, 2]
    );
}

/// Value sorts by quantity, not by text: `10k` after `2k` and `4k7`
/// between them, parsed values ahead of free text, `R2` before `R10`.
#[test]
fn value_column_sorts_by_quantity() {
    let rows = vec![
        row(&["R1"], 1, "10k"),
        row(&["R2"], 1, "2k"),
        row(&["R3"], 1, "DNP"),
        row(&["R4"], 1, "4k7"),
        row(&["C1"], 1, "1uF"),
        row(&["C2"], 1, "100n"),
    ];
    let columns = vec![BomColumn::Designator, BomColumn::Value];
    let asc = sorted_row_order(&rows, &columns, Some((1, true)));
    // Ohm family (2k, 4k7, 10k), then farad (100n, 1uF), then text.
    assert_eq!(asc, vec![1, 3, 0, 5, 4, 2]);
    let desc = sorted_row_order(&rows, &columns, Some((1, false)));
    assert_eq!(desc, vec![2, 4, 5, 0, 3, 1]);
}

/// m112: the numbers (Qty, prices, Stock, Buy Qty) are right-aligned in the
/// mono face; the footprint is mono too; text columns are neither.
#[test]
fn numeric_columns_are_right_aligned_and_mono() {
    for c in [
        BomColumn::Qty,
        BomColumn::UnitPrice,
        BomColumn::ExtendedPrice,
        BomColumn::Stock,
        BomColumn::BuyQty,
    ] {
        assert!(is_numeric(&c) && is_mono(&c), "{c:?}");
    }
    assert!(!is_numeric(&BomColumn::Footprint) && is_mono(&BomColumn::Footprint));
    for c in [
        BomColumn::Designator,
        BomColumn::Value,
        BomColumn::Description,
        BomColumn::Name,
    ] {
        assert!(!is_numeric(&c) && !is_mono(&c), "{c:?}");
    }
}

/// m112 and decisions F25, on both Graphite themes: a flagged line takes the
/// soft warning (or error) tint and its gutter the flag colour; a selected
/// line takes the neutral selection fill, never the warning amber; a clean
/// line has no fill at rest and notes flag nothing.
#[test]
fn flagged_lines_are_tinted_and_selection_stays_neutral() {
    for &id in ThemeId::BUILTINS {
        let t = theme_tokens(id);
        let r = crate::styles::tokens::resolve(&t);
        let warn = Some(BomIssueSeverity::Warning);
        let tint = line_fill(&t, warn, false).expect("a warning tints its line");
        assert_eq!(
            (tint.r, tint.g, tint.b),
            (r.warn_soft.r, r.warn_soft.g, r.warn_soft.b)
        );
        assert!(tint.a < r.warn_soft.a, "{id:?}: the drawn 70 % mix");
        assert_eq!(flag_ink(&t, warn), Some(crate::styles::ti(t.warning)));

        let error = line_fill(&t, Some(BomIssueSeverity::Error), false).expect("tinted");
        assert_eq!(
            (error.r, error.g, error.b),
            (r.err_soft.r, r.err_soft.g, r.err_soft.b)
        );
        assert_eq!(
            flag_ink(&t, Some(BomIssueSeverity::Error)),
            Some(crate::styles::ti(t.error))
        );

        let selection = crate::styles::ti(t.selection);
        assert_eq!(line_fill(&t, warn, true), Some(selection), "{id:?}");
        assert_eq!(line_fill(&t, None, true), Some(selection), "{id:?}");
        assert_ne!(Some(selection), line_fill(&t, warn, false), "{id:?}");

        assert_eq!(line_fill(&t, None, false), None);
        assert_eq!(line_fill(&t, Some(BomIssueSeverity::Info), false), None);
        assert_eq!(flag_ink(&t, Some(BomIssueSeverity::Info)), None);
        assert_eq!(flag_ink(&t, None), None);
    }
}
