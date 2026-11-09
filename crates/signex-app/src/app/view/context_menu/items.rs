//! Shared context-menu primitives — the pure `DropdownEntry` builders
//! (`dd_kb`, `dd_msg`, `dd_disabled`, `save_entry`), the hover-driven
//! submenu launcher (`submenu_launcher`, an item row with hover hooks), and the
//! two `&self` lookups the menu builders need (`keymap_shortcut_label`,
//! `library_row_ref`).
//!
//! Data-to-view (#269): every context menu is a `Vec<DropdownEntry>` that
//! the shared `signex_widgets::active_bar_dropdown` widget renders, so the
//! canvas / project-tree / tab menus share ONE row renderer with the
//! schematic + footprint active bars (ADR-0003). The widget owns all row
//! chrome (icon column, label, shortcut, hover, disabled greying), so the
//! hand-built row buttons that used to live here are gone.

use super::*;

use iced::widget::svg::Handle;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

/// A keyboard-command row: clicking it dispatches `action` through the
/// context-menu message bridge. `shortcut` renders right-aligned (empty
/// = no hint); `icon` is optional (the widget still reserves the glyph
/// column so labels stay aligned).
pub(super) fn dd_kb(
    icon: Option<Handle>,
    label: &str,
    shortcut: &str,
    action: ContextAction,
) -> DropdownEntry<Message> {
    dd_msg(
        icon,
        label,
        shortcut,
        Message::ContextMenu(ContextMenuMsg::Action(action)),
    )
}

/// A row that publishes an arbitrary `Message` on click.
pub(super) fn dd_msg(
    icon: Option<Handle>,
    label: &str,
    shortcut: &str,
    message: Message,
) -> DropdownEntry<Message> {
    let mut item = DropdownItem::new(label, message);
    if let Some(handle) = icon {
        item = item.icon(handle);
    }
    if !shortcut.is_empty() {
        item = item.shortcut(shortcut);
    }
    DropdownEntry::Item(item)
}

/// A greyed, non-clickable row — used for "coming soon" stubs and gated
/// actions. `right` is the optional right-column text (a keyboard hint,
/// a `vX.Y` version badge, or the `›` submenu chevron for placeholder
/// launchers). Built as a passive `DropdownItem` (no `on_press`).
pub(super) fn dd_disabled(
    icon: Option<Handle>,
    label: &str,
    right: Option<&str>,
) -> DropdownEntry<Message> {
    let mut item = DropdownItem::passive(label).disabled(true);
    if let Some(handle) = icon {
        item = item.icon(handle);
    }
    if let Some(right) = right {
        item = item.shortcut(right);
    }
    DropdownEntry::Item(item)
}

/// Catalog menu text for `command_id`. An id missing from the catalog shows
/// the bare id, which the catalog-agreement test turns into a failure.
pub(super) fn command_text(command_id: &str) -> String {
    crate::keymap::metadata_for_str(command_id)
        .map_or(command_id, |meta| meta.menu_label())
        .to_string()
}

/// A row that runs the catalog command `command_id`: its label is the
/// command's menu text, so no surface keeps a private copy (#271).
/// `shortcut` is the active keymap's hint for the same id (empty = none).
pub(super) fn dd_cmd(
    icon: Option<Handle>,
    command_id: &'static str,
    shortcut: &str,
    message: Message,
) -> DropdownEntry<Message> {
    tag_command(
        dd_msg(icon, &command_text(command_id), shortcut, message),
        command_id,
    )
}

/// A row that runs the catalog command `command_id` through the command
/// registry, like the palette and the keymap do (#1727): clicking it closes
/// the menu and dispatches the id with the point the menu was opened at, and
/// with `project` when the row stands on a project. An id that fails to
/// parse gives a greyed row instead of a panic.
pub(super) fn dd_command(
    icon: Option<Handle>,
    command_id: &'static str,
    shortcut: &str,
    project: Option<String>,
) -> DropdownEntry<Message> {
    match crate::keymap::AppCommandId::new(command_id) {
        Ok(id) => dd_cmd(
            icon,
            command_id,
            shortcut,
            Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
                id,
                project,
            })),
        ),
        Err(_) => dd_cmd_disabled(icon, command_id, shortcut),
    }
}

/// [`dd_cmd`] greyed because the command cannot run right now. The row
/// keeps the command's `shortcut` hint in the right-hand column.
pub(super) fn dd_cmd_disabled(
    icon: Option<Handle>,
    command_id: &'static str,
    shortcut: &str,
) -> DropdownEntry<Message> {
    let right = (!shortcut.is_empty()).then_some(shortcut);
    tag_command(
        dd_disabled(icon, &command_text(command_id), right),
        command_id,
    )
}

pub(super) fn tag_command(
    entry: DropdownEntry<Message>,
    command_id: &'static str,
) -> DropdownEntry<Message> {
    match entry {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.command(command_id)),
        other => other,
    }
}

/// A greyed row for a feature that does not exist yet: the same passive
/// row as [`dd_disabled`], plus the shared "planned" tooltip so the row
/// reads as unbuilt rather than broken (#1251).
pub(super) fn dd_planned(
    icon: Option<Handle>,
    label: &str,
    right: Option<&str>,
) -> DropdownEntry<Message> {
    match dd_disabled(icon, label, right) {
        DropdownEntry::Item(item) => {
            DropdownEntry::Item(item.tooltip(crate::active_bar::PLANNED_TOOLTIP))
        }
        other => other,
    }
}

/// A greyed launcher for a submenu that does not exist yet: a planned row
/// with the chevron in the chevron column.
pub(super) fn dd_planned_submenu(icon: Option<Handle>, label: &str) -> DropdownEntry<Message> {
    match dd_planned(icon, label, None) {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.submenu()),
        other => other,
    }
}

/// Save row for the project-root menu — active only when a schematic tab
/// is open or the project metadata is dirty. Label and `shortcut` come from
/// the `save_document` command like every other surface.
pub(super) fn save_entry(enabled: bool, shortcut: &str) -> DropdownEntry<Message> {
    if enabled {
        dd_command(None, "save_document", shortcut, None)
    } else {
        dd_cmd_disabled(None, "save_document", shortcut)
    }
}

/// Submenu launcher row (`Place ›`, `Align ›`, `Add New to Project ›`): a
/// plain item row with the chevron, so it has the same height, padding and
/// type as its neighbours and a flyout lines up with it. Hovering it
/// starts the 200 ms open timer (`ContextMenuMsg::SubmenuHover`), a click
/// opens the flyout at once, and `active` keeps it lit while its flyout is
/// open.
pub(super) fn submenu_launcher(
    icon: Option<Handle>,
    label: &str,
    kind: ContextSubmenu,
    active: bool,
) -> DropdownEntry<Message> {
    let mut item = DropdownItem::new(
        label,
        Message::ContextMenu(ContextMenuMsg::SubmenuOpen(kind)),
    )
    .submenu()
    .hover(
        Message::ContextMenu(ContextMenuMsg::SubmenuHover(kind)),
        Message::ContextMenu(ContextMenuMsg::SubmenuLeave),
    )
    .active(active);
    if let Some(handle) = icon {
        item = item.icon(handle);
    }
    DropdownEntry::Item(item)
}

impl Signex {
    /// Identity of the library leaf at `tree_path` (`[project_idx,
    /// libraries_branch_idx, library_idx]`): the owning project's id plus
    /// the leaf's key, which is the library's resolved `.snxlib` path.
    /// `None` when the path doesn't sit at a library leaf.
    pub(super) fn library_row_ref(&self, tree_path: &[usize]) -> Option<crate::app::LibraryRowRef> {
        let project_idx = *tree_path.first()?;
        let node = signex_widgets::tree_view::get_node(
            self.document_state.panel_ctx.project_tree.as_slice(),
            tree_path,
        )?;
        Some(crate::app::LibraryRowRef {
            project_id: self.document_state.projects.get(project_idx)?.id,
            root: std::path::PathBuf::from(&node.key),
        })
    }

    /// Resolve a keymap command id to its display label under the active
    /// profile. A command the profile does not bind (or unbinds) has no
    /// hint, which every caller renders as an empty label (#1378).
    pub(in crate::app::view) fn keymap_shortcut_label(&self, command_id: &str) -> String {
        self.ui_state
            .active_keymap
            .shortcut_label_for_id(command_id)
    }
}
