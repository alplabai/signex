//! The native (macOS system menu bar) form of the main menu.
//!
//! Pure and compiled on every OS, so the tests run on all CI platforms: the
//! [`MenuBarModel`] becomes an [`iced::menu::Menu`] that the iced fork syncs to
//! the system menu bar after every update (`iced::daemon(..).menu(..)`; other
//! platforms never call it). The row texts, the lock, the app menu, the quit
//! path, the fallback and why a row shows its shortcut but carries no key
//! equivalent: docs/MENU_BAR.md, "Native macOS menu".

use iced::menu::{self as iced_menu, Item, Predefined};

use super::MenuMessage;
use super::model::{ItemState, Menu, MenuBarModel, MenuEntry, MenuItem};
use crate::app::{Message, WindowMsg};

#[cfg(test)]
pub(crate) mod probe;
mod state;
#[cfg(test)]
mod tests;

pub(crate) use state::NativeMenuState;

/// Title of the app menu. AppKit shows the process name regardless.
const APP_MENU_TITLE: &str = "Signex";

const ABOUT_LABEL: &str = "About Signex";
const PREFERENCES_LABEL: &str = "Preferences...";

/// The one key equivalent the bar registers (docs/MENU_BAR.md).
const PREFERENCES_KEY: &str = "CmdOrCtrl+,";

/// Gap between a row's label and its shortcut text.
const SHORTCUT_GAP: &str = "    ";

/// The bar the model describes: the app menu first, then the model's roots.
/// `locked` greys every model row (a confirm dialog is open); the app menu
/// stays live. Pure: the same model gives the same menu.
pub(crate) fn build(model: &MenuBarModel, locked: bool) -> iced_menu::Menu<Message> {
    let mut items = vec![app_menu()];
    items.extend(
        model
            .menus
            .iter()
            .map(|menu| model_menu(menu, true, locked)),
    );
    iced_menu::Menu::new(items)
}

/// What a model row sends: its action, routed and lock-checked when it
/// arrives (`handle_native_menu_row_chosen`).
fn model_row(action: MenuMessage) -> Message {
    Message::Window(WindowMsg::NativeMenuRowChosen(action))
}

fn app_menu() -> Item<Message> {
    let preferences = crate::keymap::AppCommandId::new("open_preferences")
        .ok()
        .map(|id| Message::Menu(MenuMessage::Command(id)));
    iced_menu::submenu(
        APP_MENU_TITLE,
        [
            iced_menu::item(ABOUT_LABEL, Message::Menu(MenuMessage::OpenAbout)),
            iced_menu::separator(),
            iced_menu::item_maybe(PREFERENCES_LABEL, preferences).shortcut(PREFERENCES_KEY),
            iced_menu::separator(),
            iced_menu::predefined(Predefined::Hide),
            iced_menu::predefined(Predefined::HideOthers),
            iced_menu::predefined(Predefined::ShowAll),
            iced_menu::separator(),
            // A quit request, answered by the unsaved-changes guard
            // (`WindowMsg::QuitRequested`): Command+Q, Dock Quit and logout
            // all take this one path.
            iced_menu::predefined(Predefined::Quit),
        ],
    )
}

fn model_menu(menu: &Menu, is_root: bool, locked: bool) -> Item<Message> {
    let entries = if is_root {
        without_exit(&menu.entries)
    } else {
        &menu.entries[..]
    };
    iced_menu::submenu(
        menu.title.clone(),
        entries.iter().map(|entry| match entry {
            MenuEntry::Separator => iced_menu::separator(),
            MenuEntry::Submenu(sub) => model_menu(sub, false, locked),
            MenuEntry::Item(item) => model_item(item, locked),
        }),
    )
}

fn model_item(item: &MenuItem, locked: bool) -> Item<Message> {
    let message = match &item.state {
        ItemState::Placeholder | ItemState::Unavailable => None,
        ItemState::Live { action, is_enabled } => {
            (*is_enabled && !locked).then(|| model_row(action.clone()))
        }
    };
    // The keymap's shortcut is text in the label, not a key equivalent: a
    // registered one would take the key before the keymap sees it.
    let label = match &item.shortcut {
        Some(shortcut) => format!("{}{SHORTCUT_GAP}{}", item.label, shortcut.text),
        None => item.label.clone(),
    };
    iced_menu::item_maybe(label, message).checked(item.checked)
}

/// A root menu's entries without the Exit row and the separator above it:
/// the app menu carries Quit.
fn without_exit(entries: &[MenuEntry]) -> &[MenuEntry] {
    let is_exit = |entry: &MenuEntry| {
        matches!(
            entry,
            MenuEntry::Item(MenuItem {
                state: ItemState::Live {
                    action: MenuMessage::Exit,
                    ..
                },
                ..
            })
        )
    };
    match entries {
        [rest @ .., MenuEntry::Separator, last] if is_exit(last) => rest,
        [rest @ .., last] if is_exit(last) => rest,
        _ => entries,
    }
}
