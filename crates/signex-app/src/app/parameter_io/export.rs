//! Export…: the parts the Parameter Manager lists written to a CSV file.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{ids, t};
use signex_output::parameter_csv::{self, ListedSheet};

use super::{ParameterIoMsg, file_name};
use crate::app::contracts::ParameterScope;
use crate::app::{Message, Signex};

impl Signex {
    /// The file Export… writes: the parts of the sheets the Parameter
    /// Manager lists (the active one, or every listed one with All sheets,
    /// then with a Sheet column), named as the table names them. `None`
    /// without an active schematic.
    pub(crate) fn listed_parameter_csv(&self) -> Option<Result<Vec<u8>, String>> {
        self.document_state.active_engine()?;
        let sheets: Vec<ListedSheet<'_>> = (self.parameter_documents().iter())
            .map(|sheet| sheet.listed())
            .collect();
        let with_sheet = self.ui_state.parameter_table.scope == ParameterScope::AllSheets;
        Some(parameter_csv::export(&sheets, with_sheet).map_err(|e| e.to_string()))
    }

    /// Ask where to save, then write the file off the update thread. The
    /// text is taken now, so edits made while the dialog is open are not in
    /// it.
    pub(super) fn export_parameters(&mut self) -> Task<Message> {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        let all_sheets = (self.ui_state.parameter_table.scope == ParameterScope::AllSheets)
            .then(|| self.parameter_sheets().len());
        // All sheets is named after the project, Active sheet after the sheet.
        let project = all_sheets
            .and_then(|_| self.document_state.document_project(&sheet))
            .map(|project| project.path.clone());
        let stem = (project.as_deref().unwrap_or(sheet.as_path()).file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let offered = format!("{stem}-parameters.csv");
        let bytes = match self.listed_parameter_csv() {
            Some(Ok(bytes)) => bytes,
            Some(Err(why)) => {
                self.log_export_failed(Path::new(&offered), why);
                return Task::none();
            }
            None => return Task::none(),
        };
        let i18n = &self.ui_state.i18n;
        let mut dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::PARAMETER_IO_EXPORT_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_CSV_EXT, &["csv"])
            .set_file_name(offered);
        if let Some(dir) = sheet.parent() {
            dialog = dialog.set_directory(dir);
        }
        Task::perform(
            async move {
                let path = dialog.save_file().await?;
                let target = path.clone();
                let result = crate::app::path_op_jobs::run_blocking(move || {
                    signex_types::atomic_io::atomic_write(&target, &bytes)
                        .map_err(|e| e.to_string())
                })
                .await
                .and_then(std::convert::identity);
                Some((path, result))
            },
            move |written| {
                ParameterIoMsg::Exported {
                    sheet,
                    all_sheets,
                    written,
                }
                .into()
            },
        )
    }

    /// Say in Messages where the file of `sheet` (or of `all_sheets`
    /// sheets) went, or why it was not written.
    pub(super) fn parameters_exported(
        &mut self,
        sheet: &Path,
        all_sheets: Option<usize>,
        written: Option<(PathBuf, Result<(), String>)>,
    ) {
        let Some((path, result)) = written else {
            return;
        };
        match result {
            Ok(()) => {
                let i18n = &self.ui_state.i18n;
                let file = path.display().to_string();
                let line = match all_sheets {
                    Some(count) => t!(
                        i18n,
                        ids::PARAMETER_IO_EXPORTED_ALL,
                        "sheets" => count,
                        "file" => file
                    ),
                    None => t!(
                        i18n,
                        ids::PARAMETER_IO_EXPORTED,
                        "sheet" => file_name(sheet),
                        "file" => file
                    ),
                };
                crate::diagnostics::log_info(line.into_string());
            }
            Err(why) => self.log_export_failed(&path, why),
        }
    }

    fn log_export_failed(&self, path: &Path, why: String) {
        let line = t!(
            self.ui_state.i18n,
            ids::PARAMETER_IO_EXPORT_FAILED,
            "file" => path.display().to_string(),
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }
}
