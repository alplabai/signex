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

/// One row. `action` is `None` for a placeholder that has no handler yet;
/// a row with an action but `is_enabled == false` is a live row that is
/// greyed out in the current context. A row only dispatches when it has an
/// action and is enabled (see [`MenuItem::dispatches`]).
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub action: Option<MenuMessage>,
    pub shortcut: Option<Shortcut>,
    pub is_enabled: bool,
}

/// A row's shortcut: the display text, plus the command id it belongs to so a
/// native menu can derive a real accelerator from the keymap instead of
/// parsing the text.
#[derive(Debug, Clone)]
pub struct Shortcut {
    pub command_id: &'static str,
    pub text: String,
}

impl MenuItem {
    /// The message a click sends, if any.
    pub fn dispatches(&self) -> Option<&MenuMessage> {
        self.action.as_ref().filter(|_| self.is_enabled)
    }
}

/// Build the menu bar for `ctx`. Pure: the same context gives the same model.
pub fn menu_bar_model(ctx: &MenuContext) -> MenuBarModel {
    MenuBarModel {
        menus: vec![
            menus::file(ctx),
            menus::edit(ctx),
            menus::view(ctx),
            menus::place(ctx),
            menus::design(ctx),
            menus::tools(ctx),
            menus::window(),
            menus::help(ctx),
        ],
    }
}
