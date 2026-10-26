//! Which printed pages a [`PageRange`] selects. `Specific` and `Range` name
//! sheet numbers (the "Sheet N" a page prints), not positions in the printed
//! list (#1081); for pages numbered 1..n in order the two agree.

use super::{PageRange, PdfError};
use crate::SheetSnapshot;

/// The zero-based indices into `sheets` that `range` prints, in printed order.
/// A number that several pages carry selects each of them. An endpoint or
/// requested number that no page carries is an error naming it.
pub(crate) fn resolve_page_range(
    range: &PageRange,
    sheets: &[SheetSnapshot],
    current: usize,
) -> Result<Vec<usize>, PdfError> {
    match range {
        PageRange::All => Ok((0..sheets.len()).collect()),
        PageRange::Current => Ok(vec![current]),
        PageRange::Specific(numbers) => {
            let mut out = Vec::with_capacity(numbers.len());
            for &number in numbers {
                check_printed(sheets, number)?;
                out.extend(carrying(sheets, number));
            }
            Ok(out)
        }
        PageRange::Range(start, end) => {
            let (low, high) = ((*start).min(*end), (*start).max(*end));
            check_printed(sheets, high)?;
            check_printed(sheets, low)?;
            Ok(in_range(sheets, low, high))
        }
    }
}

/// `Ok` when every sheet number `range` names is printed by one of `sheets`,
/// else the error an export of that range would stop with. For a caller that
/// has to say why the preview of `range` is empty.
pub fn check_page_range(range: &PageRange, sheets: &[SheetSnapshot]) -> Result<(), PdfError> {
    resolve_page_range(range, sheets, 0).map(|_| ())
}

/// [`resolve_page_range`] for the preview, which never fails: an unprinted
/// number in `Specific` selects nothing, an unprinted range endpoint empties
/// the range.
pub(crate) fn preview_page_indices(
    range: &PageRange,
    sheets: &[SheetSnapshot],
    current: usize,
) -> Vec<usize> {
    match range {
        PageRange::Current if sheets.is_empty() => Vec::new(),
        PageRange::Specific(numbers) => numbers
            .iter()
            .flat_map(|number| carrying(sheets, *number))
            .collect(),
        _ => resolve_page_range(range, sheets, current).unwrap_or_default(),
    }
}

/// The indices of the pages that print `number`.
fn carrying(sheets: &[SheetSnapshot], number: usize) -> impl Iterator<Item = usize> + '_ {
    sheets
        .iter()
        .enumerate()
        .filter(move |(_, sheet)| sheet.sheet_number == number)
        .map(|(index, _)| index)
}

fn in_range(sheets: &[SheetSnapshot], low: usize, high: usize) -> Vec<usize> {
    sheets
        .iter()
        .enumerate()
        .filter(|(_, sheet)| (low..=high).contains(&sheet.sheet_number))
        .map(|(index, _)| index)
        .collect()
}

/// `Ok` when some page prints `number`. Otherwise a number outside
/// `1..=count` is out of range and one inside it is a sheet that is not
/// among the printed pages.
fn check_printed(sheets: &[SheetSnapshot], number: usize) -> Result<(), PdfError> {
    if carrying(sheets, number).next().is_some() {
        return Ok(());
    }
    let count = sheet_count(sheets);
    if number == 0 || number > count {
        Err(PdfError::OutOfRange(number, count))
    } else {
        Err(PdfError::NotPrinted(number))
    }
}

/// M in "Sheet N of M": the project's sheet count, which may exceed the
/// number of printed pages.
fn sheet_count(sheets: &[SheetSnapshot]) -> usize {
    sheets
        .iter()
        .map(|sheet| sheet.sheet_count)
        .max()
        .unwrap_or(0)
        .max(sheets.len())
}
