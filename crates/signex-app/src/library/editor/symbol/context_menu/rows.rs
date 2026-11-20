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

use crate::keymap::command_menu_message;
use crate::library::editor::symbol::state::{
    SymbolSelection, selected_is_alignable, selected_is_deletable, selection_is_join_eligible,
};
use crate::library::messages::{
    SymbolEditorMsg, SymbolRotatePivotMsg, SymbolSelectionMsg, SymbolToolMsg,
};
use signex_i18n::{MessageId, key_ids, menu_ids};
use signex_library::Symbol;

/// The glyph in a row's icon column; the view maps it to an SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIcon {
    Place,
    Rotate,
    Align,
    SelectAll,
    Delete,
}

/// The key a row shows at its right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKeys {
    /// The canvas's own key for the same action (Space rotates).
    Canvas(MessageId),
    /// The key the active keymap binds to this command id.
    Command(&'static str),
}

/// One row of the symbol context menu. `id` is a stable, kebab-case,
/// `symbol.`-namespaced command id (the command-registry epic will
/// index these — see `menu_bar` / keymap catalog for the sibling
/// convention). `msg` is the message this row fires on click; `None`
/// for a submenu header, which instead toggles `submenu`'s visibility.
/// `submenu`, when present, holds the rows shown in place directly
/// below this row while it's the open submenu (accordion — mirrors
/// the footprint context menu's Place ▸ / Align ▸ expand-in-place
/// behaviour), not a hover flyout. `keys` names the key that does the
/// same; `divider_before` draws a divider above the row.
pub struct SymbolMenuRow {
    pub id: &'static str,
    pub label: MessageId,
    pub enabled: bool,
    pub msg: Option<SymbolEditorMsg>,
    pub submenu: Option<Vec<SymbolMenuRow>>,
    pub icon: Option<RowIcon>,
    pub keys: Option<RowKeys>,
    pub divider_before: bool,
}

impl SymbolMenuRow {
    fn item(id: &'static str, label: MessageId, enabled: bool, msg: SymbolEditorMsg) -> Self {
        Self {
            id,
            label,
            enabled,
            msg: Some(msg),
            submenu: None,
            icon: None,
            keys: None,
            divider_before: false,
        }
    }

    fn submenu(id: &'static str, label: MessageId, children: Vec<SymbolMenuRow>) -> Self {
        Self {
            submenu: Some(children),
            msg: None,
            ..Self::item(id, label, true, SymbolEditorMsg::Deselect)
        }
    }

    fn icon(mut self, icon: RowIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    fn keys(mut self, keys: RowKeys) -> Self {
        self.keys = Some(keys);
        self
    }

    fn after_divider(mut self) -> Self {
        self.divider_before = true;
        self
    }
}

/// Build the full row tree for the current selection. Every row is
/// present unconditionally — selection-awareness lives entirely in
/// `enabled` (not row presence), so callers/tests get a stable id
/// list regardless of selection state; the renderer greys out /
/// disables clicks on a `enabled: false` row.
///
/// Order (mockup row 141): Place ▸, the transforms (Rotate Selection, Align ▸), a
/// divider, Join into Polygon, Select All, Deselect All, Fit to Window,
/// and Delete last.
pub fn build_symbol_context_menu_rows(
    sym: &Symbol,
    view: crate::library::editor::symbol::state::PartView,
    selected: &Option<SymbolSelection>,
) -> Vec<SymbolMenuRow> {
    let transformable = selected_is_alignable(selected);
    vec![
        place_submenu().icon(RowIcon::Place),
        // Space on the canvas: counter-clockwise about the world origin.
        SymbolMenuRow::item(
            "symbol.rotate-selection",
            menu_ids::CTX_SYMBOL_ROTATE_SELECTION,
            transformable,
            SymbolEditorMsg::RotateSelected {
                clockwise: false,
                pivot: SymbolRotatePivotMsg::WorldOrigin,
            },
        )
        .icon(RowIcon::Rotate)
        .keys(RowKeys::Canvas(key_ids::KEY_SPACE)),
        SymbolMenuRow::submenu(
            "symbol.align",
            menu_ids::CTX_ALIGN,
            vec![SymbolMenuRow::item(
                "symbol.align-to-grid",
                menu_ids::CTX_SYMBOL_ALIGN_TO_GRID,
                transformable,
                SymbolEditorMsg::AlignSelectedToGrid,
            )],
        )
        .icon(RowIcon::Align),
        SymbolMenuRow::item(
            "symbol.join-into-polygon",
            menu_ids::CTX_SYMBOL_JOIN_POLYGON,
            selection_is_join_eligible(sym, view, selected),
            SymbolEditorMsg::JoinSelectionIntoPolygon,
        )
        .after_divider(),
        SymbolMenuRow::item(
            "symbol.select-all",
            command_menu_message("select_all"),
            true,
            SymbolEditorMsg::Select(SymbolSelectionMsg::All),
        )
        .icon(RowIcon::SelectAll),
        SymbolMenuRow::item(
            "symbol.deselect-all",
            menu_ids::CTX_DESELECT_ALL,
            selected.is_some(),
            SymbolEditorMsg::Deselect,
        ),
        SymbolMenuRow::item(
            "symbol.fit-to-window",
            menu_ids::CTX_FIT_TO_WINDOW,
            true,
            SymbolEditorMsg::Fit,
        ),
        SymbolMenuRow::item(
            "symbol.delete",
            command_menu_message("delete_selection"),
            selected_is_deletable(selected),
            SymbolEditorMsg::DeleteSelected,
        )
        .icon(RowIcon::Delete)
        .keys(RowKeys::Command("delete_selection")),
    ]
}

/// `(id, label, tool)` for every `Place ▸` row — one per canvas
/// placement tool, mirroring the SchLib Place menu / toolbar tool set
/// (`SymbolTool`).
pub const PLACE_TOOLS: &[(&str, MessageId, SymbolToolMsg)] = &[
    (
        "symbol.place-pin",
        menu_ids::CTX_SYMBOL_PIN,
        SymbolToolMsg::AddPin,
    ),
    (
        "symbol.place-line",
        menu_ids::CTX_SYMBOL_LINE,
        SymbolToolMsg::PlaceLine,
    ),
    (
        "symbol.place-rectangle",
        menu_ids::CTX_SHAPE_RECTANGLE,
        SymbolToolMsg::PlaceRectangle,
    ),
    (
        "symbol.place-circle",
        menu_ids::CTX_SHAPE_CIRCLE,
        SymbolToolMsg::PlaceCircle,
    ),
    (
        "symbol.place-arc",
        menu_ids::CTX_SHAPE_ARC,
        SymbolToolMsg::PlaceArc,
    ),
    (
        "symbol.place-text",
        menu_ids::CTX_SYMBOL_TEXT,
        SymbolToolMsg::PlaceText,
    ),
    (
        "symbol.place-polygon",
        menu_ids::CTX_SYMBOL_POLYGON,
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
    SymbolMenuRow::submenu("symbol.place", menu_ids::MENU_PLACE, children)
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

    /// The top-level id list is stable regardless of selection state, in
    /// the mockup row 141 order: Place ▸ first, the transforms, a divider, the kept
    /// rows, Delete last.
    #[test]
    fn top_level_ids_are_stable() {
        let sym = Symbol::empty("T");
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &None,
        );
        assert_eq!(
            row_ids(&rows),
            vec![
                "symbol.place",
                "symbol.rotate-selection",
                "symbol.align",
                "symbol.join-into-polygon",
                "symbol.select-all",
                "symbol.deselect-all",
                "symbol.fit-to-window",
                "symbol.delete",
            ]
        );
        let dividers: Vec<&str> = rows
            .iter()
            .filter(|r| r.divider_before)
            .map(|r| r.id)
            .collect();
        assert_eq!(dividers, ["symbol.join-into-polygon"]);
        let align = find(&rows, "symbol.align");
        let align_rows = align.submenu.as_ref().expect("Align is a submenu");
        assert_eq!(row_ids(align_rows), vec!["symbol.align-to-grid"]);
        assert!(matches!(
            align_rows[0].msg,
            Some(SymbolEditorMsg::AlignSelectedToGrid)
        ));
        assert!(matches!(
            find(&rows, "symbol.rotate-selection").msg,
            Some(SymbolEditorMsg::RotateSelected {
                clockwise: false,
                ..
            })
        ));
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

    /// Delete and Select All read their text through a command id
    /// string; a mistyped id would quietly show "Unnamed command".
    #[test]
    fn command_rows_keep_their_english() {
        let sym = Symbol::empty("T");
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &None,
        );
        let en = signex_i18n::I18n::en_us_ref();
        let text = |id| en.message(find(&rows, id).label, &[]).to_string();
        assert_eq!(text("symbol.delete"), "Delete");
        assert_eq!(text("symbol.select-all"), "Select All");
    }

    /// Empty selection: Join into Polygon, Delete, and Deselect All
    /// are disabled; Select All and Fit to Window stay enabled.
    #[test]
    fn empty_selection_disables_selection_dependent_rows() {
        let sym = Symbol::empty("T");
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &None,
        );

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(!find(&rows, "symbol.delete").enabled);
        assert!(!find(&rows, "symbol.deselect-all").enabled);
        assert!(find(&rows, "symbol.select-all").enabled);
        assert!(find(&rows, "symbol.fit-to-window").enabled);
        assert!(find(&rows, "symbol.place").enabled);
        // Rotate would push an empty undo step with nothing selected.
        assert!(!find(&rows, "symbol.rotate-selection").enabled);
        let align = find(&rows, "symbol.align");
        assert!(align.enabled);
        assert!(!align.submenu.as_ref().expect("submenu")[0].enabled);
    }

    /// With a graphic selected, Rotate and Align To Grid are live.
    #[test]
    fn a_selection_enables_rotate_and_align_to_grid() {
        let rows = build_symbol_context_menu_rows(
            &Symbol::empty("T"),
            crate::library::editor::symbol::state::PartView::normal(1),
            &Some(SymbolSelection::Graphic(0)),
        );
        assert!(find(&rows, "symbol.rotate-selection").enabled);
        assert!(
            find(&rows, "symbol.align")
                .submenu
                .as_ref()
                .expect("submenu")[0]
                .enabled
        );
    }

    /// `SymbolSelection::All` is a no-op for `delete_selected`, so the
    /// Delete row must be disabled — otherwise clicking it pushes a
    /// wasted undo snapshot that can evict real history. Regression
    /// guard for the delete-on-All undo-pollution fix.
    #[test]
    fn all_selection_disables_delete() {
        let sym = Symbol::empty("T");
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &Some(SymbolSelection::All),
        );

        assert!(!find(&rows, "symbol.delete").enabled);
        // Select All / Fit stay enabled; Deselect All is enabled too
        // (a non-empty selection can be cleared).
        assert!(find(&rows, "symbol.select-all").enabled);
        assert!(find(&rows, "symbol.deselect-all").enabled);
    }

    /// A selection of only Line graphics enables Join into Polygon.
    #[test]
    fn line_only_selection_enables_join() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            body_style: 1,
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [4.0, 0.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        sym.graphics.push(SymbolGraphic {
            body_style: 1,
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
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &selected,
        );

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
            body_style: 1,
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [4.0, 0.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        let selected = Some(SymbolSelection::Graphic(0));
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &selected,
        );

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(find(&rows, "symbol.delete").enabled);
    }

    /// A single selected Arc, by contrast, enables Join into Polygon —
    /// a sufficiently large sweep can self-close.
    #[test]
    fn single_arc_selection_enables_join() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            body_style: 1,
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
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &selected,
        );

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
                body_style: 1,
                kind: SymbolGraphicKind::Line { from, to },
                stroke_width: 0.15,
                fill: None,
                part_number: 0,
            });
        }
        let selected = Some(SymbolSelection::All);
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &selected,
        );

        assert!(find(&rows, "symbol.join-into-polygon").enabled);
    }

    /// A mixed selection containing a Rectangle disables Join into
    /// Polygon, but Delete stays enabled (a non-empty selection is
    /// still deletable even when it can't be joined).
    #[test]
    fn mixed_selection_with_rectangle_disables_join_but_not_delete() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            body_style: 1,
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [4.0, 0.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        sym.graphics.push(SymbolGraphic {
            body_style: 1,
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
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &selected,
        );

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(find(&rows, "symbol.delete").enabled);
    }

    /// A single selected Polygon can't be joined (it's already the
    /// output kind, not an eligible source) but can be deleted.
    #[test]
    fn polygon_selected_disables_join_but_not_delete() {
        let mut sym = Symbol::empty("T");
        sym.graphics.push(SymbolGraphic {
            body_style: 1,
            kind: SymbolGraphicKind::Polygon {
                vertices: vec![[0.0, 0.0], [4.0, 0.0], [2.0, 3.0]],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        let selected = Some(SymbolSelection::Graphic(0));
        let rows = build_symbol_context_menu_rows(
            &sym,
            crate::library::editor::symbol::state::PartView::normal(1),
            &selected,
        );

        assert!(!find(&rows, "symbol.join-into-polygon").enabled);
        assert!(find(&rows, "symbol.delete").enabled);
    }
}
