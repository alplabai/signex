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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::symbol::canvas::SymbolTool;
    use crate::library::editor::symbol::updates::apply_symbol_primitive_edit;
    use signex_library::{Symbol, SymbolFile};
    use std::path::PathBuf;

    fn new_editor(name: &str) -> SymEditor {
        SymEditor::new(
            PathBuf::from(format!("{name}.snxsym")),
            SymbolFile::from_symbol(Symbol::empty(name)),
        )
    }

    /// Footprint parity: switching the tool away from `PlacePolygon`
    /// with >= 3 vertices collected commits synchronously, in the
    /// same handler that changes `editor.tool` — no separate message
    /// round-trip.
    #[test]
    fn set_tool_away_from_polygon_commits_synchronously() {
        let mut editor = new_editor("A");
        editor.tool = SymbolTool::PlacePolygon;
        editor.polygon_vertices = vec![(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)];

        apply_symbol_ui(&mut editor, SymbolEditorMsg::SetTool(SymbolToolMsg::Select));

        assert_eq!(editor.tool, SymbolTool::Select);
        assert_eq!(
            editor.primitive().graphics.len(),
            1,
            "stash committed on tool switch"
        );
        assert_eq!(editor.undo_snapshots.len(), 1);
        assert!(editor.polygon_vertices.is_empty());
    }

    /// Switching away from `PlacePolygon` with < 3 vertices discards
    /// the stash — no graphic, no undo entry.
    #[test]
    fn set_tool_away_from_polygon_discards_short_stash() {
        let mut editor = new_editor("A");
        editor.tool = SymbolTool::PlacePolygon;
        editor.polygon_vertices = vec![(0.0, 0.0), (4.0, 0.0)];

        apply_symbol_ui(&mut editor, SymbolEditorMsg::SetTool(SymbolToolMsg::Select));

        assert!(editor.primitive().graphics.is_empty());
        assert_eq!(editor.undo_snapshots.len(), 0);
        assert!(editor.polygon_vertices.is_empty());
    }

    /// Regression for the cross-tab corruption class this fix
    /// replaces (the vertex stash used to live on the canvas
    /// `Program::State`, which iced reuses across `.snxsym` tabs).
    /// The stash now lives on each document's own `SymbolEditorState`,
    /// so two independent editors — standing in for two open tabs —
    /// never share it: placing vertices and switching tools on editor
    /// A must not create, touch, or leak into editor B in any way.
    #[test]
    fn polygon_stash_never_crosses_between_two_editors() {
        let mut editor_a = new_editor("A");
        let mut editor_b = new_editor("B");

        editor_a.tool = SymbolTool::PlacePolygon;
        apply_symbol_primitive_edit(
            &mut editor_a,
            SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor_a,
            SymbolEditorMsg::PolygonClick { x: 4.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor_a,
            SymbolEditorMsg::PolygonClick { x: 2.0, y: 3.0 },
        );

        // Simulate the bug scenario's "click tab B" — but B is now a
        // wholly separate `SymbolEditorState`, not a shared canvas
        // widget slot, so there's nothing for B's first event to
        // observe from A.
        apply_symbol_primitive_edit(
            &mut editor_b,
            SymbolEditorMsg::SetTool(SymbolToolMsg::Select),
        );

        assert!(
            editor_b.polygon_vertices.is_empty(),
            "B never saw A's in-flight stash"
        );
        assert!(
            editor_b.primitive().graphics.is_empty(),
            "B's symbol stays untouched"
        );
        assert_eq!(
            editor_b.undo_snapshots.len(),
            0,
            "B never pushed an undo entry"
        );

        // A's own (still-pending) tool switch now correctly commits
        // A's — and only A's — polygon.
        apply_symbol_primitive_edit(
            &mut editor_a,
            SymbolEditorMsg::SetTool(SymbolToolMsg::Select),
        );
        assert_eq!(editor_a.primitive().graphics.len(), 1);
        assert_eq!(editor_a.undo_snapshots.len(), 1);
    }
}
