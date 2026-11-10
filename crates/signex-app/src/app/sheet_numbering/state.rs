//! The dialog's draft: the pages as read and the numbers being typed. Pure
//! data; the rules come from `signex_net` (`parse_page_number`,
//! `number_issues`).

use std::path::PathBuf;
use std::sync::Arc;

use signex_net::NumberingIssue;

use super::read::{NumberingRead, NumberingRow};
use crate::find_replace::ChildSheetSignature;

/// The Number Schematic Sheets dialog, while open. Nothing is written until
/// Apply, and Apply writes to memory only.
#[derive(Debug, Clone)]
pub struct SheetNumberingState {
    /// The read this dialog waits for or shows.
    pub(crate) generation: u64,
    /// `documents_epoch` when the read started: an engine opened or dropped
    /// since then makes the read stale.
    pub(crate) epoch: u64,
    /// The open sheets' sheet symbols when the read started.
    pub(crate) child_sheets: ChildSheetSignature,
    /// The name manual entries are scoped to.
    pub(crate) project_name: String,
    /// `None` while the read runs.
    pub(crate) read: Option<Arc<NumberingRead>>,
    /// The number typed for each row, as text.
    pub(crate) drafts: Vec<String>,
    /// Auto-number was pressed: a manual number equal to its position is
    /// cleared too, so the page follows its position from now on.
    pub(crate) clear_manual: bool,
    /// Why the read or Apply failed, shown in the dialog.
    pub(crate) error: Option<NumberingProblem>,
}

/// Why the dialog could not read the project or Apply refused. The dialog
/// words it through the catalog; the Messages panel line is [`Self::english`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberingProblem {
    /// The project's root sheet is absent or does not parse.
    RootUnreadable(PathBuf),
    /// The read worker itself failed; the text is the system's reason.
    ReadFailed(String),
    /// A sheet was opened or closed, or a sheet symbol changed, since the read.
    Stale,
    /// A parent sheet that is not open is being saved, renamed or moved.
    Busy(PathBuf),
    /// A parent sheet was written by a newer Signex and opens read-only.
    ReadOnly(PathBuf),
    /// A parent sheet that is not open could not be loaded.
    NotLoaded(PathBuf),
    /// A parent sheet refused the change.
    NotChanged(PathBuf),
}

impl NumberingProblem {
    /// The reason in English, for the Messages panel.
    pub(crate) fn english(&self) -> String {
        match self {
            Self::RootUnreadable(path) => {
                format!("the root sheet {} could not be read", path.display())
            }
            Self::ReadFailed(why) => format!("the sheets could not be read: {why}"),
            Self::Stale => {
                "the sheets changed while the dialog was open; close it and open it again"
                    .to_string()
            }
            Self::Busy(path) => format!("{} is being saved or moved", path.display()),
            Self::ReadOnly(path) => {
                format!(
                    "{} is read-only (written by a newer Signex)",
                    path.display()
                )
            }
            Self::NotLoaded(path) => format!("{} could not be loaded", path.display()),
            Self::NotChanged(path) => format!("{} could not be changed", path.display()),
        }
    }
}

/// One change Apply makes: set the manual number of row `row`, or clear it
/// (`None`) so the page takes its position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageEdit {
    pub(crate) row: usize,
    pub(crate) page: Option<usize>,
}

/// A row with what the view shows for it.
pub(crate) struct NumberedRow<'a> {
    pub(crate) row: &'a NumberingRow,
    pub(crate) draft: &'a str,
    /// The draft is not a sheet number.
    pub(crate) invalid: bool,
    /// Apply changes this row.
    pub(crate) changed: bool,
}

impl SheetNumberingState {
    pub(crate) fn reading(
        generation: u64,
        epoch: u64,
        child_sheets: ChildSheetSignature,
        project_name: String,
    ) -> Self {
        Self {
            generation,
            epoch,
            child_sheets,
            project_name,
            read: None,
            drafts: Vec::new(),
            clear_manual: false,
            error: None,
        }
    }

    /// The read arrived: every row starts at the number it has today.
    pub(crate) fn show(&mut self, read: Arc<NumberingRead>) {
        self.drafts = read.rows.iter().map(|r| r.number.to_string()).collect();
        self.read = Some(read);
        self.clear_manual = false;
        self.error = None;
    }

    /// Why the read failed or Apply refused, while the dialog shows it.
    pub fn problem(&self) -> Option<&NumberingProblem> {
        self.error.as_ref()
    }

    /// How many pages the project has; `None` while the read runs.
    pub fn page_count(&self) -> Option<usize> {
        self.read.as_ref().map(|read| read.rows.len())
    }

    pub(crate) fn rows(&self) -> &[NumberingRow] {
        self.read.as_ref().map_or(&[], |read| read.rows.as_slice())
    }

    /// Type `text` as the number of row `row`. A top-level page has no
    /// symbol to hold a number, so it cannot be edited.
    pub(crate) fn edit(&mut self, row: usize, text: String) {
        let editable = self.rows().get(row).is_some_and(|r| r.placement.is_some());
        if editable && let Some(draft) = self.drafts.get_mut(row) {
            *draft = text;
        }
    }

    /// Every page back to its position in the hierarchy.
    pub(crate) fn auto_number(&mut self) {
        let positions: Vec<String> = self.rows().iter().map(|r| r.position.to_string()).collect();
        self.drafts = positions;
        self.clear_manual = true;
    }

    /// The number row `index` would print: what is typed when it parses,
    /// else (for the issues check) what it prints today.
    fn parsed(&self, index: usize) -> Option<usize> {
        let row = self.rows().get(index)?;
        if row.placement.is_none() {
            return Some(row.number);
        }
        signex_net::parse_page_number(self.drafts.get(index)?)
    }

    /// The rows the view lists, in hierarchy order.
    pub(crate) fn numbered_rows(&self) -> Vec<NumberedRow<'_>> {
        let edits = self.edits();
        self.rows()
            .iter()
            .enumerate()
            .map(|(index, row)| NumberedRow {
                row,
                draft: self.drafts.get(index).map_or("", String::as_str),
                invalid: self.parsed(index).is_none(),
                changed: edits.iter().any(|edit| edit.row == index),
            })
            .collect()
    }

    /// Some number typed is not a sheet number.
    pub(crate) fn has_invalid(&self) -> bool {
        (0..self.rows().len()).any(|index| self.parsed(index).is_none())
    }

    /// Clashes, numbers above the count and gaps among the numbers as typed.
    pub(crate) fn issues(&self) -> Vec<NumberingIssue> {
        let numbers: Vec<usize> = (0..self.rows().len())
            .map(|index| {
                self.parsed(index)
                    .unwrap_or_else(|| self.rows()[index].number)
            })
            .collect();
        signex_net::number_issues(&numbers)
    }

    /// What Apply writes. A number equal to the page's position clears the
    /// manual entry rather than writing it.
    pub(crate) fn edits(&self) -> Vec<PageEdit> {
        let mut edits = Vec::new();
        for (index, row) in self.rows().iter().enumerate() {
            if row.placement.is_none() {
                continue;
            }
            let Some(number) = self.parsed(index) else {
                continue;
            };
            let page = (number != row.position).then_some(number);
            let clears = self.clear_manual && row.manual && page.is_none();
            if number != row.number || clears {
                edits.push(PageEdit { row: index, page });
            }
        }
        edits
    }

    /// Apply is offered: the read is in, every number is valid and one changes.
    pub(crate) fn can_apply(&self) -> bool {
        self.read.is_some() && !self.has_invalid() && !self.edits().is_empty()
    }
}
