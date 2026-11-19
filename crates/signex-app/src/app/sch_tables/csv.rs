//! CSV import and export of a typed table (#1300). The text format is
//! RFC 4180: comma separated, a field with a comma, quote or line break is
//! quoted and its quotes doubled. The first line holds the column titles.
//! The dialogs and the file IO run in a `Task`, never in `update`.

use std::path::PathBuf;

use iced::Task;
use signex_i18n::ids;
use signex_types::schematic::{SchDrawing, SchTable};

use super::TablesMsg;
use crate::app::{Message, Signex};

/// Largest CSV file an import reads (bytes).
const MAX_CSV_BYTES: u64 = 4 * 1024 * 1024;

/// The fields of `text`, row by row.
pub(crate) fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    let mut row_has_content = false;
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' => {
                quoted = true;
                row_has_content = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                row_has_content = true;
            }
            '\r' => {}
            '\n' => {
                if row_has_content || !field.is_empty() {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                row_has_content = false;
            }
            _ => {
                field.push(c);
                row_has_content = true;
            }
        }
    }
    if row_has_content || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// `table` as CSV: the titles of its shown columns, then its rows.
pub(crate) fn write_csv(table: &SchTable) -> String {
    let shown = table.shown_columns();
    let mut out = String::new();
    let line = |cells: Vec<String>| cells.join(",") + "\r\n";
    out.push_str(&line(
        shown
            .iter()
            .map(|&i| csv_field(&table.columns[i].title))
            .collect(),
    ));
    for r in 0..table.rows.len() {
        out.push_str(&line(
            shown.iter().map(|&i| csv_field(table.cell(r, i))).collect(),
        ));
    }
    out
}

/// The titles and rows an import gives, every row padded to the widest.
pub(crate) fn imported_table(text: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut rows = parse_csv(text);
    if rows.is_empty() {
        return Err("the file holds no rows".to_string());
    }
    let titles = rows.remove(0);
    let width = rows
        .iter()
        .map(Vec::len)
        .chain(std::iter::once(titles.len()))
        .max()
        .unwrap_or(0);
    let pad = |mut r: Vec<String>| {
        r.resize(width, String::new());
        r
    };
    Ok((pad(titles), rows.into_iter().map(pad).collect()))
}

fn read_csv(path: &std::path::Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_CSV_BYTES {
        return Err(format!("the file is larger than {MAX_CSV_BYTES} bytes"));
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    imported_table(&text)
}

impl Signex {
    /// The selected typed table on the active sheet, by id.
    fn typed_table(&self, id: uuid::Uuid) -> Option<SchTable> {
        let engine = self.document_state.active_engine()?;
        engine.document().drawings.iter().find_map(|d| match d {
            SchDrawing::Table(t) if t.uuid == id && !t.source.is_bound() => Some(t.clone()),
            _ => None,
        })
    }

    pub(super) fn csv_import_dialog(&mut self, table: uuid::Uuid) -> Task<Message> {
        if self.typed_table(table).is_none() {
            return Task::none();
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::TABLE_CSV_IMPORT_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_CSV_EXT, &["csv"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        Task::perform(
            async move {
                let path = dialog.pick_file().await?;
                let read = path.clone();
                let result = crate::app::path_op_jobs::run_blocking(move || read_csv(&read))
                    .await
                    .and_then(std::convert::identity);
                Some((path, result))
            },
            move |picked| Message::from(TablesMsg::CsvImportRead { table, picked }),
        )
    }

    pub(super) fn csv_import_read(
        &mut self,
        table: uuid::Uuid,
        picked: Option<CsvRead>,
    ) -> Task<Message> {
        let Some((path, result)) = picked else {
            return Task::none();
        };
        match result {
            Ok((titles, rows)) => {
                let edit = super::AnnotationEdit::TableImported(titles, rows);
                self.apply_annotation_edit_to(table, &edit);
            }
            Err(why) => crate::diagnostics::log_warning(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    ids::TABLE_CSV_IMPORT_FAILED,
                    "file" => path.display().to_string(),
                    "why" => why
                )
                .into_string(),
            ),
        }
        Task::none()
    }

    pub(super) fn csv_export_dialog(&mut self, table: uuid::Uuid) -> Task<Message> {
        let Some(found) = self.typed_table(table) else {
            return Task::none();
        };
        let text = write_csv(&found);
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::TABLE_CSV_EXPORT_TITLE)
            .set_file_name("table.csv")
            .filter_in(i18n, ids::FILE_FILTER_CSV_EXT, &["csv"]);
        Task::perform(
            async move {
                let path = dialog.save_file().await?;
                let target = path.clone();
                let result = crate::app::path_op_jobs::run_blocking(move || {
                    signex_types::atomic_io::atomic_write(&target, text.as_bytes())
                        .map_err(|e| e.to_string())
                })
                .await
                .and_then(std::convert::identity);
                Some((path, result))
            },
            |written| Message::from(TablesMsg::CsvExported(written)),
        )
    }

    pub(super) fn csv_exported(&mut self, written: Option<(PathBuf, Result<(), String>)>) {
        let Some((path, result)) = written else {
            return;
        };
        let i18n = &self.ui_state.i18n;
        match result {
            Ok(()) => crate::diagnostics::log_info(
                signex_i18n::t!(i18n, ids::TABLE_CSV_EXPORTED, "file" => path.display().to_string())
                    .into_string(),
            ),
            Err(why) => crate::diagnostics::log_warning(
                signex_i18n::t!(
                    i18n,
                    ids::TABLE_CSV_EXPORT_FAILED,
                    "file" => path.display().to_string(),
                    "why" => why
                )
                .into_string(),
            ),
        }
    }
}

/// What a CSV import read: the file, and its titles and rows or why not.
pub(crate) type CsvRead = (PathBuf, super::CsvCells);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_keep_commas_quotes_and_line_breaks() {
        let rows = parse_csv("a,\"b,c\",\"say \"\"hi\"\"\"\r\n\"two\nlines\",,x\n");
        assert_eq!(
            rows,
            vec![
                vec!["a".to_string(), "b,c".into(), "say \"hi\"".into()],
                vec!["two\nlines".to_string(), String::new(), "x".into()],
            ]
        );
    }

    #[test]
    fn blank_lines_and_a_byte_order_mark_are_skipped() {
        let rows = parse_csv("\u{feff}Addr,Register\n\n0x00,Config\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "Addr");
    }

    #[test]
    fn an_import_pads_short_rows_to_the_widest() {
        let (titles, rows) = imported_table("A,B,C\n1\n2,3\n").expect("rows");
        assert_eq!(titles, vec!["A", "B", "C"]);
        assert_eq!(rows[0], vec!["1", "", ""]);
        assert_eq!(rows[1], vec!["2", "3", ""]);
        assert!(imported_table("").is_err());
    }

    #[test]
    fn export_round_trips_through_import() {
        let mut t = SchTable::new_typed(uuid::Uuid::from_u128(1), Default::default());
        t.columns[0].title = "Addr".into();
        t.columns[1].title = "Note, with comma".into();
        t.columns[2].shown = false;
        t.set_cell(0, 0, "0x05".into());
        t.set_cell(0, 1, "say \"x\"".into());
        let (titles, rows) = imported_table(&write_csv(&t)).expect("rows");
        assert_eq!(titles, vec!["Addr", "Note, with comma"]);
        assert_eq!(rows[0], vec!["0x05", "say \"x\""]);
        assert_eq!(rows.len(), 3, "every row, empty ones included");
    }
}
