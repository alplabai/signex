//! Opening the sync dialog: the selected sheet symbol, its child sheet (the
//! live document when it is open, else read off the UI thread) and the
//! preview of both directions.

use std::path::Path;
use std::sync::Arc;

use iced::Task;
use signex_types::schematic::{ChildSheet, SchematicSheet};

use super::{
    ChildRead, SheetSyncEntries, SheetSyncMsg, SheetSyncPreviewMode, SheetSyncPreviews,
    SheetSyncProblem, SheetSyncState,
};
use crate::app::{Message, Signex};
use crate::library::closed_sheets::FileStamp;

pub(super) const LABEL: &str = "Synchronize Sheet Entries and Ports";

/// Read the closed child sheet at `path` and stamp its file first, so a
/// write after the stamp is seen. Blocking: run it off the UI thread.
fn read_child(path: &Path) -> Result<Arc<ChildRead>, String> {
    let stamp = FileStamp::of(path);
    match crate::app::project_sheets::load_sheet_file(path) {
        Ok(Some(sheet)) => Ok(Arc::new(ChildRead { sheet, stamp })),
        Ok(None) => Err(format!("there is no file at {}", path.display())),
        Err(why) => Err(why),
    }
}

/// The changes each direction would make between `symbol` and `child`.
pub(super) fn previews_of(symbol: &ChildSheet, child: &SchematicSheet) -> SheetSyncPreviews {
    SheetSyncPreviews {
        to_symbol: signex_engine::entry_changes_from_sheet(
            symbol,
            &signex_engine::exposed_sheet_ports(child),
        ),
        to_sheet: signex_engine::port_changes_from_symbol(symbol, child),
    }
}

/// The written side of each direction under `previews`.
pub(super) fn entries_of(
    symbol: &ChildSheet,
    child: &SchematicSheet,
    previews: &SheetSyncPreviews,
) -> SheetSyncEntries {
    SheetSyncEntries {
        to_symbol: signex_engine::symbol_entries_after_sync(symbol, child, &previews.to_symbol),
        to_sheet: signex_engine::sheet_ports_after_sync(symbol, child, &previews.to_sheet),
    }
}

impl Signex {
    /// Open the dialog for the selected sheet symbol. Its child sheet is
    /// the open document when there is one; else it is read off the UI
    /// thread and the preview follows when the read lands.
    pub(super) fn open_sheet_sync(&mut self) -> Task<Message> {
        let Some((symbol, child_filename)) = self.selected_child_sheet() else {
            crate::diagnostics::log_info(format!("{LABEL}: select a sheet symbol first."));
            return Task::none();
        };
        let Some(parent) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        let Some(child_path) = self.resolve_child_sheet_path(&child_filename) else {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the sheet symbol refers to no file ('{child_filename}')"
            ));
            return Task::none();
        };
        let child_path = self.resolve_held_spelling(child_path);
        let symbol_name = self
            .document_state
            .active_engine()
            .and_then(|engine| {
                engine
                    .document()
                    .child_sheets
                    .iter()
                    .find(|s| s.uuid == symbol)
            })
            .map_or_else(String::new, |s| s.name.clone());
        self.ui_state.sheet_sync_reads += 1;
        let generation = self.ui_state.sheet_sync_reads;
        let open = self.document_state.engines.contains_key(&child_path);
        self.ui_state.sheet_sync = Some(Box::new(SheetSyncState {
            generation,
            parent,
            symbol,
            symbol_name,
            child_filename,
            child_path: child_path.clone(),
            epoch: self.document_state.documents_epoch,
            direction: signex_engine::SheetSyncDirection::default(),
            read: None,
            previews: None,
            entries: None,
            preview_mode: SheetSyncPreviewMode::default(),
            preview_expanded: false,
            checking: false,
            problem: None,
        }));
        if open {
            self.refresh_sheet_sync_previews();
            return Task::none();
        }
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || read_child(&child_path)),
            move |result| {
                Message::from(SheetSyncMsg::ChildRead {
                    generation,
                    result: result.and_then(std::convert::identity),
                })
            },
        )
    }

    /// The closed child sheet was read: show the preview, or why not.
    pub(super) fn sheet_sync_child_read(
        &mut self,
        generation: u64,
        result: Result<Arc<ChildRead>, String>,
    ) {
        let Some(state) = self.ui_state.sheet_sync.as_mut() else {
            return;
        };
        if state.generation != generation {
            return;
        }
        match result {
            Ok(read) => {
                state.read = Some(read);
                self.refresh_sheet_sync_previews();
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!("{LABEL}: {why}"));
                state.problem = Some(SheetSyncProblem::ReadFailed(why));
            }
        }
    }

    /// Work out both previews again from the live sheets.
    pub(super) fn refresh_sheet_sync_previews(&mut self) {
        let Some(state) = self.ui_state.sheet_sync.as_ref() else {
            return;
        };
        let result = self.sheet_sync_sides(state).map(|(symbol, child)| {
            let previews = previews_of(&symbol, &child);
            let entries = entries_of(&symbol, &child, &previews);
            (previews, entries)
        });
        let Some(state) = self.ui_state.sheet_sync.as_mut() else {
            return;
        };
        match result {
            Ok((previews, entries)) => {
                state.previews = Some(previews);
                state.entries = Some(entries);
            }
            Err(problem) => state.problem = Some(problem),
        }
    }

    /// The sheet symbol and the child sheet the dialog syncs, as they are
    /// now, or [`SheetSyncProblem::Stale`] when the symbol is gone or
    /// re-pointed, or a sheet was opened or closed since the dialog opened.
    /// The symbol is read off the dialog's own parent sheet, whichever sheet
    /// the main window shows: the dialog may belong to an undocked window.
    pub(super) fn sheet_sync_sides(
        &self,
        state: &SheetSyncState,
    ) -> Result<(ChildSheet, SchematicSheet), SheetSyncProblem> {
        let docs = &self.document_state;
        if docs.documents_epoch != state.epoch {
            return Err(SheetSyncProblem::Stale);
        }
        let symbol = docs
            .engines
            .get(&state.parent)
            .and_then(|engine| {
                engine
                    .document()
                    .child_sheets
                    .iter()
                    .find(|s| s.uuid == state.symbol && s.filename == state.child_filename)
                    .cloned()
            })
            .ok_or(SheetSyncProblem::Stale)?;
        let child = match docs.engines.get(&state.child_path) {
            Some(engine) => engine.document().clone(),
            None => state
                .read
                .as_ref()
                .map(|read| read.sheet.clone())
                .ok_or(SheetSyncProblem::Stale)?,
        };
        Ok((symbol, child))
    }
}
