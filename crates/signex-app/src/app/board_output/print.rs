//! Board Print / PDF: open with the options last used, change them, write
//! the PDF.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{I18n, MessageId, Translatable, ids, t};
use signex_output::{BoardLayerSet, BoardPdfOutput, BoardPrintOptions, BoardPrintText, board_pdf};

use super::{BoardOutputDialog, BoardOutputMsg, BoardPrintState};
use crate::app::{Message, Signex};

/// The catalog name of a layer set.
pub(crate) fn set_label(set: BoardLayerSet) -> MessageId {
    match set {
        BoardLayerSet::Top => ids::BOARD_PRINT_SET_TOP,
        BoardLayerSet::Bottom => ids::BOARD_PRINT_SET_BOTTOM,
        BoardLayerSet::Inner => ids::BOARD_PRINT_SET_INNER,
        BoardLayerSet::Mechanical => ids::BOARD_PRINT_SET_MECHANICAL,
    }
}

/// The words a printed title block carries, in `i18n`'s language.
pub(super) fn print_text(i18n: &I18n, title: &str) -> BoardPrintText {
    BoardPrintText {
        title: title.to_string(),
        sets: BoardLayerSet::ALL
            .into_iter()
            .map(|set| (set, t!(i18n, set_label(set)).into_string()))
            .collect(),
        mirrored: t!(i18n, ids::BOARD_PRINT_MIRRORED).into_string(),
    }
}

impl Signex {
    /// Open Board Print / PDF for the board in view.
    pub(super) fn open_board_print(&mut self) {
        let Some((board, board_name)) = self.board_in_view() else {
            self.explain_no_board();
            return;
        };
        let options = self.ui_state.board_output.print_options.clone();
        self.ui_state.board_output.dialog =
            Some(Box::new(BoardOutputDialog::Print(BoardPrintState {
                board,
                board_name,
                options,
            })));
    }

    /// Change the open dialog's options; the next opening starts from them.
    pub(super) fn edit_board_print(&mut self, change: impl FnOnce(&mut BoardPrintOptions)) {
        let output = &mut self.ui_state.board_output;
        if let Some(BoardOutputDialog::Print(state)) = output.dialog.as_deref_mut() {
            change(&mut state.options);
            output.print_options = state.options.clone();
        }
    }

    /// The save dialog for the PDF, with the options as they are now.
    pub(super) fn request_board_pdf(&mut self) -> Task<Message> {
        let Some(BoardOutputDialog::Print(state)) = self.ui_state.board_output.dialog.as_deref()
        else {
            return Task::none();
        };
        if state.options.page_plan().is_empty() {
            self.refuse_export(ids::BOARD_PRINT_NO_SETS);
            return Task::none();
        }
        let board = state.board.clone();
        let options = Box::new(state.options.clone());
        let target = self.save_target(Some(&board));
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        let file_name = target.file_name("board", "", "pdf");
        Task::perform(
            async move {
                target
                    .dialog(picker.dialog().title_in(&i18n, ids::FILE_DIALOG_EXPORT_PDF))
                    .await
                    .filter_in(&i18n, ids::FILE_FILTER_PDF, &["pdf"])
                    .set_file_name(file_name)
                    .save_file()
                    .await
            },
            move |path| match path {
                Some(path) => Message::from(BoardOutputMsg::PdfPathChosen {
                    board,
                    options,
                    path,
                }),
                None => Message::noop(),
            },
        )
    }

    /// Draw `board`, as it is now, with `options` and write it to `path`.
    pub(super) fn write_board_pdf(
        &mut self,
        board: &Path,
        options: &BoardPrintOptions,
        path: PathBuf,
    ) -> Task<Message> {
        match self.board_pdf_of(board, options) {
            Ok(output) => self.write_pdf_file(output, path),
            Err(Some(why)) => {
                self.report_export_failure(why);
                Task::none()
            }
            Err(None) => {
                self.refuse_board_gone(board);
                Task::none()
            }
        }
    }

    /// The PDF of the open board at `board`; `Err(None)` when it is not
    /// open, `Err(Some(reason))` when it cannot be drawn.
    pub(super) fn board_pdf_of(
        &self,
        board: &Path,
        options: &BoardPrintOptions,
    ) -> Result<BoardPdfOutput, Option<Translatable>> {
        let pcb = self.document_state.pcb_board_shown(board).ok_or(None)?;
        let title = self
            .document_state
            .tabs
            .iter()
            .find(|tab| tab.path.as_path() == board)
            .map_or_else(|| super::file_title(board), |tab| tab.title.clone());
        let text = print_text(&self.ui_state.i18n, &title);
        board_pdf(&pcb, options, &text)
            .map_err(|e| Some(Translatable::new(ids::BOARD_PRINT_FAILED).arg("why", e.to_string())))
    }

    /// Write a made PDF to `path` off the UI thread.
    pub(super) fn write_pdf_file(
        &mut self,
        output: BoardPdfOutput,
        path: PathBuf,
    ) -> Task<Message> {
        let log = format!(
            "Wrote {} ({} page(s), {} bytes)",
            path.display(),
            output.pages.len(),
            output.bytes.len()
        );
        let warnings = pdf_warnings(&output);
        self.write_export(path, output.bytes, "PDF", log, None, warnings)
    }
}

/// What a made PDF warns of: characters no font covers, and pages the
/// board runs off at 1:1.
pub(super) fn pdf_warnings(output: &BoardPdfOutput) -> Vec<String> {
    let uncovered =
        crate::app::handlers::menu::export::uncovered_chars_warning(&output.uncovered_chars);
    let off_page: Vec<String> = output
        .pages
        .iter()
        .enumerate()
        .filter(|(_, page)| page.overflows)
        .map(|(index, _)| (index + 1).to_string())
        .collect();
    let overflow = (!off_page.is_empty()).then(|| {
        format!(
            "PDF export: at 1:1 the board is larger than the printable area and runs off page(s) {}; choose Fit to Page or a larger page",
            off_page.join(", ")
        )
    });
    uncovered.into_iter().chain(overflow).collect()
}
