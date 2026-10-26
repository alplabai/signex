//! Page-range selection by sheet number (#1081): a project's pages carry the
//! project's numbers, which need not match their printed positions.

use super::page_range::{preview_page_indices, resolve_page_range};
use super::tests::sample_ctx;
use super::{PageRange, PdfError};
use crate::SheetSnapshot;

/// Printed pages carrying `numbers`, in that order, out of `count` sheets.
fn numbered(numbers: &[usize], count: usize) -> Vec<SheetSnapshot> {
    sample_ctx(numbers.len())
        .sheets
        .into_iter()
        .zip(numbers)
        .map(|(page, number)| SheetSnapshot {
            sheet_number: *number,
            sheet_count: count,
            ..page
        })
        .collect()
}

#[test]
fn specific_selects_the_page_that_carries_the_number() {
    let sheets = numbered(&[1, 4, 2, 3], 4);
    assert_eq!(
        resolve_page_range(&PageRange::Specific(vec![2]), &sheets, 0).unwrap(),
        [2]
    );
    assert_eq!(
        preview_page_indices(&PageRange::Specific(vec![2]), &sheets, 0),
        [2]
    );
}

#[test]
fn a_range_selects_the_numbers_inside_it_in_printed_order() {
    let sheets = numbered(&[1, 3, 4, 2], 4);
    for range in [PageRange::Range(2, 3), PageRange::Range(3, 2)] {
        assert_eq!(resolve_page_range(&range, &sheets, 0).unwrap(), [1, 3]);
        assert_eq!(preview_page_indices(&range, &sheets, 0), [1, 3]);
    }
}

#[test]
fn a_number_no_printed_page_carries_is_named_in_the_error() {
    // Five sheets in the project; the page numbered 5 is not printed.
    let sheets = numbered(&[1, 4, 2, 3], 5);
    let err = resolve_page_range(&PageRange::Specific(vec![5]), &sheets, 0).unwrap_err();
    assert!(matches!(err, PdfError::NotPrinted(5)), "{err:?}");
    assert!(err.to_string().contains("sheet 5"), "{err}");
    let err = resolve_page_range(&PageRange::Range(4, 5), &sheets, 0).unwrap_err();
    assert!(matches!(err, PdfError::NotPrinted(5)), "{err:?}");
    // Above the project's count: the old out-of-range error.
    let err = resolve_page_range(&PageRange::Specific(vec![6]), &sheets, 0).unwrap_err();
    assert!(matches!(err, PdfError::OutOfRange(6, 5)), "{err:?}");
}

#[test]
fn the_preview_selects_nothing_for_an_unprinted_number() {
    let sheets = numbered(&[1, 4, 2, 3], 5);
    assert_eq!(
        preview_page_indices(&PageRange::Specific(vec![5, 4]), &sheets, 0),
        [1]
    );
    assert!(preview_page_indices(&PageRange::Range(4, 5), &sheets, 0).is_empty());
}

#[test]
fn a_manual_number_above_the_count_is_still_selectable() {
    let sheets = numbered(&[1, 7, 2], 3);
    assert_eq!(
        resolve_page_range(&PageRange::Specific(vec![7]), &sheets, 0).unwrap(),
        [1]
    );
}

#[test]
fn current_is_the_current_index_whatever_the_numbers() {
    let sheets = numbered(&[1, 4, 2, 3], 4);
    assert_eq!(
        resolve_page_range(&PageRange::Current, &sheets, 2).unwrap(),
        [2]
    );
}
