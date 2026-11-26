//! Project ▸ Add Table of Contents Sheet (#1300): writes `contents.snxsch`
//! next to the root sheet, with a heading (`<project> · Contents`) and a
//! table bound to the project's sheets, then adds it to the project like
//! Add Existing does. The table starts with the last compile's pages, or
//! before any compile with the pages the project lists (mockup row 156).
//! The write is atomic, runs in a `Task`, and refuses a file that is already
//! there. See docs/SCHEMATIC_TABLES.md.

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
/// `links`.
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

/// The rows the contents table starts with.
#[derive(Debug)]
enum StartRows {
    /// The last compile's pages, one per channel.
    Compiled(Vec<super::rows::ContentsEntry>),
    /// Before any compile: the pages the project lists, with the sheet
    /// numbers the Navigator shows for them. The title blocks of
    /// the sheets that are not open are read with the write, off the update
    /// thread.
    Declared(Vec<super::rows::DeclaredPage>),
}

/// The text of project `name`'s contents sheet, its table filled from
/// `start`; `read` gives a closed sheet's title block.
fn contents_text(
    name: &str,
    word: &str,
    start: StartRows,
    read: &dyn Fn(&std::path::Path) -> Option<signex_net::SheetTitle>,
) -> Result<String, String> {
    let entries = match start {
        StartRows::Compiled(entries) => entries,
        StartRows::Declared(pages) => super::rows::declared_contents(&pages, read),
    };
    let columns = SchTable::columns_for(&TableSource::ProjectSheets { per_channel: true });
    let (rows, links) = super::rows::contents_table_rows(&entries, &columns);
    signex_types::format::SnxSchematic::new(contents_sheet(name, word, rows, links))
        .write_string()
        .map_err(|e| e.to_string())
}

/// The title block of the sheet file at `path`, read from disk.
fn title_on_disk(path: &std::path::Path) -> Option<signex_net::SheetTitle> {
    crate::app::project_sheets::load_sheet_file(path)
        .ok()
        .flatten()
        .map(|sheet| signex_net::SheetTitle::of(&sheet))
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
        let start = self.start_rows(project_id, &dir);
        let word = t!(i18n, ids::TABLE_TOC_HEADING).into_string();
        let target = path.clone();
        Task::perform(
            async move {
                crate::app::path_op_jobs::run_blocking(move || {
                    let text = contents_text(&name, &word, start, &title_on_disk)?;
                    write_new(&target, &text)
                })
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

    /// The rows `project`'s contents sheet in `dir` starts with: the last
    /// compile's pages, or before any compile the pages the project lists.
    fn start_rows(
        &self,
        project: crate::app::state::ProjectId,
        dir: &std::path::Path,
    ) -> StartRows {
        match self.contents_now(project, dir) {
            Some(entries) => StartRows::Compiled(entries),
            None => StartRows::Declared(self.declared_pages(project, dir)),
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

    /// The pages `project` lists, in its order, each linked from `dir`, with
    /// the title block of its open tab when it has one and its page number
    /// when the Navigator shows this project.
    fn declared_pages(
        &self,
        project: crate::app::state::ProjectId,
        dir: &std::path::Path,
    ) -> Vec<super::rows::DeclaredPage> {
        let Some(loaded) = self.document_state.project_by_id(project) else {
            return Vec::new();
        };
        let fold = crate::app::state::project_case::fold_of(Some(loaded));
        let root = crate::app::project_sheets::project_root_sheet_path(loaded);
        let project_dir = loaded.dir();
        loaded
            .data
            .sheets
            .iter()
            .map(|entry| {
                let path = project_dir.join(&entry.filename);
                super::rows::DeclaredPage {
                    name: entry.name.clone(),
                    file: crate::app::handlers::canvas::link_from(dir, &path, fold),
                    title: self
                        .document_state
                        .engines
                        .get(&path)
                        .map(|engine| signex_net::SheetTitle::of(engine.document())),
                    number: self.navigator_number(root.as_deref(), &path, fold),
                    path,
                }
            })
            .collect()
    }

    /// The page number of the sheet at `path` in the sheet numbering the
    /// Navigator shows (its first placement's), when the Navigator shows the
    /// project whose root sheet is `root`.
    fn navigator_number(
        &self,
        root: Option<&std::path::Path>,
        path: &std::path::Path,
        fold: crate::app::state::scope::CaseFold,
    ) -> Option<usize> {
        let navigator = &self.document_state.panel_ctx.navigator;
        let shown_root = navigator.hierarchy.as_ref()?.root.as_ref()?;
        if !shown_root.is_at(root?, fold) {
            return None;
        }
        let key = crate::app::sheet_hierarchy::normalised_key(path, fold);
        navigator
            .numbers
            .iter()
            .filter(|((sheet, _), _)| *sheet == key)
            .map(|(_, &number)| number)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use signex_net::SheetTitle;

    use super::super::rows::{ContentsEntry, DeclaredPage};
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

    fn title(title: &str, revision: &str) -> SheetTitle {
        SheetTitle {
            title: title.to_string(),
            revision: revision.to_string(),
            date: String::new(),
        }
    }

    /// The table of the written contents text, as the sheet loads it.
    fn written_table(start: StartRows, read: &dyn Fn(&Path) -> Option<SheetTitle>) -> SchTable {
        let text = contents_text("dcdc-buck", "Contents", start, read).expect("text");
        let sheet = signex_types::format::SnxSchematic::parse(&text)
            .expect("parse")
            .sheet;
        let [SchDrawing::Table(t)] = &sheet.drawings[..] else {
            panic!("one table");
        };
        t.clone()
    }

    /// The drawn cells, row by row, in the shown columns.
    fn drawn_cells(t: &SchTable) -> Vec<Vec<String>> {
        let shown = t.shown_columns();
        (0..t.layout().body_rows)
            .map(|r| shown.iter().map(|&c| t.cell(r, c).to_string()).collect())
            .collect()
    }

    /// Mockup row 156: a contents sheet added before any compile draws a row
    /// per page under its header, with each page's title block.
    #[test]
    fn before_a_compile_the_contents_table_draws_the_projects_pages() {
        let page = |name: &str, title: Option<SheetTitle>| DeclaredPage {
            name: name.to_string(),
            path: PathBuf::from(format!("/p/{name}.snxsch")),
            file: format!("{name}.snxsch"),
            title,
            number: None,
        };
        let start = StartRows::Declared(vec![
            page("top", Some(title("Top level", "B"))),
            page("power_in", None),
            page("lost", None),
        ]);
        let read = |path: &Path| {
            (path == Path::new("/p/power_in.snxsch")).then(|| title("USB-C input", "B"))
        };

        let t = written_table(start, &read);

        assert_eq!(t.layout().body_rows, 3, "one drawn row per page");
        assert_eq!(
            drawn_cells(&t),
            [
                ["1", "top", "Top level", "B"],
                ["2", "power_in", "USB-C input", "B"],
                ["3", "lost", "", ""],
            ]
        );
        assert_eq!(t.links, ["top.snxsch", "power_in.snxsch", "lost.snxsch"]);
    }

    /// After a compile the table starts with the compiled pages; no sheet
    /// file is read.
    #[test]
    fn after_a_compile_the_contents_table_starts_with_the_compiled_pages() {
        let start = StartRows::Compiled(vec![ContentsEntry {
            page: 1,
            sheet: "buck_channel · CH1".to_string(),
            title: "3.3 V buck, channel 1".to_string(),
            revision: "B".to_string(),
            last_changed: String::new(),
            file: "buck_channel.snxsch".to_string(),
        }]);
        let read = |_: &Path| -> Option<SheetTitle> { panic!("a compiled page is not read") };

        let t = written_table(start, &read);

        assert_eq!(
            drawn_cells(&t),
            [["1", "buck_channel · CH1", "3.3 V buck, channel 1", "B"]]
        );
        assert_eq!(t.links, ["buck_channel.snxsch"]);
    }

    /// The declared pages follow the project's order; an open sheet gives
    /// its tab's title block, a closed one is left for the write to read.
    #[test]
    fn the_declared_pages_take_an_open_sheets_title_block() {
        use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
        let mut app = app_workspace("/w/toc-declared", &["top.snxsch", "power.snxsch"]);
        let top = PathBuf::from("/w/toc-declared/top.snxsch");
        let mut sheet = SchematicSheet::default();
        sheet
            .title_block
            .insert("title".to_string(), "Top level".to_string());
        open_with(&mut app.document_state, &top, sheet);
        let project = app.document_state.projects[0].id;

        let pages = app.declared_pages(project, Path::new("/w/toc-declared"));

        assert_eq!(
            pages
                .iter()
                .map(|p| (p.file.as_str(), p.title.as_ref().map(|t| t.title.as_str())))
                .collect::<Vec<_>>(),
            [("top.snxsch", Some("Top level")), ("power.snxsch", None)]
        );
        assert_eq!(pages[1].path, PathBuf::from("/w/toc-declared/power.snxsch"));
    }

    /// Before a compile the rows carry the project's sheet numbers: the
    /// numbered pages by number, the rest after them in the project's order.
    #[test]
    fn the_declared_rows_follow_the_projects_sheet_numbers() {
        let page = |name: &str, number: Option<usize>| DeclaredPage {
            name: name.to_string(),
            path: PathBuf::from(format!("/p/{name}.snxsch")),
            file: format!("{name}.snxsch"),
            title: Some(SheetTitle::default()),
            number,
        };
        let start = StartRows::Declared(vec![
            page("notes", None),
            page("power", Some(3)),
            page("top", Some(1)),
            page("spare", None),
        ]);

        let t = written_table(start, &|_: &Path| -> Option<SheetTitle> { None });

        let pages: Vec<Vec<String>> = drawn_cells(&t)
            .into_iter()
            .map(|row| row[..2].to_vec())
            .collect();
        assert_eq!(
            pages,
            [["1", "top"], ["3", "power"], ["4", "notes"], ["5", "spare"]]
        );
    }

    /// The sheet the Navigator shows for `root`, with the page numbers of
    /// `numbers` (one per placement).
    fn navigator_showing(app: &mut Signex, root: &str, numbers: &[(&str, u128, usize)]) {
        use crate::app::sheet_hierarchy::{
            SheetHierarchy, SheetNode, SheetNodeKind, normalised_key,
        };
        use signex_types::schematic::InstancePath;
        let fold = crate::app::state::project_case::fold_of(app.document_state.projects.first());
        let navigator = &mut app.document_state.panel_ctx.navigator;
        navigator.hierarchy = Some(SheetHierarchy {
            fold,
            root: Some(SheetNode {
                name: "top".to_string(),
                filename: "top.snxsch".to_string(),
                placed_as: None,
                path: PathBuf::from(root),
                kind: SheetNodeKind::Present,
                detail: None,
                children: Vec::new(),
            }),
            loose: Vec::new(),
        });
        navigator.numbers = numbers
            .iter()
            .map(|&(path, step, number)| {
                let at = InstancePath::from_chain(vec![uuid::Uuid::from_u128(step)]);
                ((normalised_key(Path::new(path), fold), at), number)
            })
            .collect();
    }

    /// The declared pages take the Navigator's sheet numbers (a sheet placed
    /// twice takes its first page), and none from another project's.
    #[test]
    fn the_declared_pages_take_the_navigators_page_numbers() {
        use crate::app::handlers::menu::export::tests::app_workspace;
        let mut app = app_workspace(
            "/w/toc-numbers",
            &["top.snxsch", "reg.snxsch", "notes.snxsch"],
        );
        let project = app.document_state.projects[0].id;
        let numbers = [
            ("/w/toc-numbers/top.snxsch", 0, 1),
            ("/w/toc-numbers/reg.snxsch", 1, 3),
            ("/w/toc-numbers/reg.snxsch", 2, 2),
        ];
        let numbers_of = |app: &Signex| {
            app.declared_pages(project, Path::new("/w/toc-numbers"))
                .iter()
                .map(|p| p.number)
                .collect::<Vec<_>>()
        };

        navigator_showing(&mut app, "/w/toc-numbers/top.snxsch", &numbers);
        assert_eq!(numbers_of(&app), [Some(1), Some(2), None]);

        navigator_showing(&mut app, "/w/other/top.snxsch", &numbers);
        assert_eq!(numbers_of(&app), [None, None, None]);
    }

    /// Mockup row 156: with no compile the sheet starts with the listed
    /// pages, with one it starts with the compiled pages.
    #[test]
    fn the_contents_sheet_starts_from_the_compile_only_when_there_is_one() {
        use crate::app::handlers::menu::export::tests::app_workspace;
        let mut app = app_workspace("/w/toc-start", &["top.snxsch", "power.snxsch"]);
        let project = app.document_state.projects[0].id;
        let dir = Path::new("/w/toc-start");

        let StartRows::Declared(pages) = app.start_rows(project, dir) else {
            panic!("no compile yet: the listed pages");
        };
        assert_eq!(
            pages.iter().map(|p| p.file.as_str()).collect::<Vec<_>>(),
            ["top.snxsch", "power.snxsch"]
        );

        app.document_state.compiled_design.results.insert(
            project,
            crate::app::core::compiled_design::CompiledResult {
                design: std::sync::Arc::new(signex_net::CompiledDesign::default()),
                paths: Default::default(),
                compiled_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
                miniatures: Default::default(),
                stale: Default::default(),
                has_unread_sheets: false,
                unread_paths: Vec::new(),
                bare_links_missing: Vec::new(),
            },
        );
        assert!(
            matches!(app.start_rows(project, dir), StartRows::Compiled(_)),
            "after a compile: the compiled pages"
        );
    }
}
