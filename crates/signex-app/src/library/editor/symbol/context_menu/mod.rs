//! Right-click canvas context menu (VIEW) for the symbol editor.
//!
//! Mounted like `library::editor::footprint::context_menu` (window-absolute
//! coords, dismiss-layer overlay) and drawn by the same shared
//! `signex_widgets::active_bar_dropdown` row renderer, so both editors'
//! menus have one icon column, aligned labels and right-aligned keys.
//! `rows` is the declarative row data; [`flatten`] is the one place it
//! becomes `DropdownEntry`s.

mod rows;

use std::path::Path;

use iced::widget::svg::Handle;
use signex_i18n::I18n;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::app::SymbolDocument;
use crate::icons as ic;
use crate::library::editor::symbol::state::SymbolContextSubmenu;
use crate::library::messages::{
    LibraryMessage, PrimitiveEdit, SymbolContextSubmenuMsg, SymbolEditorMsg,
};

pub use rows::{PLACE_TOOLS, RowIcon, SymbolMenuRow, build_symbol_context_menu_rows};

/// Fixed panel width in px, as the mockup's menu: a fixed width keeps
/// every row's shortcut and chevron column aligned.
pub const MENU_WIDTH: f32 = 230.0;

/// Theme the icon handles are built for. The card tints every icon with
/// its own ink, so the handle's theme tint is never seen.
const ICON_THEME: ThemeId = ThemeId::DEFAULT;

/// Build the right-click context menu card for the active symbol
/// editor. Returns `None` when the menu is closed.
pub fn view_context_menu<'a>(
    editor: &'a SymbolDocument,
    tokens: &'a ThemeTokens,
    path: &'a Path,
    i18n: &I18n,
) -> Option<iced::Element<'a, LibraryMessage>> {
    let menu_state = editor.context_menu.as_ref()?;
    let rows = build_symbol_context_menu_rows(editor.primitive(), editor.view(), &editor.selected);
    let entries = flatten(rows, menu_state.open_submenu, path, false, i18n);
    Some(signex_widgets::active_bar_dropdown::view(
        entries,
        tokens,
        Some(MENU_WIDTH),
    ))
}

/// Walk the declarative row tree into the flat `Vec<DropdownEntry>`
/// the generic renderer draws. A submenu's children render directly
/// below their header (indented via a leading marker — the shared
/// widget has no indent slot) only while `open_submenu` names that
/// header: an accordion in place, not a hover flyout.
fn flatten(
    rows: Vec<SymbolMenuRow>,
    open_submenu: Option<SymbolContextSubmenu>,
    path: &Path,
    indented: bool,
    i18n: &I18n,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let mut out = Vec::with_capacity(rows.len());
    for mut row in rows {
        if row.divider_before {
            out.push(DropdownEntry::Separator);
        }
        match row.submenu.take() {
            Some(children) => {
                let is_open = submenu_for_id(row.id).is_some_and(|(s, _)| open_submenu == Some(s));
                out.push(submenu_header_entry(&row, is_open, path, i18n));
                if is_open {
                    out.extend(flatten(children, open_submenu, path, true, i18n));
                }
            }
            None => out.push(leaf_entry(row, path, indented, i18n)),
        }
    }
    out
}

/// The submenu a header row stands for, as state and as message.
fn submenu_for_id(id: &str) -> Option<(SymbolContextSubmenu, SymbolContextSubmenuMsg)> {
    match id {
        "symbol.place" => Some((SymbolContextSubmenu::Place, SymbolContextSubmenuMsg::Place)),
        "symbol.align" => Some((SymbolContextSubmenu::Align, SymbolContextSubmenuMsg::Align)),
        _ => None,
    }
}

fn icon_handle(icon: RowIcon) -> Handle {
    match icon {
        RowIcon::Place => ic::icon_dd_place_menu(ICON_THEME),
        RowIcon::Rotate => ic::icon_dd_rotate(ICON_THEME),
        RowIcon::Align => ic::icon_dd_align_menu(ICON_THEME),
        RowIcon::SelectAll => ic::icon_dd_select_all(ICON_THEME),
        RowIcon::Delete => ic::icon_dd_delete(ICON_THEME),
    }
}

fn wrap(path: &Path, msg: SymbolEditorMsg) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(msg),
    }
}

/// A submenu-launcher row — toggles `open_submenu` on click. Not
/// wrapped in `ContextMenuAction`: opening/closing a submenu must not
/// close the whole popover.
fn submenu_header_entry(
    row: &SymbolMenuRow,
    is_open: bool,
    path: &Path,
    i18n: &I18n,
) -> DropdownEntry<LibraryMessage> {
    let toggle = if is_open {
        SymbolEditorMsg::ContextMenuOpenSubmenu(None)
    } else {
        SymbolEditorMsg::ContextMenuOpenSubmenu(submenu_for_id(row.id).map(|(_, msg)| msg))
    };
    let mut item = DropdownItem::new(i18n.message_plain(row.label, &[]), wrap(path, toggle))
        .submenu()
        .active(is_open);
    if let Some(icon) = row.icon {
        item = item.icon(icon_handle(icon));
    }
    DropdownEntry::Item(item)
}

/// A real-action row — wraps `row.msg` in `ContextMenuAction` so the
/// dispatcher applies it and closes the menu in one step, and
/// disables the row (no `on_press`) when `!row.enabled`.
fn leaf_entry(
    row: SymbolMenuRow,
    path: &Path,
    indented: bool,
    i18n: &I18n,
) -> DropdownEntry<LibraryMessage> {
    let text = i18n.message_plain(row.label, &[]);
    let label = if indented { format!("  {text}") } else { text };
    // `row.msg` is `None` only for submenu headers, already routed to
    // `submenu_header_entry` by `flatten` before reaching here.
    let inner = row
        .msg
        .expect("leaf row (no submenu) always carries a message");
    let action = wrap(path, SymbolEditorMsg::ContextMenuAction(Box::new(inner)));
    let mut item = DropdownItem::new(label, action).disabled(!row.enabled);
    if let Some(icon) = row.icon {
        item = item.icon(icon_handle(icon));
    }
    if let Some(keys) = row.keys {
        item = item.shortcut(i18n.message_plain(keys, &[]));
    }
    DropdownEntry::Item(item)
}
