//! Declarative model of the main menu bar.
//!
//! Plain data with no renderer types: the in-window iced menu renders it
//! today, and a native OS menu can be built from the same value. Every row
//! carries the [`MenuMessage`] it dispatches and, for a keymap-backed row, the
//! command id the shortcut was resolved from. The model is built by
//! [`menu_bar_model`], a pure function of the [`MenuContext`].

mod items;
mod menus;

use super::{MenuContext, MenuMessage};
use crate::keymap::AppCommandId;

/// The whole bar: the root menus, left to right.
#[derive(Debug, Clone)]
pub struct MenuBarModel {
    pub menus: Vec<Menu>,
}

/// A root menu or a submenu: a title over an ordered list of entries.
#[derive(Debug, Clone)]
pub struct Menu {
    pub title: String,
    pub entries: Vec<MenuEntry>,
}

#[derive(Debug, Clone)]
pub enum MenuEntry {
    Item(MenuItem),
    Submenu(Menu),
    Separator,
}

/// One row: a label, an optional shortcut, and what the row can do.
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub shortcut: Option<Shortcut>,
    pub state: ItemState,
}

/// A placeholder is a planned feature with no handler yet (greyed, "planned"
/// tooltip); an unavailable row is greyed informational text; a live row has one and is greyed out
/// unless enabled in the current context.
#[derive(Debug, Clone)]
pub enum ItemState {
    Placeholder,
    Unavailable,
    Live {
        action: MenuMessage,
        is_enabled: bool,
    },
}

/// A row's shortcut: the display text, plus the command id it belongs to so a
/// native menu can derive a real accelerator from the keymap instead of
/// parsing the text.
#[derive(Debug, Clone)]
pub struct Shortcut {
    pub command_id: AppCommandId,
    pub text: String,
}

impl MenuItem {
    /// The message a click sends: only a live, enabled row has one.
    pub fn dispatches(&self) -> Option<&MenuMessage> {
        match &self.state {
            ItemState::Live {
                action,
                is_enabled: true,
            } => Some(action),
            _ => None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.dispatches().is_some()
    }
}

/// Titles of the root menus, left to right: the one list both the model and
/// the chrome's width estimate read.
pub const ROOT_TITLES: [&str; 8] = [
    "File", "Edit", "View", "Place", "Design", "Tools", "Window", "Help",
];

/// Build the menu bar for `ctx`. Pure: the same context gives the same model.
pub fn menu_bar_model(ctx: &MenuContext) -> MenuBarModel {
    let entries = [
        menus::file(ctx),
        menus::edit(ctx),
        menus::view(ctx),
        menus::place(ctx),
        menus::design(ctx),
        menus::tools(ctx),
        menus::window(),
        menus::help(ctx),
    ];
    MenuBarModel {
        menus: ROOT_TITLES
            .into_iter()
            .zip(entries)
            .map(|(title, entries)| Menu {
                title: title.to_owned(),
                entries,
            })
            .collect(),
    }
}
