//! Import…: a CSV file read off the update thread into the Import
//! Parameters preview, whose Apply writes every change as one undo step.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::Command;
use signex_i18n::{I18n, ids, t};
use signex_output::parameter_csv::{self, ImportPlan, ParameterCsv, ParameterCsvError};

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
    /// The sheet the file goes into: the active one when Import… was
    /// pressed.
    pub sheet: PathBuf,
    /// The file read.
    pub file: PathBuf,
    pub csv: ParameterCsv,
}

impl Signex {
    /// Ask for a CSV file and read it off the update thread, for the active
    /// sheet.
    pub(super) fn import_parameters(&mut self) -> Task<Message> {
        if self.document_state.active_engine().is_none() {
            return Task::none();
        }
        let Some(sheet) = self.document_state.active_path.clone() else {
            return Task::none();
        };
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
            move |picked| ParameterIoMsg::ImportRead { sheet, picked }.into(),
        )
    }

    /// The file was read: open the preview for `sheet`, or say in Messages
    /// why the file was refused.
    pub(super) fn parameter_import_read(
        &mut self,
        sheet: PathBuf,
        picked: Option<ImportRead>,
    ) -> Task<Message> {
        let Some((file, result)) = picked else {
            return Task::none();
        };
        match result {
            Ok(csv) => {
                self.ui_state.parameter_import =
                    Some(Box::new(ParameterImportState { sheet, file, csv }));
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
    /// sheet as it is. `None` with no preview, or when the sheet is closed.
    pub(crate) fn parameter_import_plan(&self) -> Option<ImportPlan> {
        let state = self.ui_state.parameter_import.as_ref()?;
        let engine = self.document_state.engines.get(&state.sheet)?;
        let parts = parameter_csv::listed_parts(engine.document());
        Some(parameter_csv::plan_import(&parts, &state.csv))
    }

    /// Apply: every change in one engine batch through the edit gateway, so
    /// one Undo takes the import back. The preview stays open when the sheet
    /// is not the active one or the edit was refused.
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
        let Some(plan) = self.parameter_import_plan() else {
            return Task::none();
        };
        let commands: Vec<Command> = plan
            .changes
            .iter()
            .map(|change| Command::SetSymbolField {
                symbol_id: change.symbol,
                key: change.key.clone(),
                value: change.after.clone(),
            })
            .collect();
        // An open typing run would fold the import into the last cell's step.
        self.end_parameter_edit_runs();
        if !commands.is_empty() && !self.apply_engine_commands(commands, false, false) {
            return Task::none();
        }
        for change in plan.changes {
            self.ui_state.parameter_table.session.record(
                ParameterCell {
                    sheet: Some(sheet.clone()),
                    symbol: change.symbol,
                    key: change.key,
                },
                change.before,
            );
        }
        self.close_parameter_import()
    }

    /// Close the preview; nothing is applied.
    pub(crate) fn close_parameter_import(&mut self) -> Task<Message> {
        self.ui_state.parameter_import = None;
        self.close_detached_modal(ModalId::ParameterImport)
    }

    /// The preview's subtitle: the file and the sheet it goes into.
    pub(crate) fn parameter_import_subtitle(&self) -> Option<String> {
        let state = self.ui_state.parameter_import.as_ref()?;
        Some(self.ui_state.i18n.message_plain(
            ids::PARAMETER_IO_PREVIEW_SUBTITLE,
            &[
                ("file", signex_i18n::Arg::from(file_name(&state.file))),
                ("sheet", signex_i18n::Arg::from(file_name(&state.sheet))),
            ],
        ))
    }
}
