//! Main Iced application — module root, update loop, view tree.

use iced::{Task, Theme};
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

use crate::canvas::CanvasEvent;
use crate::dock::PanelPosition;
use crate::menu_bar::{self, MenuMessage};
use crate::status_bar;
use crate::tab_bar::{self, TabMessage};
use crate::toolbar::ToolMessage;

mod actions;
mod bootstrap;
pub(crate) mod command;
pub mod command_palette;
pub mod contracts;
mod dir_listing;
pub(crate) mod dispatch;
mod documents;
mod handlers;
mod helpers;
mod load_gateway;
mod mutation_gateway;
mod pcb_dirty_adapter;
pub(crate) mod project_sheets;
mod runtime;
mod selection_request;
pub(crate) mod sheet_hierarchy;
mod state;
pub(crate) use state::library_entry;
pub(crate) mod stitched_netlist;
mod symbol_history;
mod undo_groups;
pub use undo_groups::{UndoGroupKind, UndoGroupSheets};
pub(crate) mod view;

pub use contracts::{
    AnnotateMsg, AppQuitConfirmState, BomPreviewMsg, CommandPaletteMsg, ContextAction,
    ContextMenuMsg, ContextMenuState, ContextSubmenu, DragTarget, EditMsg, EnableVersionControlMsg,
    EnableVersionControlOutcome, EnableVersionControlState, ErcMsg, ExcludeDirtyChoice, ExportMsg,
    FileMsg, GitCommitFailure, GridPickerState, GridPropertiesMsg, GridPropertiesState, InputMsg,
    LibraryRowRef, Message, MoveSelectionMsg, NetColorMsg, OverlayMsg, ParameterManagerMsg,
    PreferencesMsg, PrintPreviewMsg, ProjectCloseChoice, ProjectCloseConfirmState, ProjectMsg,
    ProjectOptionsState, ProjectRenameJob, ProjectTreeAction, ProjectTreeContextMenuState,
    RemoveChoice, RemoveDialogState, RemoveMsg, RenameDialogState, RenameMsg,
    SelectionFilterCustomState, SelectionFilterMsg, StatusBarRequest, TabContextAction,
    TabContextMenuState, TextEditMsg, TextEditState, TrackItem, UiMsg, VersionControlScope,
    WindowMsg,
};
pub use documents::{
    ComponentEditorTab, DrawMode, FootprintDocument, FootprintHistorySnapshot, GraphicFillPicker,
    LocalColorPicker, LocalColorSlot, SchematicTabSession, SymbolDocument, SymbolViewRequest,
    TabDocument, TabInfo, TabKind, Tool,
};
pub(crate) use handlers::selection_workflow::passes_filter;
pub use state::net_class_styles::{ResolveFailed, ResolvedStyles};
pub use state::{
    CaseFold, DocumentState, InteractionState, KeymapRecorderState, LoadedProject, PageSetupInputs,
    PreviewState, ProjectId, Signex, UiState,
};
pub use symbol_history::SymbolHistorySnapshot;

// Re-exported so modal dialogs outside `state` can reference the type.
pub use state::AnnotateOrder;

impl Signex {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Messages that complete a save, delete, rename or library write may
        // have changed the disk under an unchanged path set; the probe below
        // then runs now instead of at the next 2 s tick.
        if state::project_fs::message_touches_disk(&message) {
            self.document_state.project_fs.invalidate();
        }
        let before = self.active_tab_path();
        let mut tasks = vec![self.dispatch_update(message)];
        // Any activation path (select, open, close-neighbour, Ctrl+Tab)
        // ends here: scroll the tab strip to the newly active tab.
        if self.active_tab_path() != before {
            let docs = &self.document_state;
            tasks.push(tab_bar::reveal_active_tab(docs.active_tab, docs.tabs.len()));
        }
        // Every schematic open queued a look for an earlier run's recovery
        // copy; start them whichever path opened the tab (#1208).
        let checks = self.start_due_recovery_checks();
        tasks.push(checks);
        // `refresh_panel_ctx` only records what the project tree needs from
        // the disk; the probe that fetches it starts here (#111).
        if let Some(probe) = self.start_project_fs_probe(false) {
            tasks.push(probe);
        }
        if tasks.len() == 1 {
            return tasks.pop().expect("one task");
        }
        Task::batch(tasks)
    }
}
