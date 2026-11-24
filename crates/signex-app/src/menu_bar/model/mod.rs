//! Declarative model of the main menu bar.
//!
//! Plain data with no renderer types: the in-window iced menu renders it
//! today, and a native OS menu can be built from the same value. Every row
//! carries the [`MenuMessage`] it dispatches and, for a keymap-backed row, the
//! command id the shortcut was resolved from. The model is built by
//! [`menu_bar_model`], a pure function of the [`MenuContext`].

mod items;
mod menus;
mod sheet_place;

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
    /// `Some` for a toggle row (a panel): whether its check mark shows.
    pub checked: Option<bool>,
}

/// A placeholder is a feature with no handler yet (greyed, its `reason` as
/// the tooltip, else "planned"); an unavailable row is greyed informational
/// text; a live row has one and is greyed out unless enabled in the current
/// context; a header is a small caption over the rows below it, not a
/// command at all: docs/MENU_BAR.md#header-entries
#[derive(Debug, Clone)]
pub enum ItemState {
    Placeholder {
        reason: Option<String>,
    },
    Unavailable,
    Live {
        action: MenuMessage,
        is_enabled: bool,
    },
    Header,
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

    /// Whether the row is a section caption rather than a command row.
    pub fn is_header(&self) -> bool {
        matches!(self.state, ItemState::Header)
    }
}

/// The English titles of the root menus, left to right, which the tests pin;
/// the bar reads [`ROOT_TITLE_IDS`]. Reports and Window are submenus of Tools
/// and View: docs/MENU_BAR.md#root-menus
#[cfg(test)]
pub const ROOT_TITLES: [&str; 8] = [
    "File", "Edit", "View", "Project", "Place", "Design", "Tools", "Help",
];

/// The catalog message of each root title, left to right.
const ROOT_TITLE_IDS: [signex_i18n::MessageId; 8] = [
    signex_i18n::ids::MENU_FILE,
    signex_i18n::menu_ids::MENU_EDIT,
    signex_i18n::menu_ids::MENU_VIEW,
    signex_i18n::menu_ids::MENU_PROJECT,
    signex_i18n::menu_ids::MENU_PLACE,
    signex_i18n::menu_ids::MENU_DESIGN,
    signex_i18n::menu_ids::MENU_TOOLS,
    signex_i18n::menu_ids::MENU_HELP,
];

/// The root titles as the bar draws them under `i18n`.
pub fn root_titles(i18n: &signex_i18n::I18n) -> Vec<String> {
    ROOT_TITLE_IDS
        .into_iter()
        .map(|id| i18n.message(id, &[]).into_string())
        .collect()
}

/// Build the menu bar for `ctx`. Pure: the same context gives the same model.
pub fn menu_bar_model(ctx: &MenuContext) -> MenuBarModel {
    let entries = [
        menus::file(ctx),
        menus::edit(ctx),
        menus::view(ctx),
        menus::project(ctx),
        menus::place(ctx),
        menus::design(ctx),
        menus::tools(ctx),
        menus::help(ctx),
    ];
    MenuBarModel {
        menus: root_titles(&ctx.i18n)
            .into_iter()
            .zip(entries)
            .map(|(title, entries)| Menu { title, entries })
            .collect(),
    }
}
