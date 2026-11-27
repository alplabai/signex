//! The curve drawings (#1267) through the drawing commands: place and undo,
//! move, a quarter turn and a mirror, the lock.

use crate::*;
use signex_types::schematic::{CurveShape, Point, SchCurve, SchDrawing, SelectedKind};
use uuid::Uuid;

fn arc() -> SchDrawing {
    SchDrawing::Curve(SchCurve::new(
        Uuid::from_u128(1),
        CurveShape::EllipticalArc {
            center: Point::new(20.0, 10.0),
            radius_x: 6.0,
            radius_y: 3.0,
            start_deg: 0.0,
            end_deg: 90.0,
        },
        0.0,
    ))
}

fn kinds() -> [SchDrawing; 3] {
    let curve = |n, shape| SchDrawing::Curve(SchCurve::new(Uuid::from_u128(n), shape, 0.0));
    [
        arc(),
        curve(
            2,
            CurveShape::Ellipse {
                center: Point::new(40.0, 40.0),
                radius_x: 5.0,
                radius_y: 2.5,
            },
        ),
        curve(
            3,
            CurveShape::round_rect(Point::new(10.0, 50.0), Point::new(30.0, 60.0)),
        ),
    ]
}

fn placed(drawing: SchDrawing) -> Engine {
    let mut engine = Engine::new(SchematicSheet::default()).expect("engine");
    engine
        .execute(Command::PlaceSchDrawing { drawing })
        .expect("placed");
    engine
}

fn the_drawing(engine: &Engine) -> &SchDrawing {
    &engine.document().drawings[0]
}

fn item(d: &SchDrawing) -> SelectedItem {
    SelectedItem::new(d.uuid(), SelectedKind::Drawing)
}

fn ends(d: &SchDrawing) -> (Point, Point) {
    let SchDrawing::Curve(c) = d else {
        panic!("a curve");
    };
    let outline = c.flatten();
    (outline[0], outline[outline.len() - 1])
}

fn transformed(drawing: &SchDrawing, transform: SelectionTransform) -> SchDrawing {
    let mut engine = placed(drawing.clone());
    engine
        .execute(Command::TransformSelection {
            items: vec![item(drawing)],
            centre: Point::new(0.0, 0.0),
            transform,
            keep_wires: false,
        })
        .expect("transformed");
    the_drawing(&engine).clone()
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn each_curve_places_and_undoes() {
    for drawing in kinds() {
        let mut engine = placed(drawing.clone());
        assert_eq!(the_drawing(&engine), &drawing);
        engine.undo().expect("undo");
        assert!(engine.document().drawings.is_empty());
    }
}

#[test]
fn each_curve_moves_by_the_drag_and_keeps_its_size() {
    for drawing in kinds() {
        let mut engine = placed(drawing.clone());
        engine
            .execute(Command::MoveSelection {
                items: vec![item(&drawing)],
                dx: 2.54,
                dy: -1.27,
                keep_wires: false,
                wiring: signex_types::corner::DragWiringMode::Auto,
            })
            .expect("moved");
        let mut expected = drawing.clone();
        expected.map_annotation_points(|p| Point::new(p.x + 2.54, p.y - 1.27));
        assert_eq!(the_drawing(&engine), &expected);
    }
}

#[test]
fn a_quarter_turn_swaps_the_radii_and_keeps_the_arc_ends_on_their_circles() {
    let drawing = arc();
    let turned = transformed(&drawing, SelectionTransform::RotateClockwise);
    let SchDrawing::Curve(SchCurve {
        shape: CurveShape::EllipticalArc {
            radius_x, radius_y, ..
        },
        ..
    }) = &turned
    else {
        panic!("still an elliptical arc: {turned:?}");
    };
    assert!((radius_x - 3.0).abs() < 1e-9 && (radius_y - 6.0).abs() < 1e-9);
    let ((a0, a1), (b0, b1)) = (ends(&drawing), ends(&turned));
    let norm = |p: Point| p.x.hypot(p.y);
    assert!((norm(a0) - norm(b0)).abs() < 1e-9, "{a0:?} -> {b0:?}");
    assert!((norm(a1) - norm(b1)).abs() < 1e-9, "{a1:?} -> {b1:?}");
}

#[test]
fn a_mirror_covers_the_mirrored_arc() {
    let drawing = arc();
    let mirrored = transformed(&drawing, SelectionTransform::Mirror(MirrorAxis::Horizontal));
    let ((a0, a1), (b0, b1)) = (ends(&drawing), ends(&mirrored));
    let flip = |p: Point| Point::new(-p.x, p.y);
    assert!(close(b0, flip(a1)), "{b0:?} vs {:?}", flip(a1));
    assert!(close(b1, flip(a0)), "{b1:?} vs {:?}", flip(a0));
}

#[test]
fn a_locked_curve_refuses_a_move() {
    let drawing = arc();
    let mut engine = placed(drawing.clone());
    engine
        .execute(Command::SetItemLocked {
            item: item(&drawing),
            locked: true,
        })
        .expect("locked");
    let refused = engine.execute(Command::MoveSelection {
        items: vec![item(&drawing)],
        dx: 5.0,
        dy: 0.0,
        keep_wires: false,
        wiring: signex_types::corner::DragWiringMode::Auto,
    });
    assert!(
        matches!(refused, Err(EngineError::ItemLocked { .. })),
        "{refused:?}"
    );
}
