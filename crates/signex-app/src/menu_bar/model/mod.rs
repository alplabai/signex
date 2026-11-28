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
    /// A glyph in the check column, drawn while no check mark shows there.
    pub icon: Option<MenuIcon>,
    /// What a tile row would do, previewed beside it while it is hovered
    /// (mockup row 207); a native menu shows none.
    pub preview: Option<TilePreview>,
}

/// The preview beside a tile row of View ▸ Window (mockup row 207): which
/// documents land in which pane. Plain data, built by the app.
#[derive(Debug, Clone, PartialEq)]
pub struct TilePreview {
    /// How the panes are arranged.
    pub layout: crate::app::TileLayout,
    /// "Tile Vertically · 4 open documents".
    pub title: String,
    /// Each pane's document names, in pane order.
    pub panes: Vec<Vec<String>>,
    /// The line under the panes: what the arrangement does.
    pub note: String,
}

/// The previews of the three tile rows; `None` where the row would arrange
/// fewer than two documents.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TilePreviews {
    pub horizontal: Option<TilePreview>,
    pub vertical: Option<TilePreview>,
    pub grid: Option<TilePreview>,
}

/// The glyph a row draws in its check column (mockup row 166). Plain data;
/// the renderer picks the image, a native menu draws none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuIcon {
    Save,
    Reset,
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
/// The rows extensions add come after every built-in row of their root
/// menu, below one separator (#2495); with none, the bar is the built-in one.
pub fn menu_bar_model(ctx: &MenuContext) -> MenuBarModel {
    let mut entries = [
        menus::file(ctx),
        menus::edit(ctx),
        menus::view(ctx),
        menus::project(ctx),
        menus::place(ctx),
        menus::design(ctx),
        menus::tools(ctx),
        menus::help(ctx),
    ];
    for (index, root) in entries.iter_mut().enumerate() {
        let mut rows = ctx
            .extension_rows
            .iter()
            .filter(|row| row.root.index() == index)
            .peekable();
        if rows.peek().is_some() {
            root.push(MenuEntry::Separator);
            root.extend(rows.map(|row| items::extension_item(ctx, row)));
        }
    }
    MenuBarModel {
        menus: root_titles(&ctx.i18n)
            .into_iter()
            .zip(entries)
            .map(|(title, entries)| Menu { title, entries })
            .collect(),
    }
}
