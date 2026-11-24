//! Board Reports: open on a table, switch tables, filter and sort the rows
//! on screen, write the table in view as CSV (every row, in the report's
//! order).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_output::{BoardReport, BoardReportKind, ReportCell, ReportColumn, board_report};
use signex_types::designator::compare_references;

use super::{BoardOutputDialog, BoardOutputMsg, BoardReportsState};
use crate::app::{Message, Signex};

/// How Board Reports orders its rows on screen: by one column, either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportSort {
    pub column: ReportColumn,
    pub descending: bool,
}

impl ReportSort {
    /// The order a report of `kind` opens in: Net Lengths longest first,
    /// the others in the order the report makes.
    pub fn opening(kind: BoardReportKind) -> Option<Self> {
        (kind == BoardReportKind::NetLength).then_some(Self {
            column: ReportColumn::Length,
            descending: true,
        })
    }

    /// The order after a click on `column`'s heading while `current` is
    /// shown: the same column turns round, another sorts ascending.
    pub fn picked(current: Option<Self>, column: ReportColumn) -> Self {
        match current {
            Some(sort) if sort.column == column => Self {
                column,
                descending: !sort.descending,
            },
            _ => Self {
                column,
                descending: false,
            },
        }
    }
}

/// The rows of `report` the dialog shows, as indices into `report.rows`:
/// those with a cell whose CSV text holds `filter` (ignoring case; an empty
/// filter keeps every row), in `sort`'s order. The sort is stable, so rows
/// that tie keep the report's order.
pub fn shown_rows(report: &BoardReport, filter: &str, sort: Option<ReportSort>) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    let mut rows: Vec<usize> = (0..report.rows.len())
        .filter(|&i| {
            needle.is_empty()
                || report.rows[i]
                    .iter()
                    .any(|cell| cell.csv_text().to_lowercase().contains(&needle))
        })
        .collect();
    let Some(sort) = sort else {
        return rows;
    };
    let Some(at) = report.columns.iter().position(|c| *c == sort.column) else {
        return rows;
    };
    rows.sort_by(|&a, &b| {
        let (x, y) = (report.rows[a].get(at), report.rows[b].get(at));
        // A missing value goes last either way round.
        match (is_missing(x), is_missing(y)) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) if sort.descending => compare_cells(x, y).reverse(),
            (false, false) => compare_cells(x, y),
        }
    });
    rows
}

fn is_missing(cell: Option<&ReportCell>) -> bool {
    matches!(cell, None | Some(ReportCell::Missing))
}

/// Two present cells of one column: numbers by value, text in designator
/// order (`R2` before `R10`).
fn compare_cells(a: Option<&ReportCell>, b: Option<&ReportCell>) -> Ordering {
    use ReportCell as C;
    let number = |cell: Option<&ReportCell>| match cell {
        Some(C::Count(n)) => Some(*n as f64),
        Some(C::Millimetres(v) | C::Degrees(v)) => Some(*v),
        _ => None,
    };
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        _ => compare_references(
            &a.map(ReportCell::csv_text).unwrap_or_default(),
            &b.map(ReportCell::csv_text).unwrap_or_default(),
        ),
    }
}

impl Signex {
    /// Open Board Reports on `kind` for the board in view.
    pub(super) fn open_board_report(&mut self, kind: BoardReportKind) {
        let Some((board, board_name)) = self.board_in_view() else {
            self.explain_no_board();
            return;
        };
        let Some(report) = self.board_report_of(&board, kind) else {
            self.refuse_board_gone(&board);
            return;
        };
        self.ui_state.board_output.dialog = Some(Box::new(BoardOutputDialog::Reports(
            BoardReportsState::new(board, board_name, report),
        )));
    }

    /// Change the open Board Reports' view (filter, sort); the table and
    /// the board are untouched.
    pub(super) fn edit_board_report(&mut self, change: impl FnOnce(&mut BoardReportsState)) {
        if let Some(BoardOutputDialog::Reports(state)) =
            self.ui_state.board_output.dialog.as_deref_mut()
        {
            change(state);
            state.refresh_shown();
        }
    }

    /// Show table `kind`, made from the board as it is now. Another kind
    /// opens unfiltered in its own order; the same kind keeps the filter
    /// and the sort.
    pub(super) fn pick_board_report(&mut self, kind: BoardReportKind) {
        let Some(BoardOutputDialog::Reports(state)) = self.ui_state.board_output.dialog.as_deref()
        else {
            return;
        };
        let board = state.board.clone();
        let Some(report) = self.board_report_of(&board, kind) else {
            self.refuse_board_gone(&board);
            return;
        };
        if let Some(BoardOutputDialog::Reports(state)) =
            self.ui_state.board_output.dialog.as_deref_mut()
        {
            if state.report.kind != kind {
                state.filter.clear();
                state.sort = ReportSort::opening(kind);
            }
            state.report = report;
            state.refresh_shown();
        }
    }

    /// Report `kind` of the open board at `board`; `None` when it is not
    /// open.
    pub(super) fn board_report_of(
        &self,
        board: &Path,
        kind: BoardReportKind,
    ) -> Option<BoardReport> {
        self.document_state
            .pcb_board_shown(board)
            .map(|b| board_report(&b, kind))
    }

    /// What a CSV export writes: the table in view; with Board Print open,
    /// the BOM of that dialog's board; else the BOM of the board in view.
    pub(crate) fn board_csv_source(&self) -> Option<(PathBuf, BoardReportKind)> {
        match self.ui_state.board_output.dialog.as_deref() {
            Some(BoardOutputDialog::Reports(state)) => {
                Some((state.board.clone(), state.report.kind))
            }
            Some(BoardOutputDialog::Print(state)) => {
                Some((state.board.clone(), BoardReportKind::Bom))
            }
            None => self
                .board_in_view()
                .map(|(board, _)| (board, BoardReportKind::Bom)),
        }
    }

    /// The save dialog for [`Self::board_csv_source`].
    pub(super) fn request_board_report_csv(&mut self) -> Task<Message> {
        let Some((board, kind)) = self.board_csv_source() else {
            self.explain_no_board();
            return Task::none();
        };
        let target = self.save_target(Some(&board));
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        let file_name = target.file_name("board", &format!("-{}", kind.id()), "csv");
        Task::perform(
            async move {
                target
                    .dialog(
                        picker
                            .dialog()
                            .title_in(&i18n, ids::BOARD_REPORTS_EXPORT_TITLE),
                    )
                    .await
                    .filter_in(&i18n, ids::FILE_FILTER_CSV_EXT, &["csv"])
                    .set_file_name(file_name)
                    .save_file()
                    .await
            },
            move |path| match path {
                Some(path) => Message::from(BoardOutputMsg::CsvPathChosen { board, kind, path }),
                None => Message::noop(),
            },
        )
    }

    /// Write report `kind` of `board`, as the board is now, to `path`.
    pub(super) fn write_board_report_csv(
        &mut self,
        board: &Path,
        kind: BoardReportKind,
        path: PathBuf,
    ) -> Task<Message> {
        let Some(report) = self.board_report_of(board, kind) else {
            self.refuse_board_gone(board);
            return Task::none();
        };
        self.write_report_file(&report, path)
    }

    /// Write `report` as CSV to `path` off the UI thread.
    pub(super) fn write_report_file(
        &mut self,
        report: &BoardReport,
        path: PathBuf,
    ) -> Task<Message> {
        let log = format!(
            "Wrote {} ({} report, {} rows)",
            path.display(),
            report.kind.id(),
            report.rows.len()
        );
        self.write_export(path, report.to_csv(), "CSV", log, None, Vec::new())
    }

    /// The board an output was asked for has closed since: say so.
    pub(super) fn refuse_board_gone(&mut self, board: &Path) {
        let reason =
            Translatable::new(ids::BOARD_OUTPUT_BOARD_GONE).arg("board", super::file_title(board));
        self.refuse_export(reason);
    }
}
