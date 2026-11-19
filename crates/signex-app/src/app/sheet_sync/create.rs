//! Create Sheet Symbol From Sheet: pick a sheet file, read what its checks
//! need off the UI thread (the file, the project's closed sheets, a link out
//! of the project folder), and place a sheet symbol for it on the active sheet
//! with an entry for every port, as one undo step. The checks and the
//! placement are the operand form's (`place_symbol_from_sheet_file`), so one
//! command id has one behaviour. The symbol goes to the cursor's last point on
//! this sheet, else under the sheet's content.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::ids;
use signex_types::schematic::Point;

use super::SheetSyncMsg;
use crate::app::command::EditReads;
use crate::app::{Message, Signex};

const LABEL: &str = "Create Sheet Symbol From Sheet";

/// The reads Create Sheet Symbol From Sheet waited for, for the sheet
/// `target`, and the revisions they depend on.
#[derive(Debug, Clone)]
pub struct SymbolSheetReads {
    pub(super) path: PathBuf,
    pub(super) target: PathBuf,
    pub(super) revisions: BTreeMap<PathBuf, u64>,
    pub(super) reads: Arc<EditReads>,
}

impl Signex {
    /// The sheet a new sheet symbol lands on: the active schematic, when it
    /// takes an edit. Says why not in the Messages panel.
    fn create_symbol_target(&mut self) -> Option<PathBuf> {
        if !self.has_active_schematic() {
            crate::diagnostics::log_warning(format!("{LABEL} works on a schematic sheet."));
            return None;
        }
        if let Some(reason) = self.refuse_if_active_read_only() {
            crate::diagnostics::log_warning(format!("{LABEL}: {reason}"));
            return None;
        }
        self.document_state.active_path.clone()
    }

    /// Ask for the sheet file to make a sheet symbol for.
    pub(super) fn create_symbol_dialog(&mut self) -> Task<Message> {
        if self.create_symbol_target().is_none() {
            return Task::none();
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::SHEET_SYNC_PICK_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_SCHEMATIC, &["snxsch"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        Task::perform(dialog.pick_file(), |path| {
            Message::from(SheetSyncMsg::CreateSymbolPicked(path))
        })
    }

    /// Read what placing the picked file on the active sheet needs, off the
    /// UI thread.
    pub(super) fn read_symbol_sheet(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let Some(target) = self.create_symbol_target() else {
            return Task::none();
        };
        let (revisions, read) = self.symbol_sheet_reads(&target, &path);
        Task::perform(read, move |reads| {
            Message::from(SheetSyncMsg::CreateSymbolRead(SymbolSheetReads {
                path: path.clone(),
                target: target.clone(),
                revisions: revisions.clone(),
                reads: Arc::new(reads),
            }))
        })
    }

    /// The reads landed: place the sheet symbol, unless the user moved to
    /// another sheet meanwhile. The outcome goes to the Messages panel.
    pub(super) fn symbol_sheet_read(&mut self, read: SymbolSheetReads) {
        let SymbolSheetReads {
            path,
            target,
            revisions,
            reads,
        } = read;
        if self.document_state.active_path.as_deref() != Some(target.as_path()) {
            crate::diagnostics::log_info(format!(
                "{LABEL} cancelled: the sheet changed before the file was read."
            ));
            return;
        }
        let at = self.symbol_from_sheet_spot(&target);
        let file = path.to_string_lossy().into_owned();
        let placed = self.with_gui_reads(&target, &revisions, reads, |app| {
            app.place_symbol_from_sheet_file(&file, at, false)
        });
        match placed {
            Ok(placed) => {
                let entries = placed.entries.len();
                crate::diagnostics::log_info(format!(
                    "{LABEL}: placed sheet symbol '{}' for {} with {entries} sheet entr{}",
                    placed.name,
                    placed.reference,
                    if entries == 1 { "y" } else { "ies" }
                ));
            }
            Err(why) => crate::diagnostics::log_warning(format!("{LABEL}: {why}")),
        }
    }

    /// The top-left corner of the new symbol: the cursor's last point on
    /// `target`, on the grid; `None` puts it under the sheet's content.
    fn symbol_from_sheet_spot(&self, target: &Path) -> Option<Point> {
        let ui = &self.ui_state;
        if ui.cursor_document.as_deref() != Some(target) {
            return None;
        }
        let (x, y) = self.snap_to_grid(ui.cursor_x, ui.cursor_y);
        Some(Point::new(x, y))
    }
}
