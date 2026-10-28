//! Row constructors the menu builders share.

use super::{Menu, MenuEntry, MenuItem, Shortcut};
use crate::menu_bar::{MenuContext, MenuMessage, cmd_label, enabled, shortcut_for};

/// A keymap-backed row: the command id, the label used when the command
/// catalog has none, and the shortcut text used when the active keymap binds
/// none.
#[derive(Clone, Copy)]
pub(super) struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: &'static str,
}

pub(super) const fn cmd(id: &'static str, label: &'static str, shortcut: &'static str) -> Cmd {
    Cmd {
        id,
        label,
        shortcut,
    }
}

fn plain(label: &str, action: Option<MenuMessage>, is_enabled: bool) -> MenuEntry {
    MenuEntry::Item(MenuItem {
        label: label.to_owned(),
        action,
        shortcut: None,
        is_enabled,
    })
}

/// An always-enabled row.
pub(super) fn item(label: &str, action: MenuMessage) -> MenuEntry {
    plain(label, Some(action), true)
}

/// A live row, greyed out unless `is_enabled`.
pub(super) fn item_if(label: &str, action: MenuMessage, is_enabled: bool) -> MenuEntry {
    plain(label, Some(action), is_enabled)
}

/// A placeholder with no handler yet.
pub(super) fn stub(label: &str) -> MenuEntry {
    plain(label, None, false)
}

fn command_row(
    ctx: &MenuContext,
    cmd: Cmd,
    action: Option<MenuMessage>,
    is_enabled: bool,
) -> MenuEntry {
    MenuEntry::Item(MenuItem {
        label: cmd_label(cmd.id, cmd.label),
        action,
        shortcut: shortcut_for(ctx, cmd.id, cmd.shortcut).map(|text| Shortcut {
            command_id: cmd.id,
            text,
        }),
        is_enabled,
    })
}

/// A keymap-backed row that is always enabled.
pub(super) fn cmd_item(ctx: &MenuContext, cmd: Cmd, action: MenuMessage) -> MenuEntry {
    command_row(ctx, cmd, Some(action), true)
}

/// A keymap-backed row enabled exactly when its command is
/// ([`crate::app::command::is_enabled`]).
pub(super) fn cmd_item_gated(ctx: &MenuContext, cmd: Cmd, action: MenuMessage) -> MenuEntry {
    command_row(ctx, cmd, Some(action), enabled(ctx, cmd.id))
}

/// A keymap-backed row with its own enablement rule.
pub(super) fn cmd_item_if(
    ctx: &MenuContext,
    cmd: Cmd,
    action: MenuMessage,
    is_enabled: bool,
) -> MenuEntry {
    command_row(ctx, cmd, Some(action), is_enabled)
}

/// A keymap-backed placeholder with no handler yet.
pub(super) fn cmd_stub(ctx: &MenuContext, cmd: Cmd) -> MenuEntry {
    command_row(ctx, cmd, None, false)
}

pub(super) fn submenu(title: &str, entries: Vec<MenuEntry>) -> MenuEntry {
    MenuEntry::Submenu(menu(title, entries))
}

pub(super) fn menu(title: &str, entries: Vec<MenuEntry>) -> Menu {
    Menu {
        title: title.to_owned(),
        entries,
    }
}
