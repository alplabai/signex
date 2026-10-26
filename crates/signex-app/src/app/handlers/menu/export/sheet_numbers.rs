//! The project's sheet numbering (#1081) applied to the printed pages, and its
//! issues as Messages lines. The numbers come from `signex_net::sheet_numbering`.

use std::path::{Path, PathBuf};

use signex_net::{NumberingIssue, SheetNumbering, SheetPage};
use signex_output::SheetSnapshot;
use signex_types::schematic::InstancePath;

use crate::app::state::scope::CaseFold;

/// The Messages panel cuts a line after this many bytes
/// (`diagnostics::compact_message`); a list is capped to fit inside it.
const MAX_LINE_BYTES: usize = 157;

/// A project's sheet numbering as one export scope derived it, with what is
/// needed to key a printed page's path against it.
pub(crate) struct ScopeNumbering {
    numbering: SheetNumbering,
    issues: Vec<NumberingIssue>,
    base_dir: Option<PathBuf>,
    fold: CaseFold,
}

impl ScopeNumbering {
    pub(super) fn new(
        numbering: SheetNumbering,
        issues: Vec<NumberingIssue>,
        base_dir: Option<PathBuf>,
        fold: CaseFold,
    ) -> Self {
        Self {
            numbering,
            issues,
            base_dir,
            fold,
        }
    }

    /// M in "Sheet N of M".
    pub(super) fn count(&self) -> usize {
        self.numbering.count()
    }

    /// The walk position of placement `placement` of the sheet at `path`.
    fn index_of(&self, path: &Path, placement: &InstancePath) -> Option<usize> {
        let key =
            crate::app::project_sheets::sheet_key_with(path, self.base_dir.as_deref(), self.fold);
        self.numbering.index_of(&key, placement)
    }

    /// N for placement `placement` of the sheet at `path`.
    pub(super) fn number_of(&self, path: &Path, placement: &InstancePath) -> Option<usize> {
        self.index_of(path, placement)
            .map(|index| self.numbering.pages()[index].number)
    }

    /// N for the first placement, in walk order, of the sheet at `path`.
    pub(super) fn first_number_of(&self, path: &Path) -> Option<usize> {
        let key =
            crate::app::project_sheets::sheet_key_with(path, self.base_dir.as_deref(), self.fold);
        self.numbering
            .pages()
            .iter()
            .find(|page| page.key == key)
            .map(|page| page.number)
    }

    /// `pages` with the project's numbers, in hierarchy (walk) order: each
    /// page takes its placement's number and every page the project's count,
    /// so a page prints the same "Sheet N of M" whichever pages an export
    /// includes. A page the numbering does not know keeps its number and goes
    /// last, in its current order; that is a declared page whose key lost a
    /// `SheetKeyCollision` (reported as a stitch issue), so its number may
    /// repeat a project page's.
    pub(super) fn numbered_pages(&self, pages: Vec<SheetSnapshot>) -> Vec<SheetSnapshot> {
        let count = self.count();
        let mut placed: Vec<(Option<usize>, SheetSnapshot)> = pages
            .into_iter()
            .map(|page| {
                let index = self.index_of(&page.path, &page.placement);
                let number =
                    index.map_or(page.sheet_number, |at| self.numbering.pages()[at].number);
                let page = SheetSnapshot {
                    sheet_number: number,
                    sheet_count: count,
                    ..page
                };
                (index, page)
            })
            .collect();
        placed.sort_by_key(|(index, _)| index.unwrap_or(usize::MAX));
        placed.into_iter().map(|(_, page)| page).collect()
    }

    /// One Messages line per issue, each starting `Sheet numbering:`.
    pub(crate) fn messages(&self) -> Vec<String> {
        numbering_messages(&self.numbering, &self.issues)
    }
}

/// Log the numbering lines as-is — from the PDF export and the print-preview
/// open, the two user actions that print numbers.
pub(super) fn log_numbering_issues(numbering: Option<&ScopeNumbering>) {
    for line in numbering.map(ScopeNumbering::messages).unwrap_or_default() {
        crate::diagnostics::log_warning(line);
    }
}

fn numbering_messages(numbering: &SheetNumbering, issues: &[NumberingIssue]) -> Vec<String> {
    let page = |index: usize| numbering.pages().get(index);
    let label = |index: usize| page(index).map(SheetPage::label).unwrap_or_default();
    issues
        .iter()
        .map(|issue| match issue {
            NumberingIssue::Clash { number, pages } => {
                let names: Vec<String> = pages.iter().map(|index| label(*index)).collect();
                capped_line(
                    &format!("Sheet numbering: number {number} is printed by pages "),
                    &names,
                )
            }
            NumberingIssue::Gap { missing } => {
                let noun = if missing.len() == 1 {
                    "number"
                } else {
                    "numbers"
                };
                let list: Vec<String> = missing.iter().map(usize::to_string).collect();
                capped_line(&format!("Sheet numbering: no page prints {noun} "), &list)
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
                 valid sheet number; it is numbered {}",
                label(*index),
                page(*index).map_or(0, |found| found.number)
            ),
        })
        .collect()
}

/// `lead` followed by `items` joined by ", ", within [`MAX_LINE_BYTES`]: when
/// the whole list does not fit, the first items that do and "and N more"
/// (the first item itself cut short if even it alone does not fit).
fn capped_line(lead: &str, items: &[String]) -> String {
    let full = format!("{lead}{}", items.join(", "));
    if full.len() <= MAX_LINE_BYTES || items.len() < 2 {
        return full;
    }
    let with_more =
        |shown: &[String], hidden: usize| format!("{lead}{} and {hidden} more", shown.join(", "));
    if let Some(line) = (1..items.len())
        .rev()
        .map(|shown| with_more(&items[..shown], items.len() - shown))
        .find(|line| line.len() <= MAX_LINE_BYTES)
    {
        return line;
    }
    let tail = format!(" and {} more", items.len() - 1);
    let room = MAX_LINE_BYTES.saturating_sub(lead.len() + tail.len());
    let first = crate::text_trunc::prefix_within_bytes(&items[0], room);
    format!("{lead}{first}{tail}")
}

#[cfg(test)]
#[path = "sheet_numbers_tests.rs"]
mod tests;
