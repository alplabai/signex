//! The Bezier and Pie Chart tools (#2838) through the editor's dispatcher:
//! one undo step per placement, cancel, grips, and save / reload.

use super::*;
use crate::library::editor::symbol::canvas::SymbolTool;
use crate::library::editor::symbol::state::{self, GraphicHandle};
use crate::library::messages::SymbolToolMsg;
use signex_library::{Symbol, SymbolFile, SymbolGraphicKind};
use std::path::PathBuf;

const CLICKS: [(f64, f64); 4] = [(0.0, 0.0), (2.54, 2.54), (5.08, 2.54), (7.62, 0.0)];

fn new_editor(tool: SymbolTool) -> SymEditor {
    let mut editor = SymEditor::new(
        PathBuf::from("t.snxsym"),
        SymbolFile::from_symbol(Symbol::empty("T")),
    );
    editor.tool = tool;
    editor
}

fn click_bezier(editor: &mut SymEditor, clicks: &[(f64, f64)]) {
    for &(x, y) in clicks {
        apply_symbol_primitive_edit(editor, SymbolEditorMsg::BezierClick { x, y });
    }
}

fn place_pie(editor: &mut SymEditor, start_deg: f64, end_deg: f64) {
    apply_symbol_primitive_edit(
        editor,
        SymbolEditorMsg::AddPie {
            cx: 1.27,
            cy: 1.27,
            radius: 2.54,
            start_deg,
            end_deg,
        },
    );
}

#[test]
fn the_fourth_bezier_click_places_one_curve_as_one_undo_step() {
    let mut editor = new_editor(SymbolTool::PlaceBezier);
    click_bezier(&mut editor, &CLICKS[..3]);
    assert!(editor.primitive().graphics.is_empty());
    assert_eq!(editor.history.undo_len(), 0, "clicks alone push no undo");

    click_bezier(&mut editor, &CLICKS[3..]);
    assert_eq!(editor.history.undo_len(), 1);
    assert!(editor.polygon_vertices.is_empty());
    assert_eq!(
        editor.primitive().graphics[0].kind,
        SymbolGraphicKind::Bezier {
            points: CLICKS.map(|(x, y)| [x, y]),
        }
    );

    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Undo);
    assert!(editor.primitive().graphics.is_empty());
}

#[test]
fn four_clicks_on_one_point_place_nothing() {
    let mut editor = new_editor(SymbolTool::PlaceBezier);
    click_bezier(&mut editor, &[(1.27, 1.27); 4]);
    assert!(editor.primitive().graphics.is_empty());
    assert_eq!(editor.history.undo_len(), 0);
    assert!(editor.polygon_vertices.is_empty());
}

#[test]
fn cancelling_a_bezier_drops_its_clicks() {
    let mut editor = new_editor(SymbolTool::PlaceBezier);
    click_bezier(&mut editor, &CLICKS[..3]);
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCancel);
    assert!(editor.polygon_vertices.is_empty());
    assert!(editor.primitive().graphics.is_empty());
    assert_eq!(editor.history.undo_len(), 0);
}

#[test]
fn leaving_the_bezier_tool_places_no_shape_from_a_partial_curve() {
    let mut editor = new_editor(SymbolTool::PlaceBezier);
    click_bezier(&mut editor, &CLICKS[..3]);
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::SetTool(SymbolToolMsg::Select));
    assert_eq!(editor.tool, SymbolTool::Select);
    assert!(editor.polygon_vertices.is_empty());
    assert!(editor.primitive().graphics.is_empty());
    assert_eq!(editor.history.undo_len(), 0);
}

/// A part switch flushes the shared stash as a polygon for the Polygon
/// tool; three Bezier clicks must not become a triangle.
#[test]
fn a_part_switch_does_not_turn_bezier_clicks_into_a_polygon() {
    let mut editor = new_editor(SymbolTool::PlaceBezier);
    click_bezier(&mut editor, &CLICKS[..3]);
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::NewPart);
    assert!(editor.polygon_vertices.is_empty());
    assert!(editor.primitive().graphics.is_empty());
}

#[test]
fn each_bezier_point_has_a_grip_that_moves_only_it() {
    let mut editor = new_editor(SymbolTool::PlaceBezier);
    click_bezier(&mut editor, &CLICKS);
    let handles = state::graphic_handles(editor.primitive(), 0);
    assert_eq!(handles.len(), 4);
    assert_eq!(handles[2], (GraphicHandle::BezierPoint(2), [5.08, 2.54]));

    state::move_graphic_handle(
        editor.primitive_mut(),
        0,
        GraphicHandle::BezierPoint(1),
        1.27,
        3.81,
    );
    assert_eq!(
        editor.primitive().graphics[0].kind,
        SymbolGraphicKind::Bezier {
            points: [[0.0, 0.0], [1.27, 3.81], [5.08, 2.54], [7.62, 0.0]],
        }
    );
}

#[test]
fn a_pie_placement_is_one_undo_step_and_keeps_the_short_sweep() {
    let mut editor = new_editor(SymbolTool::PlacePie);
    place_pie(&mut editor, 30.0, -60.0);
    assert_eq!(editor.history.undo_len(), 1);
    assert_eq!(
        editor.primitive().graphics[0].kind,
        SymbolGraphicKind::Pie {
            center: [1.27, 1.27],
            radius: 2.54,
            start_deg: 300.0,
            end_deg: 30.0,
        }
    );
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Undo);
    assert!(editor.primitive().graphics.is_empty());
}

#[test]
fn a_pie_grip_changes_its_angle_and_keeps_its_radius() {
    let mut editor = new_editor(SymbolTool::PlacePie);
    place_pie(&mut editor, 0.0, 90.0);
    state::move_graphic_handle(editor.primitive_mut(), 0, GraphicHandle::ArcEnd, 1.27, -5.0);
    let SymbolGraphicKind::Pie {
        radius, end_deg, ..
    } = editor.primitive().graphics[0].kind
    else {
        panic!("expected a Pie");
    };
    assert_eq!(radius, 2.54);
    assert!((end_deg - 270.0).abs() < 1e-9, "{end_deg}");
}

#[test]
fn an_unfilled_pie_is_picked_on_its_edge_and_a_filled_one_inside() {
    let mut editor = new_editor(SymbolTool::PlacePie);
    place_pie(&mut editor, 0.0, 90.0);
    let view = editor.view();
    let filter = editor.selection_filter;
    // Inside the wedge, clear of the edge band.
    let inside = (1.27 + 1.0, 1.27 + 1.0);
    assert_eq!(
        state::hit_test_graphic(editor.primitive(), inside.0, inside.1, view, &filter),
        None
    );
    assert_eq!(
        state::hit_test_graphic(editor.primitive(), 2.54, 1.27, view, &filter),
        Some(0),
        "on the straight edge"
    );
    editor.primitive_mut().graphics[0].fill = Some([255, 0, 0, 255]);
    assert_eq!(
        state::hit_test_graphic(editor.primitive(), inside.0, inside.1, view, &filter),
        Some(0)
    );
}

#[test]
fn placed_curves_survive_save_and_reload() {
    let mut editor = new_editor(SymbolTool::PlaceBezier);
    click_bezier(&mut editor, &CLICKS);
    place_pie(&mut editor, 300.0, 30.0);
    let text = editor.file.to_toml_string().unwrap();
    assert!(text.contains("format = \"snxsym/v4\""), "{text}");
    let back = SymbolFile::from_toml_str(&text).unwrap();
    assert_eq!(back.symbols[0].graphics, editor.primitive().graphics);
}
