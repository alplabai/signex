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
mod contracts;
mod dispatch;
mod documents;
mod handlers;
mod helpers;
mod load_gateway;
mod mutation_gateway;
mod runtime;
mod selection_request;
mod state;
mod view;

pub use contracts::{
    ContextAction, ContextMenuState, DragTarget, Message, StatusBarRequest, TextEditState,
};
pub use documents::{DrawMode, SchematicTabSession, TabDocument, TabInfo, Tool};
pub use state::{DocumentState, InteractionState, Signex, UiState};

impl Signex {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.dispatch_update(message)
    }
}
