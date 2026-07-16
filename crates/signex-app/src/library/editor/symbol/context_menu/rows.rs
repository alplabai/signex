//! Pure, declarative row data for the symbol-editor right-click
//! context menu (data-to-menu, mirrors the app-level context-menu
//! layer's `Vec<DropdownEntry>` pattern in
//! `app/view/context_menu/items.rs`).
//!
//! [`build_symbol_context_menu_rows`] computes WHAT the menu contains
//! as a `Vec<SymbolMenuRow>` — a pure function of the active symbol's
//! graphics plus the current selection, no iced dependency, no
//! rendering. `super::flatten` is the one place that walks this data
//! into widgets.

use crate::library::editor::symbol::state::{SymbolSelection, selection_is_join_eligible};
use crate::library::messages::{SymbolEditorMsg, SymbolSelectionMsg, SymbolToolMsg};
use signex_library::Symbol;

/// One row of the symbol context menu. `id` is a stable, kebab-case,
/// `symbol.`-namespaced command id (the command-registry epic will
/// index these — see `menu_bar` / keymap catalog for the sibling
/// convention). `msg` is the message this row fires on click; `None`
/// for a submenu header, which instead toggles `submenu`'s visibility.
/// `submenu`, when present, holds the rows shown in place directly
/// below this row while it's the open submenu (accordion — mirrors
/// the footprint context menu's Place ▸ / Selection ▸ / View ▸
/// expand-in-place behaviour), not a hover flyout.
pub struct SymbolMenuRow {
    pub id: &'static str,
    pub label: &'static str,
    pub enabled: bool,
    pub msg: Option<SymbolEditorMsg>,
    pub submenu: Option<Vec<SymbolMenuRow>>,
}

impl SymbolMenuRow {
    fn item(id: &'static str, label: &'static str, enabled: bool, msg: SymbolEditorMsg) -> Self {
        Self {
            id,
            label,
            enabled,
            msg: Some(msg),
            submenu: None,
        }
    }

    fn submenu(id: &'static str, label: &'static str, children: Vec<SymbolMenuRow>) -> Self {
        Self {
            id,
            label,
            enabled: true,
            msg: None,
            submenu: Some(children),
        }
    }
}

/// Build the full row tree for the current selection. Every row is
/// present unconditionally — selection-awareness lives entirely in
/// `enabled` (not row presence), so callers/tests get a stable id
/// list regardless of selection state; the renderer greys out /
/// disables clicks on a `enabled: false` row.
pub fn build_symbol_context_menu_rows(
    sym: &Symbol,
    active_part: u8,
    selected: &Option<SymbolSelection>,
) -> Vec<SymbolMenuRow> {
    vec![
        place_submenu(),
        SymbolMenuRow::item(
            "symbol.join-into-polygon",
            "Join into Polygon",
            selection_is_join_eligible(sym, active_part, selected),
            SymbolEditorMsg::JoinSelectionIntoPolygon,
        ),
        SymbolMenuRow::item(
            "symbol.delete",
            "Delete",
            selected.is_some(),
            SymbolEditorMsg::DeleteSelected,
        ),
        SymbolMenuRow::item(
            "symbol.select-all",
            "Select All",
            true,
            SymbolEditorMsg::Select(SymbolSelectionMsg::All),
        ),
        SymbolMenuRow::item(
            "symbol.deselect-all",
            "Deselect All",
            selected.is_some(),
            SymbolEditorMsg::Deselect,
        ),
        SymbolMenuRow::item(
            "symbol.fit-to-window",
            "Fit to Window",
            true,
            SymbolEditorMsg::Fit,
        ),
    ]
}

/// `(id, label, tool)` for every `Place ▸` row — one per canvas
/// placement tool, mirroring the SchLib Place menu / toolbar tool set
/// (`SymbolTool`).
const PLACE_TOOLS: &[(&str, &str, SymbolToolMsg)] = &[
    ("symbol.place-pin", "Pin", SymbolToolMsg::AddPin),
    ("symbol.place-line", "Line", SymbolToolMsg::PlaceLine),
    (
        "symbol.place-rectangle",
        "Rectangle",
        SymbolToolMsg::PlaceRectangle,
    ),
    ("symbol.place-circle", "Circle", SymbolToolMsg::PlaceCircle),
    ("symbol.place-arc", "Arc", SymbolToolMsg::PlaceArc),
    ("symbol.place-text", "Text", SymbolToolMsg::PlaceText),
    (
        "symbol.place-polygon",
        "Polygon",
        SymbolToolMsg::PlacePolygon,
    ),
];

/// `Place ▸` submenu built from [`PLACE_TOOLS`]. Always enabled:
/// switching the active tool never depends on selection.
fn place_submenu() -> SymbolMenuRow {
    let children = PLACE_TOOLS
        .iter()
        .map(|&(id, label, tool)| {
            SymbolMenuRow::item(id, label, true, SymbolEditorMsg::SetTool(tool))
        })
        .collect();
    SymbolMenuRow::submenu("symbol.place", "Place", children)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{SymbolGraphic, SymbolGraphicKind};

    fn row_ids(rows: &[SymbolMenuRow]) -> Vec<&'static str> {
        rows.iter().map(|r| r.id).collect()
    }

    fn find<'a>(rows: &'a [SymbolMenuRow], id: &str) -> &'a SymbolMenuRow {
        rows.iter().find(|r| r.id == id).unwrap_or_else(|| {
            panic!("row {id:?} missing from {:?}", row_ids(rows));
        })
    }

    /// The top-level id list is stable — the exact "full set" the
    /// task names — regardless of selection state.
    #[test]
    fn top_level_ids_are_stable() {
        let sym = Symbol::empty("T");
        let rows = build_symbol_context_menu_rows(&sym, 1, &None);
        assert_eq!(
            row_ids(&rows),
            vec![
                "symbol.place",
                "symbol.join-into-polygon",
                "symbol.delete",
                "symbol.select-all",
                "symbol.deselect-all",
                "symbol.fit-to-window",
            ]
        );
        let place = find(&rows, "symbol.place");
        let children = place.submenu.as_ref().expect("Place is a submenu");
        assert_eq!(
            row_ids(children),
            vec![
                "symbol.place-pin",
                "symbol.place-line",
                "symbol.place-rectangle",
                "symbol.place-circle",
                "symbol.place-arc",
                "symbol.place-text",
                "symbol.place-polygon",
            ]
        );
    }

    /// Empty selection: Join into Polygon, Delete, and Deselect All
    /// are disabled; Select All and Fit to Window stay enabled.
    #[test]
    fn empty_selection_disables_selection_dependent_rows() {
        let sym = Symbol::empty("T");
        let rows = build_symbol_context_menu_rows(&sym, 1, &None);

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(!find(&rows, "symbol.delete").enabled);
        assert!(!find(&rows, "symbol.deselect-all").enabled);
        assert!(find(&rows, "symbol.select-all").enabled);
        assert!(find(&rows, "symbol.fit-to-window").enabled);
        assert!(find(&rows, "symbol.place").enabled);
    }

    /// A selection of only Line graphics enables Join into Polygon.
    #[test]
    fn line_only_selection_enables_join() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [4.0, 0.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [4.0, 0.0],
                to: [4.0, 4.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        let selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1],
        });
        let rows = build_symbol_context_menu_rows(&sym, 1, &selected);

        assert!(find(&rows, "symbol.join-into-polygon").enabled);
        assert!(find(&rows, "symbol.delete").enabled);
        assert!(find(&rows, "symbol.deselect-all").enabled);
    }

    /// A single selected Line disables Join into Polygon — it can
    /// never close on its own — while Delete stays enabled.
    #[test]
    fn single_line_selection_disables_join_but_not_delete() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [4.0, 0.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        let selected = Some(SymbolSelection::Graphic(0));
        let rows = build_symbol_context_menu_rows(&sym, 1, &selected);

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(find(&rows, "symbol.delete").enabled);
    }

    /// A single selected Arc, by contrast, enables Join into Polygon —
    /// a sufficiently large sweep can self-close.
    #[test]
    fn single_arc_selection_enables_join() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Arc {
                center: [0.0, 0.0],
                radius: 2.0,
                start_deg: 0.0,
                end_deg: 270.0,
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        let selected = Some(SymbolSelection::Graphic(0));
        let rows = build_symbol_context_menu_rows(&sym, 1, &selected);

        assert!(find(&rows, "symbol.join-into-polygon").enabled);
    }

    /// `SymbolSelection::All` resolves to every visible graphic, so a
    /// 4-line ring selected via All enables Join into Polygon exactly
    /// like the equivalent `Multiple` selection would.
    #[test]
    fn all_selection_with_a_joinable_ring_enables_join() {
        let mut sym = Symbol::empty("T");
        for (from, to) in [
            ([0.0, 0.0], [4.0, 0.0]),
            ([4.0, 0.0], [4.0, 4.0]),
            ([4.0, 4.0], [0.0, 4.0]),
            ([0.0, 4.0], [0.0, 0.0]),
        ] {
            sym.graphics.push(SymbolGraphic {
                kind: SymbolGraphicKind::Line { from, to },
                stroke_width: 0.15,
                fill: None,
                part_number: 0,
            });
        }
        let selected = Some(SymbolSelection::All);
        let rows = build_symbol_context_menu_rows(&sym, 1, &selected);

        assert!(find(&rows, "symbol.join-into-polygon").enabled);
    }

    /// A mixed selection containing a Rectangle disables Join into
    /// Polygon, but Delete stays enabled (a non-empty selection is
    /// still deletable even when it can't be joined).
    #[test]
    fn mixed_selection_with_rectangle_disables_join_but_not_delete() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [4.0, 0.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [2.0, 2.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        let selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1],
        });
        let rows = build_symbol_context_menu_rows(&sym, 1, &selected);

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(find(&rows, "symbol.delete").enabled);
    }

    /// A single selected Polygon can't be joined (it's already the
    /// output kind, not an eligible source) but can be deleted.
    #[test]
    fn polygon_selected_disables_join_but_not_delete() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Polygon {
                vertices: vec![[0.0, 0.0], [4.0, 0.0], [2.0, 3.0]],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        let selected = Some(SymbolSelection::Graphic(0));
        let rows = build_symbol_context_menu_rows(&sym, 1, &selected);

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(find(&rows, "symbol.delete").enabled);
    }
}
