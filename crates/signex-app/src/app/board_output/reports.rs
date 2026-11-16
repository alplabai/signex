//! Board Reports: open on a table, switch tables, write the table in view
//! as CSV.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_output::{BoardReport, BoardReportKind, board_report};

use super::{BoardOutputDialog, BoardOutputMsg, BoardReportsState};
use crate::app::{Message, Signex};

impl Signex {
    /// Open Board Reports on `kind` for the board in view.
    pub(super) fn open_board_report(&mut self, kind: BoardReportKind) {
        let Some((board, board_name)) = self.board_in_view() else {
            self.explain_no_board();
            return;
        };
        let Some(report) = self.board_report_of(&board, kind) else {
            return;
        };
        self.ui_state.board_output.dialog = Some(BoardOutputDialog::Reports(BoardReportsState {
            board,
            board_name,
            report,
        }));
    }

    /// Show table `kind`, made from the board as it is now.
    pub(super) fn pick_board_report(&mut self, kind: BoardReportKind) {
        let Some(BoardOutputDialog::Reports(state)) = self.ui_state.board_output.dialog.as_ref()
        else {
            return;
        };
        let board = state.board.clone();
        let Some(report) = self.board_report_of(&board, kind) else {
            return;
        };
        if let Some(BoardOutputDialog::Reports(state)) = self.ui_state.board_output.dialog.as_mut()
        {
            state.report = report;
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
            .pcb_board(board)
            .map(|b| board_report(b, kind))
    }

    /// The save dialog for the table in view, or the BOM of the board in
    /// view when Board Reports is not open.
    pub(super) fn request_board_report_csv(&mut self) -> Task<Message> {
        let picked = match self.ui_state.board_output.dialog.as_ref() {
            Some(BoardOutputDialog::Reports(state)) => {
                Some((state.board.clone(), state.report.kind))
            }
            _ => self
                .board_in_view()
                .map(|(board, _)| (board, BoardReportKind::Bom)),
        };
        let Some((board, kind)) = picked else {
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
