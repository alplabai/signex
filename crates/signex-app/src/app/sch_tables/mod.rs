//! Schematic tables and the annotation drawings' edits (#1300, #1267):
//! bound-table refresh after a compile, the table of contents sheet, CSV
//! import and export, cell edits in place, and the Properties edits of a
//! Bezier, dimension or table. See docs/SCHEMATIC_TABLES.md.

mod annotation_edit;
mod csv;
mod curve_edit;
#[cfg(test)]
mod curve_edit_tests;
mod panel;
mod refresh;
mod rows;
mod toc;

use std::path::PathBuf;

use iced::Task;

pub use annotation_edit::{AnnotationEdit, Axis, TableSourceKind};
pub(crate) use annotation_edit::{
    apply_annotation_edit, table_padding_holds, table_text_size_holds,
};
pub(crate) use rows::{designators, source_is_missing};
#[cfg(test)]
mod pin_edit_golden_tests;
#[cfg(test)]
mod tests;

use crate::app::{Message, Signex};

impl From<TablesMsg> for Message {
    fn from(msg: TablesMsg) -> Self {
        Self::Edit(crate::app::EditMsg::Tables(msg))
    }
}

/// The column titles and rows a CSV import read, or why it could not.
pub type CsvCells = Result<(Vec<String>, Vec<Vec<String>>), String>;

/// What happened to a table: a request from a menu or the Properties
/// panel, or a file job that finished.
#[derive(Debug, Clone)]
pub enum TablesMsg {
    /// Project ▸ Add Table of Contents Sheet was chosen.
    TableOfContentsRequested,
    /// The contents sheet's write finished (`Err("exists")`: a file was
    /// already there and was kept).
    TableOfContentsWritten {
        project_id: crate::app::state::ProjectId,
        path: PathBuf,
        result: Result<(), String>,
    },
    /// Import… was pressed for a typed table.
    CsvImportRequested(uuid::Uuid),
    /// The picked CSV file was read (`None`: the dialog was cancelled).
    /// `sheet` is the sheet the table was on when Import… was pressed: the
    /// import lands there even if another tab is active by now.
    CsvImportRead {
        sheet: PathBuf,
        table: uuid::Uuid,
        picked: Option<(PathBuf, CsvCells)>,
    },
    /// Export… was pressed for a typed table.
    CsvExportRequested(uuid::Uuid),
    /// The CSV file was written (`None`: the dialog was cancelled).
    CsvExported(Option<(PathBuf, Result<(), String>)>),
    /// A cell of a typed table was edited in place on the sheet.
    CellEdited {
        table: uuid::Uuid,
        row: usize,
        column: usize,
        text: String,
    },
}

impl Signex {
    pub(crate) fn dispatch_tables(&mut self, msg: TablesMsg) -> Task<Message> {
        let task = match msg {
            TablesMsg::TableOfContentsRequested => self.add_table_of_contents(),
            TablesMsg::TableOfContentsWritten {
                project_id,
                path,
                result,
            } => self.table_of_contents_written(project_id, path, result),
            TablesMsg::CsvImportRequested(table) => self.csv_import_dialog(table),
            TablesMsg::CsvImportRead {
                sheet,
                table,
                picked,
            } => self.csv_import_read(&sheet, table, picked),
            TablesMsg::CsvExportRequested(table) => self.csv_export_dialog(table),
            TablesMsg::CsvExported(written) => {
                self.csv_exported(written);
                Task::none()
            }
            TablesMsg::CellEdited {
                table,
                row,
                column,
                text,
            } => {
                self.apply_annotation_edit_to(table, &AnnotationEdit::TableCell(row, column, text));
                Task::none()
            }
        };
        Task::batch([task, self.finish_update()])
    }

    /// Apply `edit` to drawing `id` of the open sheet at `sheet` as one undo
    /// step (through the gateway when it is the sheet in view). `None` when
    /// the sheet is not open or holds no such drawing; `Some(false)` when
    /// the edit does not apply or changes nothing.
    pub(crate) fn apply_annotation_edit_on(
        &mut self,
        sheet: &std::path::Path,
        id: uuid::Uuid,
        edit: &AnnotationEdit,
    ) -> Option<bool> {
        let engine = self.document_state.engines.get(sheet)?;
        let drawing = engine.document().drawings.iter().find(|d| d.uuid() == id)?;
        let Some(next) = apply_annotation_edit(drawing, edit) else {
            return Some(false);
        };
        let command = signex_engine::Command::UpdateSchDrawing { drawing: next };
        if self.document_state.active_path.as_deref() == Some(sheet) {
            return Some(self.apply_engine_command(command, true, true));
        }
        // A refusal is said in Messages by `execute_on_sheet`.
        Some(self.execute_on_sheet(sheet, command).unwrap_or(false))
    }

    /// Apply `edit` to drawing `id` of the active sheet as one undo step;
    /// `false` when it does not apply or changes nothing.
    pub(crate) fn apply_annotation_edit_to(
        &mut self,
        id: uuid::Uuid,
        edit: &AnnotationEdit,
    ) -> bool {
        let Some(engine) = self.document_state.active_engine() else {
            return false;
        };
        let Some(next) = engine
            .document()
            .drawings
            .iter()
            .find(|d| d.uuid() == id)
            .and_then(|d| apply_annotation_edit(d, edit))
        else {
            return false;
        };
        self.apply_engine_command(
            signex_engine::Command::UpdateSchDrawing { drawing: next },
            true,
            true,
        )
    }
}
