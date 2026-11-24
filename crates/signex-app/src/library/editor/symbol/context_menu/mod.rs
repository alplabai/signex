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
use crate::keymap::CompiledKeymap;
use crate::library::editor::symbol::state::SymbolContextSubmenu;
use crate::library::messages::{
    LibraryMessage, PrimitiveEdit, SymbolContextSubmenuMsg, SymbolEditorMsg,
};

pub use rows::{PLACE_TOOLS, RowIcon, RowKeys, SymbolMenuRow, build_symbol_context_menu_rows};

/// Fixed panel width in px, as the mockup's menu: a fixed width keeps
/// every row's shortcut and chevron column aligned.
pub const MENU_WIDTH: f32 = 230.0;

/// Theme the icon handles are built for. The card tints every icon with
/// its own ink, so the handle's theme tint is never seen.
const ICON_THEME: ThemeId = ThemeId::DEFAULT;

/// Build the right-click context menu card for the active symbol
/// editor. Returns `None` when the menu is closed. A command row's key
/// comes from `keymap`.
pub fn view_context_menu<'a>(
    editor: &'a SymbolDocument,
    tokens: &'a ThemeTokens,
    path: &'a Path,
    keymap: &CompiledKeymap,
    i18n: &I18n,
) -> Option<iced::Element<'a, LibraryMessage>> {
    let menu_state = editor.context_menu.as_ref()?;
    let rows = build_symbol_context_menu_rows(editor.primitive(), editor.view(), &editor.selected);
    let text = RowText { keymap, i18n };
    let entries = flatten(rows, menu_state.open_submenu, path, false, &text);
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
    text: &RowText<'_>,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let mut out = Vec::with_capacity(rows.len());
    for mut row in rows {
        if row.divider_before {
            out.push(DropdownEntry::Separator);
        }
        match row.submenu.take() {
            Some(children) => {
                let is_open = submenu_for_id(row.id).is_some_and(|(s, _)| open_submenu == Some(s));
                out.push(submenu_header_entry(&row, is_open, path, text.i18n));
                if is_open {
                    out.extend(flatten(children, open_submenu, path, true, text));
                }
            }
            None => out.push(leaf_entry(row, path, indented, text)),
        }
    }
    out
}

/// What a row's texts are read from: the catalog and the active keymap.
struct RowText<'a> {
    keymap: &'a CompiledKeymap,
    i18n: &'a I18n,
}

impl RowText<'_> {
    /// The key shown for `keys`; empty when the keymap binds no key.
    fn keys(&self, keys: RowKeys) -> String {
        match keys {
            RowKeys::Canvas(id) => self.i18n.message_plain(id, &[]),
            RowKeys::Command(command) => self.keymap.shortcut_label_for_id(command),
        }
    }
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
    text: &RowText<'_>,
) -> DropdownEntry<LibraryMessage> {
    let name = text.i18n.message_plain(row.label, &[]);
    let label = if indented { format!("  {name}") } else { name };
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
    if let Some(keys) = row.keys.map(|keys| text.keys(keys))
        && !keys.is_empty()
    {
        item = item.shortcut(keys);
    }
    DropdownEntry::Item(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{
        AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutProfile,
        ShortcutProfileKind, ShortcutTrigger,
    };

    /// A keymap that binds `delete_selection` to `trigger`, or nothing.
    fn keymap(trigger: Option<&str>) -> CompiledKeymap {
        let action = ShortcutBindingAction::Command(AppCommandId::new("delete_selection").unwrap());
        CompiledKeymap::compile(&ShortcutProfile {
            id: "t".to_string(),
            name: "T".to_string(),
            kind: ShortcutProfileKind::Custom,
            schema_version: 1,
            description: None,
            base_profile: None,
            bindings: trigger
                .map(|trigger| ShortcutBinding {
                    action,
                    context: ShortcutContext::Global,
                    triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                    platform: None,
                })
                .into_iter()
                .collect(),
        })
    }

    /// The Delete row shows the key the active keymap binds to Delete
    /// Selection, and no key when the keymap binds none.
    #[test]
    fn the_delete_row_key_comes_from_the_keymap() {
        let sym = signex_library::Symbol::empty("T");
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &None,
        );
        let delete = rows.iter().find(|r| r.id == "symbol.delete").unwrap();
        let keys = delete.keys.expect("the Delete row shows a key");
        let i18n = I18n::en_us();
        let bound = keymap(Some("F4"));
        let text = RowText {
            keymap: &bound,
            i18n: &i18n,
        };
        assert_eq!(text.keys(keys), "F4");
        let unbound = keymap(None);
        let text = RowText {
            keymap: &unbound,
            i18n: &i18n,
        };
        assert_eq!(text.keys(keys), "", "an unbound command shows no key");
    }
}
