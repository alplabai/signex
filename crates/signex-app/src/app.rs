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
pub mod command_palette;
pub mod contracts;
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
mod state;
pub(crate) mod view;

pub use contracts::{
    AnnotateMsg, AppQuitConfirmState, BomPreviewMsg, CommandPaletteMsg, ContextAction,
    ContextMenuMsg, ContextMenuState, ContextSubmenu, DragTarget, EditMsg, EnableVersionControlMsg,
    EnableVersionControlState, ErcMsg, ExportMsg, FileMsg, GridPickerState, GridPropertiesMsg,
    GridPropertiesState, Message, MoveSelectionMsg, NetColorMsg, OverlayMsg, ParameterManagerMsg,
    PreferencesMsg, PrintPreviewMsg, ProjectCloseChoice, ProjectCloseConfirmState, ProjectMsg,
    ProjectOptionsState, ProjectTreeAction, ProjectTreeContextMenuState, RemoveChoice,
    RemoveDialogState, RemoveMsg, RenameDialogState, RenameMsg, SelectionFilterCustomState,
    SelectionFilterMsg, StatusBarRequest, TabContextAction, TabContextMenuState, TextEditMsg,
    TextEditState, TrackItem, UiMsg, VersionControlScope, WindowMsg,
};
pub use documents::{
    ComponentEditorTab, DrawMode, FootprintEditorState, SchematicTabSession, SymbolEditorState,
    TabDocument, TabInfo, TabKind, Tool,
};
pub use state::{
    DocumentState, InteractionState, KeymapRecorderState, LoadedProject, ProjectId, Signex, UiState,
};

// Re-exported so modal dialogs outside `state` can reference the type.
#[allow(unused_imports)]
pub use state::AnnotateOrder;

impl Signex {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.dispatch_update(message)
    }
}
