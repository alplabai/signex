//! Symbol editor — toolbar / active-bar / selection-filter UI update logic.

use super::{SymEditor, commit_or_discard_polygon};
use crate::library::messages::{SymbolEditorMsg, SymbolToolMsg};

pub(super) fn apply_symbol_ui(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    use crate::library::editor::symbol::canvas::SymbolTool;
    match msg {
        SymbolEditorMsg::SetTool(tool) => {
            let new_tool = match tool {
                SymbolToolMsg::Select => SymbolTool::Select,
                SymbolToolMsg::AddPin => SymbolTool::AddPin,
                SymbolToolMsg::PlaceRectangle => SymbolTool::PlaceRectangle,
                SymbolToolMsg::PlaceLine => SymbolTool::PlaceLine,
                SymbolToolMsg::PlaceCircle => SymbolTool::PlaceCircle,
                SymbolToolMsg::PlaceArc => SymbolTool::PlaceArc,
                SymbolToolMsg::PlaceText => SymbolTool::PlaceText,
                SymbolToolMsg::PlacePolygon => SymbolTool::PlacePolygon,
            };
            // Footprint parity ("leaving Place Polygon commits (>= 3
            // vertices) / discards it (< 3)"): flush the in-flight
            // stash HERE, synchronously, in the same handler that
            // changes `editor.tool` — not deferred to a later canvas
            // event. `editor.polygon_vertices` lives on this
            // document's own editor model, so there's no cross-tab
            // window: switching to a footprint/schematic tab or a
            // different `.snxsym` tab can never see, let alone
            // mis-commit, another document's in-flight stash.
            if editor.tool == SymbolTool::PlacePolygon
                && new_tool != SymbolTool::PlacePolygon
                && !editor.polygon_vertices.is_empty()
            {
                commit_or_discard_polygon(editor);
            }
            editor.tool = new_tool;
            editor.active_bar_menu = None;
        }
        SymbolEditorMsg::ToggleActiveBarMenu(menu) => {
            editor.active_bar_menu = match editor.active_bar_menu {
                Some(m) if m == menu => None,
                _ => Some(menu),
            };
        }
        SymbolEditorMsg::CloseActiveBarMenu => {
            editor.active_bar_menu = None;
        }
        SymbolEditorMsg::ActiveBarStub(label) => {
            crate::diagnostics::log_info(format!(
                "Symbol active bar: {label} — coming soon (SchLib Altium parity)"
            ));
            editor.active_bar_menu = None;
        }
        SymbolEditorMsg::ToggleSelectionFilter(kind) => {
            editor.selection_filter.toggle(kind);
            editor.canvas_cache.clear();
        }
        _ => {}
    }
}

