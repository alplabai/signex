//! #2234 — the board sketch as one undoable, batch-checked command.

use signex_types::pcb::Point;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_sketch::{BakedBoard, PcbCutout, PcbHole, PcbKeepout, PcbVScore};
use signex_types::sketch::SketchData;
use signex_types::sketch::attr::{KeepoutKinds, VScoreSide};
use signex_types::sketch::entity::{Entity, EntityKind};
use signex_types::sketch::id::SketchEntityId;
use signex_types::sketch::plane::PlaneId;
use uuid::Uuid;

use super::tests::{FP_A, board, id, snapshot};
use super::*;

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// Inexact values (0.1, 1/3) so only a bit-exact undo passes.
fn ring(at: f64) -> Vec<Point> {
    vec![p(at, at), p(at + 0.1, at), p(at + 0.1, at + 1.0 / 3.0)]
}

fn sketch() -> SketchData {
    let mut data = SketchData::default();
    let mut edge = Entity::new(
        SketchEntityId::new(),
        PlaneId::new(),
        EntityKind::Point {
            x: 0.1,
            y: 1.0 / 3.0,
        },
    );
    edge.board_outline = true;
    data.entities.push(edge);
    data
}

fn baked(outline: Option<Vec<Point>>) -> BakedBoard {
    BakedBoard {
        outline,
        keepouts: vec![PcbKeepout {
            uuid: Uuid::from_u128(0x501),
            boundary: ring(2.0),
            layers: vec![PcbLayer::Copper(CopperLayer::Top)],
            forbids: KeepoutKinds::ALL_COPPER,
        }],
        cutouts: vec![PcbCutout {
            uuid: Uuid::from_u128(0x502),
            boundary: ring(5.0),
            edge_radius: 0.1,
            through: true,
        }],
        holes: vec![PcbHole {
            uuid: Uuid::from_u128(0x503),
            position: p(0.1, 0.2),
            drill: 3.2,
            plated: true,
            net: Some(1),
            unknown_columns: Default::default(),
        }],
        v_scores: vec![PcbVScore {
            uuid: Uuid::from_u128(0x504),
            from: p(0.0, 15.1),
            to: p(50.1, 15.1),
            depth_fraction: 1.0 / 3.0,
            min_web: 0.0,
            side: VScoreSide::Both,
        }],
    }
}

fn with_outline() -> BoardCommand {
    BoardCommand::set_board_sketch(sketch(), baked(Some(ring(0.3))))
}

fn engine() -> BoardEngine {
    BoardEngine::new(board(), 1)
}

#[test]
fn one_step_writes_the_sketch_its_geometry_and_the_outline() {
    let mut engine = engine();
    let patch = engine.execute(with_outline()).unwrap();
    assert_eq!(patch.kind, BoardPatchKind::BoardSketchReplaced);
    assert!(patch.items.is_empty());
    assert_eq!(engine.undo_len(), 1, "one undo step");

    let stored = engine.board().sketch.as_ref().expect("a sketch");
    let want = baked(None);
    assert_eq!(stored.keepouts, want.keepouts);
    assert_eq!(stored.cutouts, want.cutouts);
    assert_eq!(stored.holes, want.holes);
    assert_eq!(stored.v_scores, want.v_scores);
    assert!(stored.owns_outline(), "the bake is recorded on the sketch");
    assert_eq!(
        engine.board().outline,
        ring(0.3),
        "every reader of the board outline sees the baked one"
    );
}

#[test]
fn undo_and_redo_are_bit_exact() {
    for command in [
        with_outline(),
        BoardCommand::set_board_sketch(sketch(), baked(None)),
        BoardCommand::set_board_sketch(SketchData::default(), BakedBoard::default()),
    ] {
        let mut engine = engine();
        let before = snapshot(engine.board());
        engine.execute(command.clone()).unwrap();
        let after = snapshot(engine.board());
        assert_ne!(before, after, "{command:?} changed nothing");

        engine.undo().unwrap().expect("a step to undo");
        assert_eq!(snapshot(engine.board()), before, "undo of {command:?}");
        engine.redo().unwrap().expect("a step to redo");
        assert_eq!(snapshot(engine.board()), after, "redo of {command:?}");
    }
}

#[test]
fn replacing_and_removing_a_sketch_undo_to_the_previous_one() {
    let mut engine = engine();
    engine.execute(with_outline()).unwrap();
    let first = snapshot(engine.board());

    engine
        .execute(BoardCommand::set_board_sketch(
            sketch(),
            baked(Some(ring(7.0))),
        ))
        .unwrap();
    engine.execute(BoardCommand::remove_board_sketch()).unwrap();
    assert!(engine.board().sketch.is_none());
    assert_eq!(engine.board().outline, ring(7.0), "the last bake stays");

    engine.undo().unwrap();
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), first);
}

#[test]
fn a_sketch_without_an_outline_leaves_the_board_outline_alone() {
    let mut engine = engine();
    let outline = engine.board().outline.clone();
    engine
        .execute(BoardCommand::set_board_sketch(
            SketchData::default(),
            baked(None),
        ))
        .unwrap();
    assert_eq!(engine.board().outline, outline);
    assert!(!engine.board().sketch.as_ref().unwrap().owns_outline());
    engine
        .execute(BoardCommand::SetBoardOutline { outline: ring(9.0) })
        .expect("the outline is still the board's own");
}

#[test]
fn the_outline_command_is_refused_while_the_sketch_owns_the_outline() {
    let mut engine = engine();
    engine.execute(with_outline()).unwrap();
    let held = snapshot(engine.board());
    let refused = engine.execute(BoardCommand::SetBoardOutline { outline: ring(9.0) });
    assert_eq!(refused, Err(BoardCommandError::OutlineOwnedBySketch));
    assert_eq!(snapshot(engine.board()), held);
    assert_eq!(engine.undo_len(), 1, "a refusal is no step");

    engine.execute(BoardCommand::remove_board_sketch()).unwrap();
    engine
        .execute(BoardCommand::SetBoardOutline { outline: ring(9.0) })
        .expect("without a sketch the outline is editable again");
}

#[test]
fn a_non_finite_sketch_is_refused_by_check_batch_and_execute() {
    let mut bad = baked(Some(ring(0.3)));
    bad.holes[0].drill = f64::NAN;
    let command = BoardCommand::set_board_sketch(sketch(), bad);
    let mut engine = engine();
    let refusal = engine
        .check_batch(std::slice::from_ref(&command))
        .unwrap_err();
    assert_eq!(refusal.index, 0);
    assert!(matches!(
        refusal.error,
        BoardCommandError::NonFiniteSketch(_)
    ));

    let before = snapshot(engine.board());
    assert!(engine.execute(command).is_err());
    assert_eq!(snapshot(engine.board()), before);

    let outline = BoardCommand::SetBoardSketch {
        sketch: None,
        outline: Some(vec![p(f64::INFINITY, 0.0)]),
    };
    assert_eq!(
        engine.execute(outline),
        Err(BoardCommandError::NonFinite { field: "outline" })
    );
}

#[test]
fn check_batch_passes_a_good_sketch_and_changes_nothing() {
    let engine = engine();
    let before = snapshot(engine.board());
    engine.check_batch(&[with_outline()]).unwrap();
    assert_eq!(snapshot(engine.board()), before);
}

#[test]
fn a_batch_with_a_refused_command_rolls_the_sketch_back() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    let batch = vec![
        with_outline(),
        BoardCommand::MoveFootprint {
            footprint: Uuid::from_u128(0xdead),
            to: p(1.0, 1.0),
        },
    ];
    assert_eq!(engine.check_batch(&batch).unwrap_err().index, 1);
    assert!(engine.execute_batch(batch).is_err());
    assert_eq!(snapshot(engine.board()), before, "all or nothing");
    assert!(!engine.can_undo());

    let together = vec![
        with_outline(),
        BoardCommand::MoveFootprint {
            footprint: id(FP_A),
            to: p(1.0, 1.0),
        },
    ];
    engine.execute_batch(together).unwrap();
    assert_eq!(engine.undo_len(), 1, "the sketch and the move are one step");
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
}
