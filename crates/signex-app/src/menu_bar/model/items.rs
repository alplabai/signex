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

/// The state of a row that runs `cmd` through `dispatch_command`.
fn command_state(cmd: Cmd, is_enabled: bool) -> ItemState {
    AppCommandId::new(cmd.id).map_or(ItemState::Unavailable, |id| {
        live(MenuMessage::Command(id), is_enabled)
    })
}

/// A keymap-backed row that is always enabled and runs its command.
pub(super) fn cmd_item(ctx: &MenuContext, cmd: Cmd) -> MenuEntry {
    command_row(
        ctx,
        cmd,
        cmd_label(cmd.id, cmd.label),
        command_state(cmd, true),
    )
}

/// A keymap-backed row enabled exactly when its command is
/// ([`crate::app::command::is_enabled`]); it runs the command.
pub(super) fn cmd_item_gated(ctx: &MenuContext, cmd: Cmd) -> MenuEntry {
    command_row(
        ctx,
        cmd,
        cmd_label(cmd.id, cmd.label),
        command_state(cmd, enabled(ctx, cmd.id)),
    )
}

/// [`cmd_item_gated`] with a label the context words (a document's name in
/// it), instead of the catalog's.
pub(super) fn cmd_item_gated_as(ctx: &MenuContext, cmd: Cmd, label: String) -> MenuEntry {
    command_row(ctx, cmd, label, command_state(cmd, enabled(ctx, cmd.id)))
}

/// A keymap-backed toggle row, always enabled, with its check mark shown
/// while `checked` (a mode the command switches).
pub(super) fn cmd_toggle(ctx: &MenuContext, cmd: Cmd, checked: bool) -> MenuEntry {
    match cmd_item(ctx, cmd) {
        MenuEntry::Item(item) => MenuEntry::Item(MenuItem {
            checked: Some(checked),
            ..item
        }),
        other => other,
    }
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
