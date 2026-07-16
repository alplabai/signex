//! Symbol editor — right-click context menu update logic. Mirrors
//! `library::editor::footprint::updates::context_menu` in structure.
//!
//! `SymbolEditorMsg::ContextMenuAction` (apply the boxed action, then
//! close the menu) is handled directly in `apply_symbol_primitive_edit`
//! rather than here, since it needs to recurse back into the top-level
//! dispatcher — every other context-menu variant is state-only and
//! lives in [`apply_symbol_context_menu`].

use super::{SymEditor, context_submenu_msg_to_state, context_target_msg_to_state};
use crate::library::editor::symbol::state::{SymbolContextMenuState, SymbolContextTarget};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_context_menu(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    use crate::library::editor::symbol::state::SymbolSelection;

    match msg {
        SymbolEditorMsg::ShowContextMenu { x, y, target } => {
            let target = context_target_msg_to_state(target);
            // Altium parity: right-click on a pin/graphic that isn't
            // already the sole selection selects it first, so
            // subsequent menu actions (Delete, Join into Polygon, …)
            // act on the right-clicked item.
            let want = match target {
                SymbolContextTarget::Pin(idx) => Some(SymbolSelection::Pin(idx)),
                SymbolContextTarget::Graphic(idx) => Some(SymbolSelection::Graphic(idx)),
                SymbolContextTarget::Empty => None,
            };
            if let Some(sel) = want
                && editor.selected.as_ref() != Some(&sel)
            {
                editor.selected = Some(sel);
                editor.canvas_cache.clear();
            }
            editor.context_menu = Some(SymbolContextMenuState {
                x,
                y,
                target,
                open_submenu: None,
            });
        }
        SymbolEditorMsg::CloseContextMenu => {
            editor.context_menu = None;
        }
        SymbolEditorMsg::ContextMenuOpenSubmenu(sm) => {
            if let Some(menu) = editor.context_menu.as_mut() {
                menu.open_submenu = sm.map(context_submenu_msg_to_state);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::symbol::state::SymbolSelection;
    use crate::library::messages::{SymbolContextSubmenuMsg, SymbolContextTargetMsg};
    use signex_library::{Symbol, SymbolFile};
    use std::path::PathBuf;

    fn new_editor() -> SymEditor {
        SymEditor::new(
            PathBuf::from("t.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("T")),
        )
    }

    /// Right-click on bare canvas opens the menu at the given coords
    /// with `Empty` target and doesn't touch the current selection.
    #[test]
    fn show_context_menu_on_empty_leaves_selection_untouched() {
        let mut editor = new_editor();
        editor.selected = Some(SymbolSelection::Pin(0));

        apply_symbol_context_menu(
            &mut editor,
            SymbolEditorMsg::ShowContextMenu {
                x: 12.0,
                y: 34.0,
                target: SymbolContextTargetMsg::Empty,
            },
        );

        let menu = editor.context_menu.as_ref().expect("menu opened");
        assert_eq!((menu.x, menu.y), (12.0, 34.0));
        assert_eq!(
            menu.target,
            crate::library::editor::symbol::state::SymbolContextTarget::Empty
        );
        assert_eq!(editor.selected, Some(SymbolSelection::Pin(0)));
    }

    /// Right-click on a graphic that isn't already the sole selection
    /// selects it first (Altium parity), then opens the menu.
    #[test]
    fn show_context_menu_on_graphic_selects_it_first() {
        let mut editor = new_editor();
        editor.selected = None;

        apply_symbol_context_menu(
            &mut editor,
            SymbolEditorMsg::ShowContextMenu {
                x: 0.0,
                y: 0.0,
                target: SymbolContextTargetMsg::Graphic(3),
            },
        );

        assert_eq!(editor.selected, Some(SymbolSelection::Graphic(3)));
        assert!(editor.context_menu.is_some());
    }

    /// Right-click on a graphic that's already the sole selection
    /// doesn't disturb it (idempotent — no-op selection write).
    #[test]
    fn show_context_menu_on_already_selected_graphic_is_idempotent() {
        let mut editor = new_editor();
        editor.selected = Some(SymbolSelection::Graphic(2));

        apply_symbol_context_menu(
            &mut editor,
            SymbolEditorMsg::ShowContextMenu {
                x: 0.0,
                y: 0.0,
                target: SymbolContextTargetMsg::Graphic(2),
            },
        );

        assert_eq!(editor.selected, Some(SymbolSelection::Graphic(2)));
    }

    #[test]
    fn close_context_menu_clears_state() {
        let mut editor = new_editor();
        apply_symbol_context_menu(
            &mut editor,
            SymbolEditorMsg::ShowContextMenu {
                x: 0.0,
                y: 0.0,
                target: SymbolContextTargetMsg::Empty,
            },
        );
        assert!(editor.context_menu.is_some());

        apply_symbol_context_menu(&mut editor, SymbolEditorMsg::CloseContextMenu);

        assert!(editor.context_menu.is_none());
    }

    /// Opening a submenu sets `open_submenu`; opening `None` collapses
    /// it again (the header row's toggle-closed click).
    #[test]
    fn context_menu_open_submenu_toggles() {
        let mut editor = new_editor();
        apply_symbol_context_menu(
            &mut editor,
            SymbolEditorMsg::ShowContextMenu {
                x: 0.0,
                y: 0.0,
                target: SymbolContextTargetMsg::Empty,
            },
        );

        apply_symbol_context_menu(
            &mut editor,
            SymbolEditorMsg::ContextMenuOpenSubmenu(Some(SymbolContextSubmenuMsg::Place)),
        );
        assert_eq!(
            editor.context_menu.as_ref().unwrap().open_submenu,
            Some(crate::library::editor::symbol::state::SymbolContextSubmenu::Place)
        );

        apply_symbol_context_menu(&mut editor, SymbolEditorMsg::ContextMenuOpenSubmenu(None));
        assert_eq!(editor.context_menu.as_ref().unwrap().open_submenu, None);
    }

    /// `ContextMenuAction` (routed at the top-level dispatcher, not
    /// here) applies its boxed action and closes the menu in one step.
    #[test]
    fn context_menu_action_applies_inner_and_closes_menu() {
        let mut editor = new_editor();
        editor
            .primitive_mut()
            .pins
            .push(signex_library::SymbolPin::new("1", "IN"));
        editor.selected = Some(SymbolSelection::Pin(0));
        apply_symbol_context_menu(
            &mut editor,
            SymbolEditorMsg::ShowContextMenu {
                x: 0.0,
                y: 0.0,
                target: SymbolContextTargetMsg::Empty,
            },
        );
        assert!(editor.context_menu.is_some());

        super::super::apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::ContextMenuAction(Box::new(SymbolEditorMsg::DeleteSelected)),
        );

        assert!(editor.context_menu.is_none(), "menu closes on action");
        assert!(editor.primitive().pins.is_empty(), "inner action applied");
    }
}
