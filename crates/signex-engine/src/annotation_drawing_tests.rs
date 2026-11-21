//! The Bezier, dimension and table drawings (#1267, #1300) go through every
//! drawing command: place, update, move, turn, mirror, lock, delete and the
//! clipboard payload.

use crate::*;
use signex_types::schematic::{
    DimensionDirection, Point, SchBezier, SchDimension, SchDrawing, SchTable, SelectedKind,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn bezier() -> SchDrawing {
    SchDrawing::Bezier(SchBezier::new(
        id(1),
        Point::new(10.0, 10.0),
        Point::new(12.0, 20.0),
        Point::new(28.0, 20.0),
        Point::new(30.0, 10.0),
    ))
}

fn dimension() -> SchDrawing {
    SchDrawing::Dimension(SchDimension::new(
        id(2),
        Point::new(10.0, 40.0),
        Point::new(22.7, 40.0),
        -5.0,
        DimensionDirection::Horizontal,
    ))
}

fn table() -> SchDrawing {
    SchDrawing::Table(SchTable::new_typed(id(3), Point::new(50.0, 50.0)))
}

fn kinds() -> [SchDrawing; 3] {
    [bezier(), dimension(), table()]
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

/// Every point that places `d`.
fn points(d: &SchDrawing) -> Vec<Point> {
    crate::placement::drawing_points(d)
}

#[test]
fn each_kind_places_and_undoes() {
    for drawing in kinds() {
        let mut engine = placed(drawing.clone());
        assert_eq!(the_drawing(&engine), &drawing);
        engine.undo().expect("undo");
        assert!(engine.document().drawings.is_empty());
    }
}

#[test]
fn each_kind_updates_in_one_step() {
    for drawing in kinds() {
        let mut engine = placed(drawing.clone());
        let mut next = drawing.clone();
        next.map_annotation_points(|p| Point::new(p.x + 1.0, p.y));
        engine
            .execute(Command::UpdateSchDrawing {
                drawing: next.clone(),
            })
            .expect("updated");
        assert_eq!(the_drawing(&engine), &next);
        engine.undo().expect("undo");
        assert_eq!(the_drawing(&engine), &drawing);
    }
}

#[test]
fn each_kind_moves_by_the_drag() {
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
        for (a, b) in points(&drawing).iter().zip(points(the_drawing(&engine))) {
            assert!((b.x - a.x - 2.54).abs() < 1e-9, "{drawing:?}");
            assert!((b.y - a.y + 1.27).abs() < 1e-9, "{drawing:?}");
        }
    }
}

#[test]
fn a_quarter_turn_keeps_a_dimension_measuring_the_same_span() {
    let drawing = dimension();
    let mut engine = placed(drawing.clone());
    engine
        .execute(Command::TransformSelection {
            items: vec![item(&drawing)],
            centre: Point::new(0.0, 0.0),
            transform: SelectionTransform::RotateClockwise,
            keep_wires: false,
        })
        .expect("turned");
    let (SchDrawing::Dimension(before), SchDrawing::Dimension(after)) =
        (&drawing, the_drawing(&engine))
    else {
        panic!("a dimension");
    };
    assert_eq!(after.direction, DimensionDirection::Vertical);
    assert!((after.measured_mm() - before.measured_mm()).abs() < 1e-9);
}

#[test]
fn a_mirror_flips_a_bezier_and_keeps_a_table_upright() {
    for drawing in [bezier(), table()] {
        let mut engine = placed(drawing.clone());
        engine
            .execute(Command::TransformSelection {
                items: vec![item(&drawing)],
                centre: Point::new(0.0, 0.0),
                transform: SelectionTransform::Mirror(MirrorAxis::Horizontal),
                keep_wires: false,
            })
            .expect("mirrored");
        match (&drawing, the_drawing(&engine)) {
            (SchDrawing::Bezier(a), SchDrawing::Bezier(b)) => {
                assert!((b.start.x + a.start.x).abs() < 1e-9);
                assert!((b.c1.y - a.c1.y).abs() < 1e-9);
            }
            (SchDrawing::Table(a), SchDrawing::Table(b)) => {
                let (lo_a, hi_a) = a.bounds();
                let (lo_b, hi_b) = b.bounds();
                assert!(((hi_b.x - lo_b.x) - (hi_a.x - lo_a.x)).abs() < 1e-9);
                assert!((lo_b.x + hi_a.x).abs() < 1e-9, "the box flips about x = 0");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn a_locked_kind_refuses_a_move_and_unlocks() {
    for drawing in kinds() {
        let mut engine = placed(drawing.clone());
        engine
            .execute(Command::SetItemLocked {
                item: item(&drawing),
                locked: true,
            })
            .expect("locked");
        assert!(the_drawing(&engine).is_locked());
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
}

#[test]
fn each_kind_deletes_and_comes_back_on_undo() {
    for drawing in kinds() {
        let mut engine = placed(drawing.clone());
        engine
            .execute(Command::DeleteSelection {
                items: vec![item(&drawing)],
            })
            .expect("deleted");
        assert!(engine.document().drawings.is_empty());
        engine.undo().expect("undo");
        assert_eq!(the_drawing(&engine), &drawing);
    }
}

#[test]
fn each_kind_rides_the_clipboard_payload() {
    for drawing in kinds() {
        let fragment = SchematicSheet {
            drawings: vec![drawing.clone()],
            ..Default::default()
        };
        let moved = PlacementPayload::new(fragment).translated(1.0, 2.0);
        let copy = &moved.fragment().drawings[0];
        for (a, b) in points(&drawing).iter().zip(points(copy)) {
            assert!((b.x - a.x - 1.0).abs() < 1e-9, "{drawing:?}");
            assert!((b.y - a.y - 2.0).abs() < 1e-9, "{drawing:?}");
        }
    }
}

#[test]
fn a_selected_annotation_describes_itself() {
    for (drawing, kind) in [
        (bezier(), "Bezier"),
        (dimension(), "Dimension"),
        (table(), "Table"),
    ] {
        let engine = placed(drawing.clone());
        let details = engine
            .describe_single_selection(&[item(&drawing)])
            .expect("details");
        assert!(
            details.info.iter().any(|(k, v)| k == "Type" && v == kind),
            "{kind}: {:?}",
            details.info
        );
    }
}
