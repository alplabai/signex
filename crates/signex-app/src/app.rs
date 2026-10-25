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
mod symbol_history;
mod undo_groups;
pub use undo_groups::UndoGroupSheets;
pub(crate) mod view;

pub use contracts::{
    AnnotateMsg, AppQuitConfirmState, BomPreviewMsg, CommandPaletteMsg, ContextAction,
    ContextMenuMsg, ContextMenuState, ContextSubmenu, DragTarget, EditMsg, EnableVersionControlMsg,
    EnableVersionControlState, ErcMsg, ExportMsg, FileMsg, GridPickerState, GridPropertiesMsg,
    GridPropertiesState, LibraryRowRef, Message, MoveSelectionMsg, NetColorMsg, OverlayMsg,
    ParameterManagerMsg, PreferencesMsg, PrintPreviewMsg, ProjectCloseChoice,
    ProjectCloseConfirmState, ProjectMsg, ProjectOptionsState, ProjectTreeAction,
    ProjectTreeContextMenuState, RemoveChoice, RemoveDialogState, RemoveMsg, RenameDialogState,
    RenameMsg, SelectionFilterCustomState, SelectionFilterMsg, StatusBarRequest, TabContextAction,
    TabContextMenuState, TextEditMsg, TextEditState, TrackItem, UiMsg, VersionControlScope,
    WindowMsg,
};
pub use documents::{
    ComponentEditorTab, DrawMode, FootprintEditorState, FootprintHistorySnapshot,
    GraphicFillPicker, LocalColorPicker, LocalColorSlot, SchematicTabSession, SymbolEditorState,
    SymbolViewRequest, TabDocument, TabInfo, TabKind, Tool,
};
pub(crate) use handlers::selection_workflow::passes_filter;
pub use state::{
    DocumentState, InteractionState, KeymapRecorderState, LoadedProject, PageSetupInputs,
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
        let task = self.dispatch_update(message);
        // `refresh_panel_ctx` only records what the project tree needs from
        // the disk; the probe that fetches it starts here (#111).
        match self.start_project_fs_probe(false) {
            Some(probe) => Task::batch([task, probe]),
            None => task,
        }
    }
}
