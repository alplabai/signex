//! Board Reports (#2404), Board Print / PDF (#2402), the assembly drawings
//! (#2486) and the board STEP export (#2488). The tables, the PDF and the
//! drawings are `signex_output::board_report`, `signex_output::board_pdf`
//! and `signex_output::assembly_drawing`, the STEP model
//! `signex_renderer::board3d::board_step`; this slice holds the open
//! dialog, routes its messages and writes the files through the export
//! write path (a `Task`, crash-safe). Nothing here changes the board, so
//! there is no undo step. Rules: `docs/BOARD_OUTPUT.md`.

mod assembly;
mod operands;
mod print;
mod reports;
mod step;

use std::path::PathBuf;
use std::sync::Arc;

use signex_output::{
    BoardLayerSet, BoardPrintOptions, BoardPrintPages, BoardPrintScale, BoardReport,
    BoardReportKind, ReportColumn,
};
use signex_types::pcb_layer::BoardSide;

pub(crate) use print::{print_text, set_label};
pub use reports::{ReportSort, shown_rows};
pub use step::BoardStepBuilt;
pub(crate) use step::{StepContext, make_step_of};

pub(crate) use assembly::{board_assembly_drawing, export_assembly_svg, side_operand};
pub(crate) use operands::{
    board_report_bom, board_report_components, board_report_net_length, board_report_netlist,
    export_board_pdf, export_board_report, export_board_step, output_board,
};

impl From<BoardOutputMsg> for crate::app::Message {
    fn from(msg: BoardOutputMsg) -> Self {
        Self::Pcb(crate::app::PcbMsg::Output(msg))
    }
}

/// What happened in Board Reports or Board Print / PDF, namespaced under
/// `PcbMsg::Output` and routed to `dispatch_board_output_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardOutputMsg {
    /// A report command ran: Board Reports opens on `kind` for the board
    /// in view.
    ReportRequested(BoardReportKind),
    /// A report kind of Board Reports' switch was picked.
    ReportPicked(BoardReportKind),
    /// The filter field of Board Reports was edited. View only: the export
    /// writes every row.
    ReportFilterEdited(String),
    /// A column heading of Board Reports was clicked: sort by that column,
    /// or turn the order round when it is the sorted one. View only.
    ReportSortPicked(ReportColumn),
    /// `export_board_report` or the dialog's Export CSV: the save dialog for
    /// the report in view.
    CsvExportRequested,
    /// The save dialog answered with `path` for report `kind` of `board`.
    CsvPathChosen {
        board: PathBuf,
        kind: BoardReportKind,
        path: PathBuf,
    },
    /// `print` or `export_board_pdf` on a board: Board Print / PDF opens.
    PrintRequested,
    /// `board_assembly_drawing` in a window: Board Print / PDF opens with
    /// the assembly set of `side` chosen, both sets when no side is named.
    AssemblyPrintRequested(Option<BoardSide>),
    PrintSetToggled(BoardLayerSet),
    PrintPagesPicked(BoardPrintPages),
    PrintScalePicked(BoardPrintScale),
    /// `true`: every layer in black.
    PrintMonochromePicked(bool),
    PrintFlagToggled(BoardPrintFlag),
    /// The dialog's Export PDF: the save dialog.
    PdfExportRequested,
    /// The save dialog answered with `path`; `options` are the dialog's
    /// when Export was pressed.
    PdfPathChosen {
        board: PathBuf,
        options: Box<BoardPrintOptions>,
        path: PathBuf,
    },
    /// The dialog's Export Assembly SVG or `export_assembly_svg`: the
    /// folder dialog.
    SvgExportRequested,
    /// The folder dialog answered with `folder`; `options` are the ones
    /// in use when the export was asked for; `replaced` are the files
    /// already in the folder that the export overwrites.
    SvgFolderChosen {
        board: PathBuf,
        options: Box<BoardPrintOptions>,
        folder: PathBuf,
        replaced: Vec<PathBuf>,
    },
    /// Close, Esc or the header's close button.
    Closed,
    /// `export_board_step` or File ▸ Export ▸ STEP 3D Model: the save
    /// dialog for the board in view.
    StepExportRequested,
    /// The save dialog answered with `path` for `board`.
    StepPathChosen {
        board: PathBuf,
        path: PathBuf,
    },
    /// The STEP model of `board` was made off the update thread, or why it
    /// could not be; `path` is where it goes.
    StepBuilt {
        board: PathBuf,
        path: PathBuf,
        built: Result<Arc<BoardStepBuilt>, String>,
    },
}

/// The on/off settings of a board print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPrintFlag {
    MirrorBottom,
    Outline,
    DrillMarks,
    TitleBlock,
}

impl BoardPrintFlag {
    pub const ALL: [Self; 4] = [
        Self::MirrorBottom,
        Self::Outline,
        Self::DrillMarks,
        Self::TitleBlock,
    ];

    /// The setting in `options` this flag is.
    pub fn get(self, options: &BoardPrintOptions) -> bool {
        match self {
            Self::MirrorBottom => options.mirror_bottom,
            Self::Outline => options.outline,
            Self::DrillMarks => options.drill_marks,
            Self::TitleBlock => options.title_block,
        }
    }

    fn toggle(self, options: &mut BoardPrintOptions) {
        let flag = match self {
            Self::MirrorBottom => &mut options.mirror_bottom,
            Self::Outline => &mut options.outline,
            Self::DrillMarks => &mut options.drill_marks,
            Self::TitleBlock => &mut options.title_block,
        };
        *flag = !*flag;
    }
}

/// The open dialog, one at a time.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardOutputDialog {
    Reports(BoardReportsState),
    Print(BoardPrintState),
}

/// Board Reports: the board, the table in view made from it when its kind
/// was picked, and how the table is filtered and sorted on screen (never in
/// the export).
#[derive(Debug, Clone, PartialEq)]
pub struct BoardReportsState {
    pub board: PathBuf,
    pub board_name: String,
    pub report: BoardReport,
    /// The filter field as typed; a row shows when a cell holds it.
    pub filter: String,
    /// The column the rows are shown sorted by; `None` keeps the report's
    /// own order.
    pub sort: Option<ReportSort>,
    /// The rows the table shows, as indices into `report.rows`: `shown_rows`
    /// of the three fields above, worked out in update so the view only
    /// reads it. Call [`Self::refresh_shown`] after changing them.
    pub shown: Vec<usize>,
}

impl BoardReportsState {
    /// Report `report` of `board`, unfiltered, in the order its kind opens
    /// with.
    pub fn new(board: PathBuf, board_name: String, report: BoardReport) -> Self {
        let sort = ReportSort::opening(report.kind);
        let mut state = Self {
            board,
            board_name,
            report,
            filter: String::new(),
            sort,
            shown: Vec::new(),
        };
        state.refresh_shown();
        state
    }

    /// Work `shown` out again from the report, the filter and the sort.
    pub fn refresh_shown(&mut self) {
        self.shown = shown_rows(&self.report, &self.filter, self.sort);
    }
}

/// Board Print / PDF: the board and the options being set.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardPrintState {
    pub board: PathBuf,
    pub board_name: String,
    pub options: BoardPrintOptions,
}

/// The slice's state in `UiState`: the open dialog, and the print options
/// last used, which the next Board Print / PDF opens with.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardOutputState {
    pub dialog: Option<Box<BoardOutputDialog>>,
    pub print_options: BoardPrintOptions,
}

impl crate::app::Signex {
    pub(crate) fn dispatch_board_output_message(
        &mut self,
        msg: BoardOutputMsg,
    ) -> iced::Task<crate::app::Message> {
        use BoardOutputMsg as M;
        match msg {
            M::ReportRequested(kind) => self.open_board_report(kind),
            M::ReportPicked(kind) => self.pick_board_report(kind),
            M::ReportFilterEdited(filter) => self.edit_board_report(|s| s.filter = filter),
            M::ReportSortPicked(column) => {
                self.edit_board_report(|s| s.sort = Some(ReportSort::picked(s.sort, column)));
            }
            M::CsvExportRequested => return self.request_board_report_csv(),
            M::CsvPathChosen { board, kind, path } => {
                return self.write_board_report_csv(&board, kind, path);
            }
            M::PrintRequested => self.open_board_print(),
            M::AssemblyPrintRequested(side) => self.open_assembly_print(side),
            M::PrintSetToggled(set) => self.edit_board_print(|o| {
                if let Some(at) = o.sets.iter().position(|s| *s == set) {
                    o.sets.remove(at);
                } else {
                    o.sets.push(set);
                }
            }),
            M::PrintPagesPicked(pages) => self.edit_board_print(|o| o.pages = pages),
            M::PrintScalePicked(scale) => self.edit_board_print(|o| o.scale = scale),
            M::PrintMonochromePicked(on) => self.edit_board_print(|o| o.monochrome = on),
            // Mirror is greyed and deaf while no bottom set is ticked; this
            // guard is the backstop.
            M::PrintFlagToggled(BoardPrintFlag::MirrorBottom) => self.edit_board_print(|o| {
                if o.has_bottom_set() {
                    BoardPrintFlag::MirrorBottom.toggle(o);
                }
            }),
            M::PrintFlagToggled(flag) => self.edit_board_print(|o| flag.toggle(o)),
            M::PdfExportRequested => return self.request_board_pdf(),
            M::PdfPathChosen {
                board,
                options,
                path,
            } => return self.write_board_pdf(&board, &options, path),
            M::SvgExportRequested => return self.request_assembly_svg(),
            M::SvgFolderChosen {
                board,
                options,
                folder,
                replaced,
            } => return self.write_assembly_svgs(&board, &options, &folder, &replaced),
            M::Closed => self.ui_state.board_output.dialog = None,
            M::StepExportRequested => return self.request_board_step(),
            M::StepPathChosen { board, path } => {
                return self.build_board_step(
                    &board,
                    path,
                    signex_renderer::board3d::StepSchema::default(),
                );
            }
            M::StepBuilt { board, path, built } => {
                return self.write_board_step(&board, path, built);
            }
        }
        iced::Task::none()
    }

    /// The board in view of the main window and its tab's name; `None`
    /// when the active tab is not a board.
    fn board_in_view(&self) -> Option<(PathBuf, String)> {
        let path = self.active_tab_path().filter(|_| self.has_active_pcb())?;
        let name = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.title.clone())
            .unwrap_or_default();
        Some((path, name))
    }

    /// Say that the board output commands need a board in view.
    fn explain_no_board(&self) {
        let line = signex_i18n::t!(self.ui_state.i18n, signex_i18n::ids::BOARD_OUTPUT_NO_PCB);
        crate::diagnostics::log_info(line.into_string());
    }
}

/// The name a board is shown by when no tab names it: its file stem.
fn file_title(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_assembly;
#[cfg(test)]
mod tests_dialogs;
#[cfg(test)]
mod tests_print;
#[cfg(test)]
mod tests_step;
