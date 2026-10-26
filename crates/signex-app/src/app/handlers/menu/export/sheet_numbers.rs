//! The project's sheet numbering (#1081) applied to the printed pages, and its
//! issues as Messages lines. The numbers come from `signex_net::sheet_numbering`.

use std::path::Path;

use signex_net::{NumberingIssue, SheetNumbering, SheetPage};
use signex_output::SheetSnapshot;

use crate::app::state::scope::CaseFold;

/// `pages` with the project's numbers: each page takes its placement's number
/// and the project's page count, so a page prints the same "Sheet N of M"
/// whichever pages an export includes. A page the numbering does not know
/// keeps its number.
pub(super) fn numbered_pages(
    pages: Vec<SheetSnapshot>,
    numbering: &SheetNumbering,
    base_dir: Option<&Path>,
    fold: CaseFold,
) -> Vec<SheetSnapshot> {
    pages
        .into_iter()
        .map(|page| {
            let key = crate::app::project_sheets::sheet_key_with(&page.path, base_dir, fold);
            match numbering.number_of(&key, &page.placement) {
                Some(number) => SheetSnapshot {
                    sheet_number: number,
                    sheet_count: numbering.count(),
                    ..page
                },
                None => page,
            }
        })
        .collect()
}

/// One Messages line per issue, each starting `Sheet numbering:`.
pub(super) fn numbering_messages(
    numbering: &SheetNumbering,
    issues: &[NumberingIssue],
) -> Vec<String> {
    let page = |index: usize| numbering.pages().get(index);
    let label = |index: usize| page(index).map(SheetPage::label).unwrap_or_default();
    issues
        .iter()
        .map(|issue| match issue {
            NumberingIssue::Clash { number, pages } => {
                let names: Vec<String> = pages.iter().map(|index| label(*index)).collect();
                format!(
                    "Sheet numbering: pages {} all print number {number}",
                    names.join(", ")
                )
            }
            NumberingIssue::Gap { missing } => {
                let noun = if missing.len() == 1 {
                    "number"
                } else {
                    "numbers"
                };
                let list: Vec<String> = missing.iter().map(usize::to_string).collect();
                format!("Sheet numbering: no page prints {noun} {}", list.join(", "))
            }
            NumberingIssue::OutOfRange {
                page: index,
                number,
            } => format!(
                "Sheet numbering: page {} is numbered {number}, above the page count {}",
                label(*index),
                numbering.count()
            ),
            NumberingIssue::InvalidOverride { page: index, text } => format!(
                "Sheet numbering: page {} has the manual number '{text}', which is not a \
                 positive whole number; it is numbered {}",
                label(*index),
                page(*index).map_or(0, |found| found.number)
            ),
        })
        .collect()
}
