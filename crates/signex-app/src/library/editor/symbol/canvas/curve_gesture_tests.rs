//! The Bezier and Pie Chart gestures through the real canvas input chain,
//! and what the editor draws for them (#2838).

use super::*;

const SCALE: f32 = 10.0;

fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

fn rig(tool: SymbolTool) -> Rig {
    let mut rig = Rig::new(Symbol::empty("C"));
    rig.editor.gui.tool = tool;
    rig
}

fn snapshot(rig: &Rig) -> RendererSnapshot {
    let editor = &rig.editor;
    SymbolCanvas::new(
        editor.primitive(),
        None,
        SymbolTool::Select,
        &editor.gui.polygon_vertices,
        editor.view(),
        false,
        &editor.gui.camera,
        1.27,
        true,
        crate::render_config::GridStyle::default(),
        true,
        Color::WHITE,
        Color::from_rgb(0.2, 0.4, 0.9),
        Color::BLACK,
        Color::BLACK,
        Color::BLACK,
    )
    .build_symbol_renderer_snapshot(&None, SCALE)
}

#[test]
fn four_presses_place_a_bezier_through_the_canvas() {
    let mut rig = rig(SymbolTool::PlaceBezier);
    for at in [(0.0, 0.0), (2.54, 2.54), (5.08, 2.54)] {
        rig.press(at);
        rig.release(at);
    }
    assert_eq!(rig.editor.gui.polygon_vertices.len(), 3);
    rig.press((7.62, 0.0));
    let graphics = &rig.editor.primitive().graphics;
    assert_eq!(graphics.len(), 1);
    let SymbolGraphicKind::Bezier { points } = graphics[0].kind else {
        panic!("expected a Bezier, got {:?}", graphics[0].kind);
    };
    assert!(close(points[1], [2.54, 2.54]), "{points:?}");
    assert!(close(points[3], [7.62, 0.0]), "{points:?}");
    assert_eq!(rig.editor.draft.history.undo_len(), 1);
}

#[test]
fn esc_and_right_click_cancel_a_bezier_and_keep_the_tool() {
    let mut rig = rig(SymbolTool::PlaceBezier);
    rig.press((0.0, 0.0));
    assert!(rig.escape(), "the canvas claims Esc mid-gesture");
    assert!(rig.editor.gui.polygon_vertices.is_empty());
    assert_eq!(rig.editor.gui.tool, SymbolTool::PlaceBezier);

    rig.press((0.0, 0.0));
    rig.right_click((2.54, 2.54));
    assert!(rig.editor.gui.polygon_vertices.is_empty());
    assert_eq!(rig.editor.gui.tool, SymbolTool::PlaceBezier);
    assert!(rig.editor.primitive().graphics.is_empty());
}

#[test]
fn three_presses_place_a_pie_through_the_canvas() {
    let mut rig = rig(SymbolTool::PlacePie);
    rig.press((0.0, 0.0));
    rig.press((2.54, 0.0));
    rig.moved((0.0, 2.54));
    rig.press((0.0, 2.54));
    let graphics = &rig.editor.primitive().graphics;
    assert_eq!(graphics.len(), 1);
    let SymbolGraphicKind::Pie {
        radius,
        start_deg,
        end_deg,
        ..
    } = graphics[0].kind
    else {
        panic!("expected a Pie, got {:?}", graphics[0].kind);
    };
    assert!((radius - 2.54).abs() < 1e-3, "{radius}");
    assert!(start_deg.abs() < 1e-3, "{start_deg}");
    assert!((end_deg - 90.0).abs() < 1e-3, "{end_deg}");
    assert_eq!(rig.editor.draft.history.undo_len(), 1);
}

#[test]
fn esc_cancels_a_pie_with_nothing_placed() {
    let mut rig = rig(SymbolTool::PlacePie);
    rig.press((0.0, 0.0));
    rig.press((2.54, 0.0));
    assert!(rig.escape());
    rig.press((5.08, 5.08));
    assert!(
        rig.editor.primitive().graphics.is_empty(),
        "the next press starts a new gesture"
    );
}

#[test]
fn five_presses_place_an_elliptical_arc_at_the_clicked_angles() {
    let mut rig = rig(SymbolTool::PlaceEllipticalArc);
    for at in [(0.0, 0.0), (5.08, 0.0), (0.0, 2.54), (2.54, 2.54)] {
        rig.press(at);
        assert!(rig.editor.primitive().graphics.is_empty());
    }
    rig.moved((-2.54, 2.54));
    rig.press((-2.54, 2.54));
    let graphics = &rig.editor.primitive().graphics;
    assert_eq!(graphics.len(), 1);
    let SymbolGraphicKind::EllipticalArc {
        radius_x,
        radius_y,
        start_deg,
        end_deg,
        ..
    } = graphics[0].kind
    else {
        panic!("expected an EllipticalArc, got {:?}", graphics[0].kind);
    };
    assert!((radius_x - 5.08).abs() < 1e-3, "{radius_x}");
    assert!((radius_y - 2.54).abs() < 1e-3, "{radius_y}");
    assert!((start_deg - 45.0).abs() < 1e-3, "geometric: {start_deg}");
    assert!((end_deg - 135.0).abs() < 1e-3, "geometric: {end_deg}");
    assert_eq!(rig.editor.draft.history.undo_len(), 1);
}

#[test]
fn esc_and_right_click_cancel_an_elliptical_arc() {
    let mut rig = rig(SymbolTool::PlaceEllipticalArc);
    rig.press((0.0, 0.0));
    rig.press((5.08, 0.0));
    assert!(rig.escape());
    assert!(!rig.cstate.ellipse.is_active());
    rig.press((0.0, 0.0));
    rig.right_click((2.54, 2.54));
    assert!(!rig.cstate.ellipse.is_active());
    assert_eq!(rig.editor.gui.tool, SymbolTool::PlaceEllipticalArc);
    assert!(rig.editor.primitive().graphics.is_empty());
}

#[test]
fn two_presses_place_a_round_rectangle() {
    let mut rig = rig(SymbolTool::PlaceRoundRectangle);
    rig.press((7.62, 5.08));
    rig.press((0.0, 0.0));
    let graphics = &rig.editor.primitive().graphics;
    assert_eq!(graphics.len(), 1);
    let SymbolGraphicKind::RoundRectangle {
        from,
        to,
        corner_radius,
    } = graphics[0].kind
    else {
        panic!("expected a RoundRectangle, got {:?}", graphics[0].kind);
    };
    assert!(close(from, [0.0, 0.0]), "{from:?}");
    assert!(close(to, [7.62, 5.08]), "{to:?}");
    assert_eq!(corner_radius, 1.27);
    assert_eq!(rig.editor.draft.history.undo_len(), 1);
}

#[test]
fn esc_cancels_a_round_rectangle() {
    let mut rig = rig(SymbolTool::PlaceRoundRectangle);
    rig.press((0.0, 0.0));
    assert!(rig.escape());
    rig.press((5.08, 5.08));
    assert!(rig.editor.primitive().graphics.is_empty());
}

#[test]
fn the_editor_draws_an_elliptical_arc_open_and_a_round_rectangle_as_a_ring() {
    let mut rig = rig(SymbolTool::Select);
    let arc = SymbolGraphicKind::EllipticalArc {
        center: [0.0, 0.0],
        radius_x: 5.08,
        radius_y: 2.54,
        start_deg: 0.0,
        end_deg: 90.0,
    };
    let rect = SymbolGraphicKind::round_rectangle([0.0, 0.0], [7.62, 5.08]);
    let (arc_points, rect_ring) = (arc.flattened_outline(), rect.flattened_outline());
    for kind in [arc, rect] {
        rig.editor
            .primitive_mut()
            .graphics
            .push(signex_library::SymbolGraphic {
                kind,
                stroke_width: 0.15,
                fill: None,
                part_number: 1,
                body_style: 1,
            });
    }
    let drawn = snapshot(&rig);
    assert_eq!(drawn.wires.len(), arc_points.len() - 1);
    assert_eq!(drawn.polygons.len(), 1);
    assert_eq!(drawn.polygons[0].vertices.len(), rect_ring.len());
}

fn pick(rig: &mut Rig, tool: crate::library::messages::SymbolToolMsg) {
    apply_symbol_primitive_edit(
        &mut rig.editor,
        crate::library::messages::SymbolEditorMsg::SetTool(tool),
    );
}

fn press_all(rig: &mut Rig, clicks: &[(f64, f64)]) {
    for &at in clicks {
        rig.press(at);
    }
}

fn first_graphic(rig: &Rig) -> &SymbolGraphicKind {
    &rig.editor.primitive().graphics[0].kind
}

const PIE_CLICKS: [(f64, f64); 3] = [(5.08, 5.08), (7.62, 5.08), (5.08, 7.62)];

#[test]
fn a_half_placed_arc_does_not_carry_into_the_pie_tool() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlaceArc);
    rig.press((0.0, 0.0));
    pick(&mut rig, T::PlacePie);
    press_all(&mut rig, &PIE_CLICKS[..2]);
    assert!(rig.editor.primitive().graphics.is_empty());
    rig.press(PIE_CLICKS[2]);
    let SymbolGraphicKind::Pie { center, .. } = *first_graphic(&rig) else {
        panic!("expected a Pie");
    };
    assert!(close(center, [5.08, 5.08]), "{center:?}");
}

#[test]
fn a_half_placed_pie_does_not_carry_into_the_arc_tool() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlacePie);
    press_all(&mut rig, &[(0.0, 0.0), (2.54, 0.0)]);
    pick(&mut rig, T::PlaceArc);
    press_all(&mut rig, &PIE_CLICKS[..2]);
    assert!(rig.editor.primitive().graphics.is_empty());
    rig.press(PIE_CLICKS[2]);
    let SymbolGraphicKind::Arc { center, .. } = *first_graphic(&rig) else {
        panic!("expected an Arc");
    };
    assert!(close(center, [5.08, 5.08]), "{center:?}");
}

#[test]
fn a_half_placed_rectangle_does_not_carry_into_the_round_rectangle_tool() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlaceRectangle);
    rig.press((0.0, 0.0));
    pick(&mut rig, T::PlaceRoundRectangle);
    rig.press((5.08, 5.08));
    assert!(rig.editor.primitive().graphics.is_empty());
    rig.press((10.16, 10.16));
    let SymbolGraphicKind::RoundRectangle { from, .. } = *first_graphic(&rig) else {
        panic!("expected a RoundRectangle");
    };
    assert!(close(from, [5.08, 5.08]), "{from:?}");
}

#[test]
fn a_half_placed_round_rectangle_does_not_carry_into_the_rectangle_tool() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlaceRoundRectangle);
    rig.press((0.0, 0.0));
    pick(&mut rig, T::PlaceRectangle);
    rig.press((5.08, 5.08));
    assert!(rig.editor.primitive().graphics.is_empty());
    rig.press((10.16, 10.16));
    let SymbolGraphicKind::Rectangle { from, .. } = *first_graphic(&rig) else {
        panic!("expected a Rectangle");
    };
    assert!(close(from, [5.08, 5.08]), "{from:?}");
}

#[test]
fn bezier_clicks_do_not_carry_into_the_polygon_tool() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlaceBezier);
    press_all(&mut rig, &[(0.0, 0.0), (2.54, 2.54), (5.08, 2.54)]);
    pick(&mut rig, T::PlacePolygon);
    assert!(rig.editor.gui.polygon_vertices.is_empty());
    assert!(rig.editor.primitive().graphics.is_empty());
}

#[test]
fn short_polygon_clicks_do_not_carry_into_the_bezier_tool() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlacePolygon);
    press_all(&mut rig, &[(0.0, 0.0), (2.54, 2.54)]);
    pick(&mut rig, T::PlaceBezier);
    assert!(rig.editor.gui.polygon_vertices.is_empty());
    assert!(rig.editor.primitive().graphics.is_empty());
}

/// Out and back with no canvas event between still ends the gesture.
#[test]
fn leaving_and_returning_to_the_elliptical_arc_tool_starts_at_the_center() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlaceEllipticalArc);
    press_all(&mut rig, &[(0.0, 0.0), (5.08, 0.0)]);
    pick(&mut rig, T::Select);
    pick(&mut rig, T::PlaceEllipticalArc);
    let clicks = [
        (2.54, 2.54),
        (7.62, 2.54),
        (2.54, 5.08),
        (5.08, 5.08),
        (0.0, 5.08),
    ];
    press_all(&mut rig, &clicks[..4]);
    assert!(rig.editor.primitive().graphics.is_empty());
    rig.press(clicks[4]);
    let SymbolGraphicKind::EllipticalArc { center, .. } = *first_graphic(&rig) else {
        panic!("expected an EllipticalArc");
    };
    assert!(close(center, [2.54, 2.54]), "{center:?}");
}

#[test]
fn picking_the_armed_tool_again_keeps_its_gesture() {
    use crate::library::messages::SymbolToolMsg as T;
    let mut rig = rig(SymbolTool::PlaceRectangle);
    rig.press((0.0, 0.0));
    pick(&mut rig, T::PlaceRectangle);
    rig.press((5.08, 5.08));
    assert_eq!(rig.editor.primitive().graphics.len(), 1);
}

#[test]
fn the_editor_draws_a_bezier_as_segments_and_a_pie_as_a_closed_ring() {
    let mut rig = rig(SymbolTool::Select);
    let graphic = |kind| signex_library::SymbolGraphic {
        kind,
        stroke_width: 0.15,
        fill: None,
        part_number: 1,
        body_style: 1,
    };
    let bezier = SymbolGraphicKind::Bezier {
        points: [[0.0, 0.0], [2.54, 2.54], [5.08, 2.54], [7.62, 0.0]],
    };
    let pie = SymbolGraphicKind::Pie {
        center: [0.0, 0.0],
        radius: 2.54,
        start_deg: 0.0,
        end_deg: 90.0,
    };
    let (bezier_points, pie_ring) = (bezier.flattened_outline(), pie.flattened_outline());
    let graphics = &mut rig.editor.primitive_mut().graphics;
    graphics.push(graphic(bezier));
    graphics.push(graphic(pie));

    let drawn = snapshot(&rig);
    assert_eq!(drawn.wires.len(), bezier_points.len() - 1);
    assert_eq!(drawn.polygons.len(), 1);
    assert_eq!(drawn.polygons[0].vertices.len(), pie_ring.len());
}
