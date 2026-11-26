//! Import…: a CSV file read off the update thread into the Import
//! Parameters preview, whose Apply writes every change, on every listed
//! sheet, as one undo step.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::Command;
use signex_i18n::{I18n, ids, t};
use signex_output::parameter_csv::{
    self, ImportPlan, ListedSheet, ParameterCsv, ParameterCsvError,
};

use super::{ImportRead, ParameterIoMsg, file_name};
use crate::app::state::{ModalId, ParameterCell};
use crate::app::{Message, Signex};

/// The largest file an import reads, in MB.
const MAX_CSV_MEGABYTES: u64 = 4;

/// Why a picked file was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportReadError {
    /// The system's words.
    Io(String),
    TooLarge,
    Csv(ParameterCsvError),
}

impl ImportReadError {
    /// The reason, as the import-failed line says it.
    fn why(&self, i18n: &I18n) -> String {
        let plain = |id| i18n.message_plain(id, &[]);
        match self {
            Self::Io(why) => why.clone(),
            Self::TooLarge => t!(
                i18n,
                ids::PARAMETER_IO_WHY_TOO_LARGE,
                "megabytes" => MAX_CSV_MEGABYTES
            )
            .into_string(),
            Self::Csv(ParameterCsvError::NotUtf8) => plain(ids::PARAMETER_IO_WHY_NOT_UTF8),
            Self::Csv(ParameterCsvError::NoHeader) => plain(ids::PARAMETER_IO_WHY_NO_HEADER),
            Self::Csv(ParameterCsvError::NoKeyColumn) => plain(ids::PARAMETER_IO_WHY_NO_KEY),
            Self::Csv(ParameterCsvError::DuplicateColumn(column)) => t!(
                i18n,
                ids::PARAMETER_IO_WHY_DUPLICATE,
                "column" => column.clone()
            )
            .into_string(),
            Self::Csv(ParameterCsvError::Write(why)) => why.clone(),
        }
    }
}

/// Read and parse `path`, refusing a file over [`MAX_CSV_MEGABYTES`].
pub(super) fn read_parameter_csv(path: &Path) -> Result<ParameterCsv, ImportReadError> {
    let io = |e: std::io::Error| ImportReadError::Io(e.to_string());
    let size = std::fs::metadata(path).map_err(io)?.len();
    if size > MAX_CSV_MEGABYTES * 1024 * 1024 {
        return Err(ImportReadError::TooLarge);
    }
    let bytes = std::fs::read(path).map_err(io)?;
    parameter_csv::parse(&bytes).map_err(ImportReadError::Csv)
}

/// The open Import Parameters preview.
#[derive(Debug, Clone)]
pub struct ParameterImportState {
    /// The sheet Import… was pressed on: the active one then.
    pub sheet: PathBuf,
    /// The sheets the file goes into: the ones the Parameter Manager listed
    /// then (`sheet` alone with Active sheet, #1784).
    pub sheets: Vec<PathBuf>,
    /// The file read.
    pub file: PathBuf,
    pub csv: ParameterCsv,
}

impl Signex {
    /// Ask for a CSV file and read it off the update thread, for the sheets
    /// the Parameter Manager lists.
    pub(super) fn import_parameters(&mut self) -> Task<Message> {
        if self.document_state.active_engine().is_none() {
            return Task::none();
        }
        let Some(sheet) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        let sheets = self.parameter_sheets();
        let i18n = &self.ui_state.i18n;
        let mut dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::PARAMETER_IO_IMPORT_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_CSV_EXT, &["csv"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        if let Some(dir) = sheet.parent() {
            dialog = dialog.set_directory(dir);
        }
        Task::perform(
            async move {
                let path = dialog.pick_file().await?;
                let read = path.clone();
                let result =
                    crate::app::path_op_jobs::run_blocking(move || read_parameter_csv(&read))
                        .await
                        .map_err(ImportReadError::Io)
                        .and_then(std::convert::identity);
                Some((path, result))
            },
            move |picked| {
                ParameterIoMsg::ImportRead {
                    sheet,
                    sheets,
                    picked,
                }
                .into()
            },
        )
    }

    /// The file was read: open the preview for `sheets`, or say in Messages
    /// why the file was refused.
    pub(super) fn parameter_import_read(
        &mut self,
        sheet: PathBuf,
        sheets: Vec<PathBuf>,
        picked: Option<ImportRead>,
    ) -> Task<Message> {
        let Some((file, result)) = picked else {
            return Task::none();
        };
        match result {
            Ok(csv) => {
                self.ui_state.parameter_import = Some(Box::new(ParameterImportState {
                    sheet,
                    sheets,
                    file,
                    csv,
                }));
                self.handle_detach_modal(ModalId::ParameterImport)
            }
            Err(error) => {
                let why = error.why(&self.ui_state.i18n);
                let line = t!(
                    self.ui_state.i18n,
                    ids::PARAMETER_IO_IMPORT_FAILED,
                    "file" => file.display().to_string(),
                    "why" => why
                );
                crate::diagnostics::log_warning(line.into_string());
                Task::none()
            }
        }
    }

    /// What Apply would change now: the open preview's file against its
    /// sheets as they are. `None` with no preview, or when the sheet Import…
    /// was pressed on is closed.
    pub(crate) fn parameter_import_plan(&self) -> Option<ImportPlan> {
        self.planned_parameter_import().map(|(_, plan)| plan)
    }

    /// [`Self::parameter_import_plan`], with the path of each sheet a
    /// change's `sheet` index names. A listed sheet closed since is left
    /// out, so its lines are unmatched.
    fn planned_parameter_import(&self) -> Option<(Vec<PathBuf>, ImportPlan)> {
        let state = self.ui_state.parameter_import.as_ref()?;
        self.document_state.engines.get(&state.sheet)?;
        let sheets = self.parameter_documents_of(&state.sheets);
        let listed: Vec<ListedSheet<'_>> = sheets.iter().map(|sheet| sheet.listed()).collect();
        let plan = parameter_csv::plan_import(&listed, &state.csv);
        let paths = sheets
            .iter()
            .map(|sheet| sheet.path.to_path_buf())
            .collect();
        Some((paths, plan))
    }

    /// Apply: every change, on every sheet, as one undo step (the
    /// Parameter Manager's own edit path, so one Undo takes the import back
    /// on each sheet it changed). The preview stays open when the sheet
    /// Import… was pressed on is not the active one or the edit was refused.
    pub(super) fn accept_parameter_import(&mut self) -> Task<Message> {
        let Some(sheet) = self
            .ui_state
            .parameter_import
            .as_ref()
            .map(|state| state.sheet.clone())
        else {
            return Task::none();
        };
        if self.document_state.active_path.as_deref() != Some(sheet.as_path()) {
            let line = t!(
                self.ui_state.i18n,
                ids::PARAMETER_IO_SHEET_NOT_ACTIVE,
                "sheet" => file_name(&sheet)
            );
            crate::diagnostics::log_warning(line.into_string());
            return Task::none();
        }
        let Some((paths, plan)) = self.planned_parameter_import() else {
            return Task::none();
        };
        let mut by_sheet: BTreeMap<PathBuf, Vec<Command>> = BTreeMap::new();
        for change in &plan.changes {
            by_sheet
                .entry(paths[change.sheet].clone())
                .or_default()
                .push(Command::SetSymbolField {
                    symbol_id: change.symbol,
                    key: change.key.clone(),
                    value: change.after.clone(),
                });
        }
        // An open typing run would fold the import into the last cell's step.
        self.end_parameter_edit_runs();
        if !by_sheet.is_empty() && !self.apply_parameter_commands(by_sheet) {
            return Task::none();
        }
        for change in plan.changes {
            self.ui_state.parameter_table.session.record(
                ParameterCell {
                    sheet: Some(paths[change.sheet].clone()),
                    symbol: change.symbol,
                    key: change.key,
                },
                change.before,
            );
        }
        self.close_parameter_import()
    }

    /// Whether the open preview goes into more than one sheet.
    pub(crate) fn is_parameter_import_across_sheets(&self) -> bool {
        (self.ui_state.parameter_import.as_ref()).is_some_and(|state| state.sheets.len() > 1)
    }

    /// Close the preview; nothing is applied.
    pub(crate) fn close_parameter_import(&mut self) -> Task<Message> {
        self.ui_state.parameter_import = None;
        self.close_detached_modal(ModalId::ParameterImport)
    }

    /// The preview's subtitle: the file and the sheet (or how many sheets)
    /// it goes into.
    pub(crate) fn parameter_import_subtitle(&self) -> Option<String> {
        let state = self.ui_state.parameter_import.as_ref()?;
        let file = ("file", signex_i18n::Arg::from(file_name(&state.file)));
        let i18n = &self.ui_state.i18n;
        Some(if self.is_parameter_import_across_sheets() {
            i18n.message_plain(
                ids::PARAMETER_IO_PREVIEW_SUBTITLE_ALL,
                &[file, ("sheets", signex_i18n::Arg::from(state.sheets.len()))],
            )
        } else {
            i18n.message_plain(
                ids::PARAMETER_IO_PREVIEW_SUBTITLE,
                &[
                    file,
                    ("sheet", signex_i18n::Arg::from(file_name(&state.sheet))),
                ],
            )
        })
    }
}
