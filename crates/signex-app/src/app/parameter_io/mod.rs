//! Export… and Import… of the Parameter Manager (#1295): the listed parts of
//! the sheets the Manager lists (its Active sheet / All sheets scope, #1784)
//! written to a CSV file, and a CSV file read back into a preview whose
//! Apply is one undo step across those sheets. The format is
//! `signex_output::parameter_csv`; the file dialogs, the read and the write
//! run in a `Task`. Rules: docs/PARAMETER_CSV.md.

mod export;
mod import;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use iced::Task;
use signex_output::parameter_csv::ParameterCsv;

pub(crate) use import::{ImportReadError, ParameterImportState};

use crate::app::{Message, ParameterManagerMsg, Signex};

impl From<ParameterIoMsg> for Message {
    fn from(msg: ParameterIoMsg) -> Self {
        Self::ParameterManager(ParameterManagerMsg::Io(msg))
    }
}

/// What a CSV file read gives: the file, and its table or why not.
pub type ImportRead = (PathBuf, Result<ParameterCsv, ImportReadError>);

/// What happened in the Parameter Manager's CSV export and import.
#[derive(Debug, Clone)]
pub enum ParameterIoMsg {
    /// Export… (the footer or the palette) was pressed.
    ExportRequested,
    /// The save dialog of `sheet`'s export closed: the file written, or why
    /// not; `None` when it was cancelled. `all_sheets` counts the sheets an
    /// All sheets export listed; `None` for Active sheet.
    Exported {
        sheet: PathBuf,
        all_sheets: Option<usize>,
        written: Option<(PathBuf, Result<(), String>)>,
    },
    /// Import… (the footer or the palette) was pressed.
    ImportRequested,
    /// The picked file of an import pressed on `sheet` was read, for the
    /// `sheets` the Manager listed then; `None` when the open dialog was
    /// cancelled.
    ImportRead {
        sheet: PathBuf,
        sheets: Vec<PathBuf>,
        picked: Option<ImportRead>,
    },
    /// Apply in the preview.
    PreviewAccepted,
    /// Cancel, Esc or the close button of the preview.
    PreviewCancelled,
}

impl Signex {
    pub(crate) fn dispatch_parameter_io(&mut self, msg: ParameterIoMsg) -> Task<Message> {
        match msg {
            ParameterIoMsg::ExportRequested => self.export_parameters(),
            ParameterIoMsg::Exported {
                sheet,
                all_sheets,
                written,
            } => {
                self.parameters_exported(&sheet, all_sheets, written);
                Task::none()
            }
            ParameterIoMsg::ImportRequested => self.import_parameters(),
            ParameterIoMsg::ImportRead {
                sheet,
                sheets,
                picked,
            } => self.parameter_import_read(sheet, sheets, picked),
            ParameterIoMsg::PreviewAccepted => self.accept_parameter_import(),
            ParameterIoMsg::PreviewCancelled => self.close_parameter_import(),
        }
    }
}

/// The file name of `path` for a Messages line or a subtitle.
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
