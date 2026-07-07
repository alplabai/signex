//! Symbol editor — toolbar / active-bar / selection-filter UI update logic.

use super::SymEditor;
use crate::library::messages::{PrimitiveEditorMsg, SymbolToolMsg};

pub(super) fn apply_symbol_ui(editor: &mut SymEditor, msg: PrimitiveEditorMsg) {
    use crate::library::editor::symbol::canvas::SymbolTool;
    match msg {
        PrimitiveEditorMsg::SymbolSetTool(tool) => {
            editor.tool = match tool {
                SymbolToolMsg::Select => SymbolTool::Select,
                SymbolToolMsg::AddPin => SymbolTool::AddPin,
                SymbolToolMsg::PlaceRectangle => SymbolTool::PlaceRectangle,
                SymbolToolMsg::PlaceLine => SymbolTool::PlaceLine,
                SymbolToolMsg::PlaceCircle => SymbolTool::PlaceCircle,
                SymbolToolMsg::PlaceArc => SymbolTool::PlaceArc,
                SymbolToolMsg::PlaceText => SymbolTool::PlaceText,
            };
            editor.active_bar_menu = None;
        }
        PrimitiveEditorMsg::SymbolToggleActiveBarMenu(menu) => {
            editor.active_bar_menu = match editor.active_bar_menu {
                Some(m) if m == menu => None,
                _ => Some(menu),
            };
        }
        PrimitiveEditorMsg::SymbolCloseActiveBarMenu => {
            editor.active_bar_menu = None;
        }
        PrimitiveEditorMsg::SymbolActiveBarStub(label) => {
            crate::diagnostics::log_info(format!(
                "Symbol active bar: {label} — coming soon (SchLib Altium parity)"
            ));
            editor.active_bar_menu = None;
        }
        PrimitiveEditorMsg::SymbolToggleSelectionFilter(kind) => {
            editor.selection_filter.toggle(kind);
            editor.canvas_cache.clear();
        }
        _ => {}
    }
}
