//! Row constructors the menu builders share.

use super::{ItemState, Menu, MenuEntry, MenuItem, Shortcut};
use crate::keymap::AppCommandId;
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

fn plain(label: &str, state: ItemState) -> MenuEntry {
    MenuEntry::Item(MenuItem {
        label: label.to_owned(),
        shortcut: None,
        state,
        checked: None,
    })
}

fn live(action: MenuMessage, is_enabled: bool) -> ItemState {
    ItemState::Live { action, is_enabled }
}

/// An always-enabled row.
pub(super) fn item(label: &str, action: MenuMessage) -> MenuEntry {
    plain(label, live(action, true))
}

/// An always-enabled panel row, checked while `kind` is open (the same
/// answer as the status-bar panel list, via [`MenuContext::open_panels`]).
pub(super) fn panel_item(
    ctx: &MenuContext,
    label: &str,
    kind: crate::panels::PanelKind,
    action: MenuMessage,
) -> MenuEntry {
    debug_assert!(action.is_panel_row());
    MenuEntry::Item(MenuItem {
        label: label.to_owned(),
        shortcut: None,
        state: live(action, true),
        checked: Some(ctx.open_panels.contains(&kind)),
    })
}

/// A live row, greyed out unless `is_enabled`.
pub(super) fn item_if(label: &str, action: MenuMessage, is_enabled: bool) -> MenuEntry {
    plain(label, live(action, is_enabled))
}

/// A greyed informational row that is not a missing feature (no tooltip).
pub(super) fn unavailable(label: &str) -> MenuEntry {
    plain(label, ItemState::Unavailable)
}

/// A planned feature with no handler yet: greyed with the "planned" tooltip.
/// Every call site is pinned by the ratchet in `planned_tests`; add a real
/// action instead of a new stub.
pub(super) fn stub(label: &str) -> MenuEntry {
    plain(label, ItemState::Placeholder)
}

fn command_row(ctx: &MenuContext, cmd: Cmd, label: String, state: ItemState) -> MenuEntry {
    let shortcut = AppCommandId::new(cmd.id).ok().and_then(|command_id| {
        shortcut_for(ctx, cmd.id, cmd.shortcut).map(|text| Shortcut { command_id, text })
    });
    MenuEntry::Item(MenuItem {
        label,
        shortcut,
        state,
        checked: None,
    })
}

/// A keymap-backed row that is always enabled.
pub(super) fn cmd_item(ctx: &MenuContext, cmd: Cmd, action: MenuMessage) -> MenuEntry {
    command_row(ctx, cmd, cmd_label(cmd.id, cmd.label), live(action, true))
}

/// A keymap-backed row enabled exactly when its command is
/// ([`crate::app::command::is_enabled`]).
pub(super) fn cmd_item_gated(ctx: &MenuContext, cmd: Cmd, action: MenuMessage) -> MenuEntry {
    command_row(
        ctx,
        cmd,
        cmd_label(cmd.id, cmd.label),
        live(action, enabled(ctx, cmd.id)),
    )
}

/// A keymap-backed row with its own enablement rule.
pub(super) fn cmd_item_if(
    ctx: &MenuContext,
    cmd: Cmd,
    action: MenuMessage,
    is_enabled: bool,
) -> MenuEntry {
    command_row(
        ctx,
        cmd,
        cmd_label(cmd.id, cmd.label),
        live(action, is_enabled),
    )
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
