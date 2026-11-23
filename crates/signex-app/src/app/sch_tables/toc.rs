//! Project ▸ Add Table of Contents Sheet (#1300): writes `contents.snxsch`
//! next to the root sheet, with a heading (`<project> · Contents`) and a
//! table bound to the project's sheets, then adds it to the project like
//! Add Existing does. The write is atomic, runs in a `Task`, and refuses a
//! file that is already there. See docs/SCHEMATIC_TABLES.md.

use std::path::PathBuf;

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::schematic::{
    HAlign, Point, SchDrawing, SchTable, SchematicSheet, TableSource, TextNote, VAlign,
};

use super::TablesMsg;
use crate::app::{Message, ProjectMsg, Signex};

/// The contents sheet's file name.
pub(crate) const CONTENTS_FILE: &str = "contents.snxsch";

/// The contents sheet's paper, the size a new sheet starts on.
const CONTENTS_PAPER: &str = "A4";
/// Where the heading and the table sit on the contents sheet (mm).
const HEADING_AT: Point = Point::new(25.4, 25.4);
const TABLE_AT: Point = Point::new(25.4, 35.56);
/// The heading's text height (mm).
const HEADING_MM: f64 = 3.81;

/// The contents sheet of project `name`: its heading (`<name> · <word>`,
/// `word` the catalog's "Contents") and a contents table with `rows` and
/// `links` (empty until the next compile fills them).
pub(crate) fn contents_sheet(
    name: &str,
    word: &str,
    rows: Vec<Vec<String>>,
    links: Vec<String>,
) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: CONTENTS_PAPER.to_string(),
        ..SchematicSheet::default()
    };
    sheet.text_notes.push(TextNote {
        uuid: uuid::Uuid::now_v7(),
        text: format!("{name} · {word}"),
        position: HEADING_AT,
        rotation: 0.0,
        font_size: HEADING_MM,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
    });
    let mut table = SchTable::new_typed(uuid::Uuid::now_v7(), TABLE_AT);
    table.source = TableSource::ProjectSheets { per_channel: true };
    table.columns = SchTable::columns_for(&table.source);
    table.rows = rows;
    table.links = links;
    sheet.drawings.push(SchDrawing::Table(table));
    sheet
}

/// Write `text` to `path` unless a file is there already. The name is
/// claimed with `create_new` first, so a file made there after the check is
/// never overwritten; the text then replaces only that empty claim, through
/// the atomic write, and a failed write gives the name back.
fn write_new(path: &std::path::Path, text: &str) -> Result<(), String> {
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(claim) => drop(claim),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err("exists".to_string());
        }
        Err(e) => return Err(e.to_string()),
    }
    signex_types::atomic_io::atomic_write(path, text.as_bytes()).map_err(|e| {
        // Only the empty claim made above is removed; nothing else was there.
        if let Err(cleanup) = std::fs::remove_file(path) {
            tracing::warn!(path = %path.display(), error = %cleanup, "could not remove the empty contents file");
        }
        e.to_string()
    })
}

impl Signex {
    pub(super) fn add_table_of_contents(&mut self) -> Task<Message> {
        let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
        let Some(project) = self.document_state.active_document_project() else {
            crate::diagnostics::log_warning(t!(i18n, ids::TABLE_TOC_NO_PROJECT).into_string());
            return Task::none();
        };
        let project_id = project.id;
        let name = project.data.name.clone();
        let dir = crate::app::project_sheets::project_root_sheet_path(project)
            .and_then(|root| root.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| project.dir().to_path_buf());
        let path = dir.join(CONTENTS_FILE);
        let (rows, links) = match self.contents_now(project_id, &dir) {
            Some(entries) => {
                let columns =
                    SchTable::columns_for(&TableSource::ProjectSheets { per_channel: true });
                super::rows::contents_table_rows(&entries, &columns)
            }
            None => (Vec::new(), Vec::new()),
        };
        let word = t!(i18n, ids::TABLE_TOC_HEADING).into_string();
        let sheet = contents_sheet(&name, &word, rows, links);
        let text = match signex_types::format::SnxSchematic::new(sheet).write_string() {
            Ok(text) => text,
            Err(error) => {
                crate::diagnostics::log_error(
                    &t!(i18n, ids::TABLE_TOC_NOT_SERIALISED).into_string(),
                    &anyhow::Error::new(error),
                );
                return Task::none();
            }
        };
        let target = path.clone();
        Task::perform(
            async move {
                crate::app::path_op_jobs::run_blocking(move || write_new(&target, &text))
                    .await
                    .and_then(std::convert::identity)
            },
            move |result| {
                Message::from(TablesMsg::TableOfContentsWritten {
                    project_id,
                    path: path.clone(),
                    result,
                })
            },
        )
    }

    pub(super) fn table_of_contents_written(
        &mut self,
        project_id: crate::app::state::ProjectId,
        path: PathBuf,
        result: Result<(), String>,
    ) -> Task<Message> {
        let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
        let file = path.display().to_string();
        match result {
            Ok(()) => {
                crate::diagnostics::log_info(
                    t!(i18n, ids::TABLE_TOC_WRITTEN, "file" => file).into_string(),
                );
                self.update(Message::Project(ProjectMsg::AddExistingFilePicked {
                    project_id,
                    paths: Some(vec![path]),
                }))
            }
            Err(why) if why == "exists" => {
                crate::diagnostics::log_warning(
                    t!(i18n, ids::TABLE_TOC_EXISTS, "file" => file).into_string(),
                );
                Task::none()
            }
            Err(why) => {
                crate::diagnostics::log_warning(
                    t!(i18n, ids::TABLE_TOC_FAILED, "file" => file, "why" => why).into_string(),
                );
                Task::none()
            }
        }
    }

    /// The contents rows `project`'s last compile gives, one per channel,
    /// their links relative to `dir`, where the contents sheet goes.
    fn contents_now(
        &self,
        project: crate::app::state::ProjectId,
        dir: &std::path::Path,
    ) -> Option<super::refresh::Contents> {
        self.compiled_contents_of(project, Some(dir))
            .map(|pages| pages.pages(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_contents_sheet_has_a_heading_and_a_bound_contents_table() {
        let word = t!(signex_i18n::I18n::en_us_ref(), ids::TABLE_TOC_HEADING).into_string();
        let sheet = contents_sheet("dcdc-buck", &word, Vec::new(), Vec::new());
        assert_eq!(sheet.text_notes[0].text, "dcdc-buck · Contents");
        let [SchDrawing::Table(t)] = &sheet.drawings[..] else {
            panic!("one table");
        };
        assert_eq!(t.source, TableSource::ProjectSheets { per_channel: true });
        assert_eq!(
            t.shown_columns()
                .iter()
                .map(|&i| t.columns[i].title.as_str())
                .collect::<Vec<_>>(),
            vec!["Page", "Sheet", "Title", "Rev"]
        );
    }

    /// Mockup row 156: the generated sheet loads with no paper size warning.
    #[test]
    fn the_contents_sheet_has_a_paper_size_this_build_recognises() {
        let sheet = contents_sheet("dcdc-buck", "Contents", Vec::new(), Vec::new());
        let (size, _) = signex_types::schematic::SheetSize::parse(&sheet.paper_size);
        assert!(size.is_recognized(), "{:?}", sheet.paper_size);
    }

    #[test]
    fn an_existing_contents_file_is_refused_and_kept() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join(CONTENTS_FILE);
        std::fs::write(&path, "mine").expect("seed");
        assert_eq!(write_new(&path, "theirs"), Err("exists".to_string()));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "mine");
        let fresh = dir.path().join("other.snxsch");
        assert_eq!(write_new(&fresh, "new"), Ok(()));
    }
}
