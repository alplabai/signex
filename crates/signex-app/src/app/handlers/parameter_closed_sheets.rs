//! The closed project sheets the Parameter Manager lists with All sheets
//! (Caner 2026-11-25). They are read from disk in a task, listed and
//! exported read-only, and imported against like the open ones. An edit that
//! reaches one opens it first: a press on one of its cells opens it, and an
//! edit across sheets (a column, Apply of an import) that would change one
//! changes nothing and says which sheets to open. Nothing is written to a
//! closed sheet's file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::schematic::SchematicSheet;

use crate::app::contracts::ParameterScope;
use crate::app::handlers::canvas::SheetOpen;
use crate::app::state::Signex;
use crate::app::{Message, ParameterManagerMsg};

impl Signex {
    /// Whether `page` is open, under its own spelling or the one its tab
    /// holds.
    fn is_parameter_page_open(&self, page: &Path) -> bool {
        let engines = &self.document_state.engines;
        engines.contains_key(page)
            || self
                .held_spelling(page)
                .is_some_and(|held| engines.contains_key(&held))
    }

    /// The pages of the active sheet's project no open document holds.
    pub(crate) fn closed_project_pages(&self) -> Vec<PathBuf> {
        let docs = &self.document_state;
        let Some(active) = docs.active_path.as_deref() else {
            return Vec::new();
        };
        crate::app::project_sheets::project_pages(docs.document_project(active))
            .into_iter()
            .filter(|page| !self.is_parameter_page_open(page))
            .collect()
    }

    /// Read the closed sheets of the active sheet's project on a worker, for
    /// All sheets. An older read still on its way is dropped when this one
    /// starts.
    pub(crate) fn read_closed_parameter_sheets(&mut self) -> Task<Message> {
        let pages = self.closed_project_pages();
        let closed = &mut self.ui_state.parameter_table.closed;
        closed.generation += 1;
        closed.sheets = Arc::default();
        closed.pages = pages.clone();
        closed.reading = !pages.is_empty();
        if pages.is_empty() {
            return Task::none();
        }
        let generation = closed.generation;
        Task::perform(read_on_worker(pages, read_sheets), move |read| {
            Message::ParameterManager(ParameterManagerMsg::ClosedSheetsRead {
                generation,
                sheets: Arc::new(read.sheets),
                unread: read.unread,
            })
        })
    }

    /// With All sheets shown, keep the closed copies in step with the tabs
    /// and the active project: a copy whose sheet opened goes; a project
    /// sheet that closed, or another project (or none) in the main window,
    /// reads the closed sheets again, since the disk may hold a newer save.
    pub(crate) fn sync_closed_parameter_sheets(&mut self) -> Task<Message> {
        let table = &self.ui_state.parameter_table;
        if !self.ui_state.parameter_manager_open || table.scope != ParameterScope::AllSheets {
            return Task::none();
        }
        let pages = self.closed_project_pages();
        let held = &table.closed.pages;
        if pages == *held {
            return Task::none();
        }
        // No closed page left (all open, or no project): this also ends a
        // read still on its way.
        if pages.is_empty() || !pages.iter().all(|page| held.contains(page)) {
            return self.read_closed_parameter_sheets();
        }
        let closed = &mut self.ui_state.parameter_table.closed;
        let kept: Vec<_> = (closed.sheets.iter())
            .filter(|(path, _)| pages.contains(path))
            .cloned()
            .collect();
        closed.sheets = Arc::new(kept);
        closed.pages = pages;
        Task::none()
    }

    /// Keep a finished read of the closed sheets, unless a newer read
    /// started or All sheets was left meanwhile. Messages names each sheet
    /// the read could not bring in, and why.
    pub(crate) fn handle_closed_parameter_sheets_read(
        &mut self,
        generation: u64,
        sheets: crate::app::state::ClosedSheetCopies,
        unread: Vec<(PathBuf, String)>,
    ) {
        let table = &mut self.ui_state.parameter_table;
        if table.closed.generation != generation || table.scope != ParameterScope::AllSheets {
            return;
        }
        table.closed.reading = false;
        table.closed.sheets = sheets;
        for (path, reason) in unread {
            let line = t!(
                self.ui_state.i18n,
                ids::DIALOG_PARAMETER_MANAGER_CLOSED_UNREAD,
                "sheet" => crate::app::view::parameter_manager::sheet_file_name(&path),
                "reason" => reason
            );
            crate::diagnostics::log_warning(line.into_string());
        }
    }

    /// The closed sheets read for All sheets that are still closed pages of
    /// the active sheet's project, by path.
    pub(crate) fn listed_closed_sheets(&self) -> Vec<(&Path, &SchematicSheet)> {
        let table = &self.ui_state.parameter_table;
        if table.scope != ParameterScope::AllSheets {
            return Vec::new();
        }
        let pages = self.closed_project_pages();
        let mut listed: Vec<(&Path, &SchematicSheet)> = (table.closed.sheets.iter())
            .filter(|(path, _)| pages.contains(path))
            .map(|(path, sheet)| (path.as_path(), sheet))
            .collect();
        listed.sort_by(|a, b| a.0.cmp(b.0));
        listed
    }

    /// The notes beside the scope switch with All sheets: the closed sheets
    /// being read, the ones listed read-only, and the ones not listed.
    pub(crate) fn parameter_scope_notes(&self) -> Vec<signex_i18n::Localized> {
        let i18n = &self.ui_state.i18n;
        let mut notes = Vec::new();
        if self.ui_state.parameter_table.scope != ParameterScope::AllSheets {
            return notes;
        }
        if self.ui_state.parameter_table.closed.reading {
            notes.push(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_CLOSED_READING));
            return notes;
        }
        let listed = self.listed_closed_sheets().len();
        if listed > 0 {
            notes.push(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_CLOSED_LISTED, "count" => listed));
        }
        let unlisted = self.closed_parameter_sheets();
        if unlisted > 0 {
            notes.push(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_CLOSED_SHEETS, "count" => unlisted));
        }
        notes
    }

    /// Open the closed sheet a pressed cell is on, so its cells can be
    /// edited. Nothing is written.
    pub(crate) fn open_closed_parameter_sheet(&mut self, sheet: PathBuf) -> Task<Message> {
        self.open_sheet(sheet, SheetOpen::plain())
    }

    /// The closed sheets among `changed` (sheets an edit across sheets would
    /// change). When there are any, Messages names them and the caller
    /// changes nothing: a closed sheet is opened before it is edited.
    pub(crate) fn refuse_closed_parameter_sheets<'p>(
        &self,
        changed: impl IntoIterator<Item = &'p PathBuf>,
    ) -> bool {
        let closed: Vec<String> = changed
            .into_iter()
            .filter(|path| !self.is_parameter_page_open(path))
            .map(|path| crate::app::view::parameter_manager::sheet_file_name(path))
            .collect();
        if closed.is_empty() {
            return false;
        }
        let line = t!(
            self.ui_state.i18n,
            ids::DIALOG_PARAMETER_MANAGER_REFUSED_CLOSED,
            "count" => closed.len(),
            "sheets" => closed.join(", ")
        );
        crate::diagnostics::log_warning(line.into_string());
        true
    }
}

/// What one read of the closed sheets brought in.
#[derive(Debug, Default)]
pub(super) struct ClosedSheetsReading {
    /// Each page that read as a sheet, by path.
    pub(super) sheets: Vec<(PathBuf, SchematicSheet)>,
    /// Each page that could not be read, with why ("could not be …").
    pub(super) unread: Vec<(PathBuf, String)>,
}

/// Run `read` over `pages` on a blocking worker. A read that fails or
/// panics reports every page as unread, with the reason.
pub(super) async fn read_on_worker(
    pages: Vec<PathBuf>,
    read: fn(&[PathBuf]) -> ClosedSheetsReading,
) -> ClosedSheetsReading {
    let held = pages.clone();
    match tokio::task::spawn_blocking(move || read(&pages)).await {
        Ok(reading) => reading,
        Err(why) => ClosedSheetsReading {
            sheets: Vec::new(),
            unread: (held.into_iter())
                .map(|page| (page, format!("could not be read (the read failed: {why})")))
                .collect(),
        },
    }
}

/// Each of `pages` that reads as a sheet, by path; a page that cannot be read
/// is reported unread (the scope bar counts it as not listed). A page that is
/// not on disk is neither. Blocking.
pub(super) fn read_sheets(pages: &[PathBuf]) -> ClosedSheetsReading {
    let mut reading = ClosedSheetsReading::default();
    for page in pages {
        match crate::app::project_sheets::load_sheet_file(page) {
            Ok(Some(sheet)) => reading.sheets.push((page.clone(), sheet)),
            Ok(None) => {}
            Err(why) => reading.unread.push((page.clone(), why)),
        }
    }
    reading
}
