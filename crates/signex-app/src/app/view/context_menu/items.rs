//! Shared context-menu primitives — the pure `DropdownEntry` builders
//! (`dd_kb`, `dd_msg`, `dd_disabled`, `save_entry`), the hover-driven
//! submenu launcher (`submenu_launcher`, an owned `Custom` row), and the
//! two `&self` lookups the menu builders need (`keymap_shortcut_label`,
//! `library_node_path_from_tree`).
//!
//! Data-to-view (#269): every context menu is a `Vec<DropdownEntry>` that
//! the shared `signex_widgets::active_bar_dropdown` widget renders, so the
//! canvas / project-tree / tab menus share ONE row renderer with the
//! schematic + footprint active bars (ADR-0003). The widget owns all row
//! chrome (icon column, label, shortcut, hover, disabled greying), so the
//! hand-built row buttons that used to live here are gone.

use super::*;

use iced::widget::svg::Handle;
use signex_types::theme::ThemeTokens;
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
    DropdownEntry::Item(DropdownItem {
        label: label.to_string(),
        icon,
        checked: false,
        shortcut: right.map(|s| s.to_string()),
        disabled: true,
        on_press: None,
    })
}

/// Save row for the project-root menu — active only when a schematic tab
/// is open or the project metadata is dirty. Altium's right-click menus
/// don't surface keyboard shortcuts, so the shortcut column stays empty
/// even though Ctrl+S still fires `MenuMessage::Save` globally.
pub(super) fn save_entry(enabled: bool) -> DropdownEntry<Message> {
    if enabled {
        dd_msg(
            None,
            "Save",
            "",
            Message::Menu(crate::menu_bar::MenuMessage::Save),
        )
    } else {
        dd_disabled(None, "Save", None)
    }
}

/// Submenu launcher row (`Place ›`, `Align ›`, `Add New to Project ›`).
///
/// This is the one context-menu row the shared widget can't express as a
/// plain `Item`: it needs `mouse_area` on_enter/on_exit so the 200 ms
/// hover timer (`ContextMenuMsg::SubmenuTickHover`) can open the flyout
/// without a click, plus an active-state highlight so the user can see
/// which submenu is open. It therefore rides the widget's `Custom`
/// escape hatch as an owned `Element<'static>`. The row is styled to
/// match the widget's `Item` rows (20 px icon column, 13 pt label,
/// `[5, 12]` padding, hover background) so it sits flush with its
/// neighbours; the `›` chevron renders right-aligned at the unified
/// `SUBMENU_ARROW_SIZE`.
pub(super) fn submenu_launcher(
    tokens: &ThemeTokens,
    icon: Option<Handle>,
    label: &str,
    kind: ContextSubmenu,
    active: bool,
) -> DropdownEntry<Message> {
    let text_c = crate::styles::ti(tokens.text);
    let hover_c = crate::styles::ti(tokens.hover);
    let active_bg = crate::styles::ti(tokens.selection);
    let arrow_c = crate::styles::ti(tokens.text_secondary);

    let mut row = iced::widget::row![]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    match icon {
        Some(handle) => {
            row = row.push(iced::widget::svg(handle).width(20).height(20).style(
                move |_: &iced::Theme, _| iced::widget::svg::Style {
                    color: Some(text_c),
                },
            ));
        }
        None => {
            row = row.push(iced::widget::Space::new().width(iced::Length::Fixed(20.0)));
        }
    }
    row = row
        .push(
            iced::widget::text(label.to_string())
                .size(13)
                .color(text_c)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(
            iced::widget::text(SUBMENU_ARROW.to_string())
                .size(SUBMENU_ARROW_SIZE)
                .color(arrow_c),
        );

    let btn = iced::widget::button(
        iced::widget::container(row)
            .padding([5, 12])
            .width(Length::Fill),
    )
    .padding(0)
    .on_press(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(kind)))
    .style(
        move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                _ if active => Some(iced::Background::Color(active_bg)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border::default(),
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        },
    );

    // mouse_area gives the hover timer its on_enter / on_exit hooks so
    // the flyout opens after 200 ms without the user clicking.
    let element: iced::Element<'static, Message> = iced::widget::mouse_area(btn)
        .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuHover(kind)))
        .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeave))
        .into();
    DropdownEntry::Custom(element)
}

impl Signex {
    /// Resolve the on-disk `.snxlib` path for a project-tree library
    /// node click. The tree path under the `Libraries` group is
    /// `[project_idx, libraries_branch_idx, library_idx]` — the
    /// matching project's `LibraryNodeInfo[library_idx].root` carries
    /// the absolute path. Returns `None` when the path doesn't sit at
    /// a library leaf (defensive — `view_project_tree_context_menu`
    /// already gates on the library role, but the icon-by-depth
    /// detection there isn't strict enough to skip the lookup).
    pub(super) fn library_node_path_from_tree(
        &self,
        tree_path: &[usize],
    ) -> Option<std::path::PathBuf> {
        let project_idx = *tree_path.first()?;
        let library_idx = *tree_path.get(2)?;
        self.document_state
            .panel_ctx
            .projects
            .get(project_idx)?
            .libraries
            .get(library_idx)
            .map(|lib| lib.root.clone())
    }

    /// Resolve a keymap command id to its display label under the active
    /// profile, falling back to `fallback` when the command is unbound.
    pub(in crate::app::view) fn keymap_shortcut_label(
        &self,
        command_id: &str,
        fallback: &str,
    ) -> String {
        crate::keymap::AppCommandId::new(command_id)
            .ok()
            .and_then(|command| self.ui_state.active_keymap.shortcut_label(&command))
            .unwrap_or_else(|| fallback.to_string())
    }
}
