//! Export… and Import… of the Parameter Manager (#1295): the active sheet's
//! listed parts written to a CSV file, and a CSV file read back into a
//! preview whose Apply is one undo step. The format is
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
    /// not; `None` when it was cancelled.
    Exported {
        sheet: PathBuf,
        written: Option<(PathBuf, Result<(), String>)>,
    },
    /// Import… (the footer or the palette) was pressed.
    ImportRequested,
    /// The picked file of an import for `sheet` was read; `None` when the
    /// open dialog was cancelled.
    ImportRead {
        sheet: PathBuf,
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
            ParameterIoMsg::Exported { sheet, written } => {
                self.parameters_exported(&sheet, written);
                Task::none()
            }
            ParameterIoMsg::ImportRequested => self.import_parameters(),
            ParameterIoMsg::ImportRead { sheet, picked } => {
                self.parameter_import_read(sheet, picked)
            }
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
