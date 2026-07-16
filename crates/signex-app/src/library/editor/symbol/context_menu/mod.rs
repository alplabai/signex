//! Right-click canvas context menu (VIEW) for the symbol editor.
//!
//! Mirrors `library::editor::footprint::context_menu`'s mounting
//! contract 1:1 (window-absolute coords, dismiss-layer overlay — see
//! that module's doc comment for the rationale) but renders through
//! the generic `signex_widgets::active_bar_dropdown` row renderer
//! (already shared by the app-level context menus and every editor's
//! active-bar dropdown — see `app/view/context_menu/items.rs`)
//! instead of a hand-built widget tree. `rows` is the declarative row
//! data this module flattens into `DropdownEntry`s; [`flatten`] is
//! the one place that conversion happens.

mod rows;

use std::path::Path;

use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::app::SymbolEditorState;
use crate::library::editor::symbol::state::SymbolContextSubmenu;
use crate::library::messages::{
    LibraryMessage, PrimitiveEdit, SymbolContextSubmenuMsg, SymbolEditorMsg,
};

pub use rows::{SymbolMenuRow, build_symbol_context_menu_rows};

/// Fixed panel width — list-style menu, not the Filter chip-grid, so a
/// fixed width (matching the footprint context menu's `MENU_WIDTH`)
/// keeps every row's shortcut/chevron column aligned. `pub` so the
/// overlay mount (`app/view/overlays/bars.rs`'s clamping) sizes its
/// screen-edge estimate off the same number instead of a second
/// hardcoded copy.
pub const MENU_WIDTH: f32 = 200.0;

/// Build the right-click context menu card for the active symbol
/// editor. Returns `None` when the menu is closed.
pub fn view_context_menu<'a>(
    editor: &'a SymbolEditorState,
    tokens: &'a ThemeTokens,
    path: &'a Path,
) -> Option<iced::Element<'a, LibraryMessage>> {
    let menu_state = editor.context_menu.as_ref()?;
    let rows = build_symbol_context_menu_rows(editor.primitive(), &editor.selected);
    let entries = flatten(rows, menu_state.open_submenu, path, false);
    Some(signex_widgets::active_bar_dropdown::view(
        entries,
        tokens,
        Some(MENU_WIDTH),
    ))
}

/// Walk the declarative row tree into the flat `Vec<DropdownEntry>`
/// the generic renderer draws — the one place this module converts
/// pure row data into the shared widget vocabulary. A submenu's
/// children render directly below their header (indented via a
/// leading marker — the shared widget has no dedicated indent slot)
/// only while `open_submenu` names that header, matching the
/// footprint context menu's accordion-in-place submenu behaviour
/// (not a hover flyout).
fn flatten(
    rows: Vec<SymbolMenuRow>,
    open_submenu: Option<SymbolContextSubmenu>,
    path: &Path,
    indented: bool,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match row.submenu {
            Some(children) => {
                let is_open = row_submenu_matches(row.id, open_submenu);
                out.push(submenu_header_entry(row.id, row.label, is_open, path));
                if is_open {
                    out.extend(flatten(children, open_submenu, path, true));
                }
            }
            None => out.push(leaf_entry(row, path, indented)),
        }
    }
    out
}

/// Whether `id`'s row is the currently accordion-open submenu. A
/// plain `match` (not a lookup table) is fine — there's exactly one
/// submenu today; a second one adds one more arm here.
fn row_submenu_matches(id: &str, open: Option<SymbolContextSubmenu>) -> bool {
    matches!(
        (id, open),
        ("symbol.place", Some(SymbolContextSubmenu::Place))
    )
}

/// The pure-data submenu id a header row's click should open.
fn submenu_msg_for_id(id: &str) -> Option<SymbolContextSubmenuMsg> {
    match id {
        "symbol.place" => Some(SymbolContextSubmenuMsg::Place),
        _ => None,
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
    id: &'static str,
    label: &'static str,
    is_open: bool,
    path: &Path,
) -> DropdownEntry<LibraryMessage> {
    let toggle = if is_open {
        SymbolEditorMsg::ContextMenuOpenSubmenu(None)
    } else {
        SymbolEditorMsg::ContextMenuOpenSubmenu(submenu_msg_for_id(id))
    };
    let chevron = if is_open { "\u{25BE}" } else { "\u{25B8}" };
    DropdownEntry::Item(DropdownItem::new(label, wrap(path, toggle)).shortcut(chevron))
}

/// A real-action row — wraps `row.msg` in `ContextMenuAction` so the
/// dispatcher applies it and closes the menu in one step, and
/// disables the row (no `on_press`) when `!row.enabled`.
fn leaf_entry(row: SymbolMenuRow, path: &Path, indented: bool) -> DropdownEntry<LibraryMessage> {
    let label = if indented {
        format!("  {}", row.label)
    } else {
        row.label.to_string()
    };
    // `row.msg` is `None` only for submenu headers, already routed to
    // `submenu_header_entry` by `flatten` before reaching here.
    let inner = row
        .msg
        .expect("leaf row (no submenu) always carries a message");
    let action = wrap(path, SymbolEditorMsg::ContextMenuAction(Box::new(inner)));
    DropdownEntry::Item(DropdownItem::new(label, action).disabled(!row.enabled))
}
