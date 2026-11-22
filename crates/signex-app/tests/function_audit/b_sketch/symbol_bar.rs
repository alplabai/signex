//! m141, the symbol editor's active bar and context menu. The order of the
//! menu is the 2026-11-19 decision for rows 140 to 142, which has no Flip.

use signex_app::library::messages::{
    SymbolEditorMsg, SymbolRotatePivotMsg, SymbolSelectionMsg, SymbolToolMsg,
};

use super::support::{SymRig, open_symbol};

fn select_pin(rig: &mut SymRig) {
    rig.send(SymbolEditorMsg::Select(SymbolSelectionMsg::Pin(0)));
}

fn rotate() -> SymbolEditorMsg {
    SymbolEditorMsg::RotateSelected {
        clockwise: false,
        pivot: SymbolRotatePivotMsg::WorldOrigin,
    }
}

#[test]
fn m141_rotate_turns_the_selected_pin_and_undo_restores_it() {
    let mut rig = open_symbol();
    select_pin(&mut rig);
    let before = rig.snapshot();

    rig.send(SymbolEditorMsg::ContextMenuAction(Box::new(rotate())));

    assert_ne!(rig.snapshot(), before);
    assert!(rig.dirty());
    assert!(
        rig.editor().context_menu.is_none(),
        "the row closes the menu"
    );
    rig.undo();
    assert_eq!(rig.snapshot(), before);
    assert!(!rig.dirty());
}

#[test]
fn m141_rotate_with_nothing_selected_is_no_edit() {
    let mut rig = open_symbol();
    rig.send(SymbolEditorMsg::Deselect);
    let before = rig.snapshot();

    rig.send(rotate());

    assert_eq!(rig.snapshot(), before);
    assert!(!rig.dirty());
}

#[test]
fn m141_delete_removes_the_selected_pin_and_undo_brings_it_back() {
    let mut rig = open_symbol();
    select_pin(&mut rig);

    rig.send(SymbolEditorMsg::ContextMenuAction(Box::new(
        SymbolEditorMsg::DeleteSelected,
    )));
    assert!(rig.symbol().pins.is_empty());
    assert!(rig.dirty());

    rig.undo();
    assert_eq!(rig.symbol().pins.len(), 1);
    assert!(!rig.dirty());
}

#[test]
fn m141_align_to_grid_puts_an_off_grid_pin_on_the_grid() {
    let mut rig = open_symbol();
    select_pin(&mut rig);
    rig.send(SymbolEditorMsg::MoveSelected { x: 2.6, y: 5.1 });
    let moved = rig.symbol().pins[0].position;

    rig.send(SymbolEditorMsg::AlignSelectedToGrid);

    let aligned = rig.symbol().pins[0].position;
    assert_ne!(aligned, moved, "the off-grid pin moved");
    rig.send(SymbolEditorMsg::AlignSelectedToGrid);
    assert_eq!(
        rig.symbol().pins[0].position,
        aligned,
        "a pin on the grid stays"
    );
}

#[test]
fn m141_the_shapes_dropdown_tools_each_place_their_shape() {
    let mut rig = open_symbol();
    let count = |rig: &SymRig| rig.symbol().graphics.len();
    let start = count(&rig);

    rig.send(SymbolEditorMsg::SetTool(SymbolToolMsg::PlaceLine));
    rig.send(SymbolEditorMsg::AddLine {
        from_x: 0.0,
        from_y: 0.0,
        to_x: 2.54,
        to_y: 0.0,
    });
    assert_eq!(count(&rig), start + 1, "Line");

    rig.send(SymbolEditorMsg::SetTool(SymbolToolMsg::PlaceCircle));
    rig.send(SymbolEditorMsg::AddCircle {
        cx: 5.08,
        cy: 5.08,
        radius: 1.27,
    });
    assert_eq!(count(&rig), start + 2, "Circle");

    rig.send(SymbolEditorMsg::SetTool(SymbolToolMsg::PlaceArc));
    rig.send(SymbolEditorMsg::AddArc {
        cx: 7.62,
        cy: 7.62,
        radius: 1.27,
        start_deg: 0.0,
        end_deg: 90.0,
    });
    assert_eq!(count(&rig), start + 3, "Arc");
}

#[test]
fn m141_a_polygon_of_three_points_is_placed() {
    let mut rig = open_symbol();
    let start = rig.symbol().graphics.len();

    rig.send(SymbolEditorMsg::SetTool(SymbolToolMsg::PlacePolygon));
    for (x, y) in [(0.0, 0.0), (2.54, 0.0), (2.54, 2.54)] {
        rig.send(SymbolEditorMsg::PolygonClick { x, y });
    }
    rig.send(SymbolEditorMsg::PolygonCommit);

    assert_eq!(rig.symbol().graphics.len(), start + 1);
}

#[test]
fn m141_a_polygon_of_two_points_is_refused() {
    let mut rig = open_symbol();
    let before = rig.snapshot();

    rig.send(SymbolEditorMsg::SetTool(SymbolToolMsg::PlacePolygon));
    for (x, y) in [(0.0, 0.0), (2.54, 0.0)] {
        rig.send(SymbolEditorMsg::PolygonClick { x, y });
    }
    rig.send(SymbolEditorMsg::PolygonCommit);

    assert_eq!(rig.snapshot(), before);
    assert!(!rig.dirty());
}

#[test]
fn m141_cancelling_a_polygon_places_nothing() {
    let mut rig = open_symbol();
    let before = rig.snapshot();

    rig.send(SymbolEditorMsg::SetTool(SymbolToolMsg::PlacePolygon));
    rig.send(SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 });
    rig.send(SymbolEditorMsg::PolygonCancel);

    assert_eq!(rig.snapshot(), before);
    assert!(!rig.dirty());
}
