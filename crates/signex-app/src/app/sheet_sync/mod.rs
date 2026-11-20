//! Bottom-up hierarchy (#1276): Design ▸ Create Sheet Symbol From Sheet and
//! Design ▸ Synchronize Sheet Entries and Ports.
//!
//! Create reads the picked sheet off the UI thread and places a sheet symbol
//! with an entry for every port as one undo step. The sync dialog previews
//! the entries or ports either direction would add, remove and rename, and
//! Apply makes exactly those changes as one undo step: on the sheet symbol
//! (`Command::ReconcileChildSheetPins`), or on the child sheet, which is
//! loaded into a tabless engine and left unsaved when it is not open. The
//! rules live in `signex_engine::sheet_sync`; docs/DECISIONS.md, "Bottom-up
//! hierarchy".

mod apply;
mod create;
mod open;
mod state;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_engine::SheetSyncDirection;

pub use create::SymbolSheetReads;
pub use state::{ChildRead, SheetSyncPreviews, SheetSyncProblem, SheetSyncState};

use crate::app::{Message, Signex};

impl From<SheetSyncMsg> for Message {
    fn from(msg: SheetSyncMsg) -> Self {
        Self::Edit(crate::app::EditMsg::SheetSync(msg))
    }
}

/// What happened in Create Sheet Symbol From Sheet or the sync dialog.
#[derive(Debug, Clone)]
pub enum SheetSyncMsg {
    /// The menu row, palette or a key asked to create a sheet symbol from a
    /// sheet file.
    CreateSymbolRequested,
    /// The file dialog answered for the sheet `target` it was opened on (in
    /// an undocked window, that window's sheet); `file` is `None` when the
    /// dialog was cancelled.
    CreateSymbolPicked {
        target: PathBuf,
        file: Option<PathBuf>,
    },
    /// What placing the picked sheet needs was read off the UI thread.
    CreateSymbolRead(SymbolSheetReads),
    /// The menu row, palette or a key asked for the sync dialog.
    Requested,
    /// Read `generation` of the closed child sheet finished.
    ChildRead {
        generation: u64,
        result: Result<Arc<ChildRead>, String>,
    },
    /// A direction was chosen.
    DirectionPicked(SheetSyncDirection),
    ApplyRequested,
    /// Apply's re-stat of the closed child sheet finished: the files that
    /// changed on disk since read `generation`, or why they could not be
    /// stated.
    DiskChecked {
        generation: u64,
        result: Result<Vec<PathBuf>, String>,
    },
    CancelRequested,
}

impl Signex {
    pub(crate) fn dispatch_sheet_sync(&mut self, msg: SheetSyncMsg) -> Task<Message> {
        let task = match msg {
            SheetSyncMsg::CreateSymbolRequested => self.create_symbol_dialog(),
            SheetSyncMsg::CreateSymbolPicked { target, file } => {
                self.read_symbol_sheet(target, file)
            }
            SheetSyncMsg::CreateSymbolRead(read) => {
                self.symbol_sheet_read(read);
                Task::none()
            }
            SheetSyncMsg::Requested => self.open_sheet_sync(),
            SheetSyncMsg::ChildRead { generation, result } => {
                self.sheet_sync_child_read(generation, result);
                Task::none()
            }
            SheetSyncMsg::DirectionPicked(direction) => {
                // A refused Apply's reason belongs to the other direction; a
                // failed read stays, since there is still no preview.
                if let Some(state) = self.ui_state.sheet_sync.as_mut() {
                    state.direction = direction;
                    if state.previews.is_some() {
                        state.problem = None;
                    }
                }
                Task::none()
            }
            SheetSyncMsg::ApplyRequested => self.apply_sheet_sync(),
            SheetSyncMsg::DiskChecked { generation, result } => {
                self.sheet_sync_disk_checked(generation, result)
            }
            SheetSyncMsg::CancelRequested => {
                self.ui_state.sheet_sync = None;
                Task::none()
            }
        };
        Task::batch([task, self.finish_update()])
    }
}
