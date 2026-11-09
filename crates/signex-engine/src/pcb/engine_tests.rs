use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::tests::{FP_A, FP_B, SEG_1, SEG_2, board, commands, id, snapshot};
use super::*;

fn engine() -> BoardEngine {
    BoardEngine::new(board(), 10)
}

fn move_to(x: f64) -> BoardCommand {
    BoardCommand::MoveFootprint {
        footprint: id(FP_A),
        to: Point { x, y: x / 3.0 },
    }
}

/// Every variant, run through the engine, undone and redone. `RestoreFace`
/// and `RestoreItems` are inverses, so they are taken from a scratch board
/// and run on the board their forward command left behind.
fn every_variant() -> Vec<(String, PcbBoard, BoardCommand)> {
    let mut all: Vec<_> = commands()
        .into_iter()
        .map(|(name, command)| (name.to_string(), board(), command))
        .collect();
    let mut flipped = board();
    let flip = BoardCommand::Flip {
        footprint: id(FP_A),
    }
    .apply(&mut flipped)
    .unwrap();
    assert!(matches!(flip.inverse, BoardCommand::RestoreFace { .. }));
    all.push(("restore face".into(), flipped, flip.inverse));
    let mut deleted = board();
    let delete = BoardCommand::DeleteItems {
        items: vec![
            BoardItemId::Segment(id(SEG_2)),
            BoardItemId::Footprint(id(FP_B)),
        ],
    }
    .apply(&mut deleted)
    .unwrap();
    assert!(matches!(delete.inverse, BoardCommand::RestoreItems { .. }));
    all.push(("restore items".into(), deleted, delete.inverse));
    all
}

#[test]
fn every_variant_round_trips_through_execute_undo_redo() {
    for (name, start, command) in every_variant() {
        let mut engine = BoardEngine::new(start, 1);
        let before = snapshot(engine.board());
        engine.execute(command).unwrap();
        let after = snapshot(engine.board());
        assert_ne!(before, after, "{name}: the command changed nothing");

        assert!(engine.undo().unwrap().is_some());
        assert_eq!(snapshot(engine.board()), before, "{name}: undo");
        assert!(engine.redo().unwrap().is_some());
        assert_eq!(snapshot(engine.board()), after, "{name}: redo");
        // And again: the redo step must undo exactly too.
        engine.undo().unwrap();
        assert_eq!(snapshot(engine.board()), before, "{name}: second undo");
    }
}

#[test]
fn a_batch_is_one_step_undone_whole_and_in_order() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    engine
        .execute_batch(vec![
            move_to(1.1),
            BoardCommand::Flip {
                footprint: id(FP_A),
            },
            move_to(2.2),
            BoardCommand::DeleteItems {
                items: vec![BoardItemId::Segment(id(SEG_1))],
            },
        ])
        .unwrap();
    assert_eq!(engine.undo_len(), 1);
    let after = snapshot(engine.board());
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
    engine.redo().unwrap();
    assert_eq!(snapshot(engine.board()), after);
}

#[test]
fn a_refused_command_changes_nothing() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    let revision = engine.revision();
    // The second command is refused: the first must be put back.
    let refused = engine.execute_batch(vec![
        move_to(5.0),
        BoardCommand::Rotate {
            footprint: id("00000000-0000-0000-0000-0000000000ff"),
            degrees: 1.0,
        },
    ]);
    assert!(matches!(
        refused,
        Err(BoardCommandError::FootprintNotFound(_))
    ));
    assert_eq!(snapshot(engine.board()), before);
    assert_eq!(engine.revision(), revision);
    assert!(!engine.can_undo());
    assert!(engine.is_at_save_point());
}

#[test]
fn the_revision_changes_on_every_execute_undo_and_redo_and_never_repeats() {
    let mut engine = engine();
    let mut seen = vec![engine.revision()];
    engine.execute(move_to(1.0)).unwrap();
    seen.push(engine.revision());
    engine.execute(move_to(2.0)).unwrap();
    seen.push(engine.revision());
    engine.undo().unwrap();
    seen.push(engine.revision());
    engine.redo().unwrap();
    seen.push(engine.revision());
    engine.undo().unwrap();
    seen.push(engine.revision());
    engine.undo().unwrap();
    seen.push(engine.revision());
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
    // Nothing to undo, a refusal and an empty batch move nothing.
    let last = engine.revision();
    assert!(engine.undo().unwrap().is_none());
    engine.execute_batch(Vec::new()).unwrap();
    assert!(
        engine
            .execute(BoardCommand::Flip {
                footprint: Uuid::nil()
            })
            .is_err()
    );
    assert_eq!(engine.revision(), last);
}

#[test]
fn undo_back_to_the_save_point_reads_clean_and_redo_dirty_again() {
    let mut engine = engine();
    assert!(engine.is_at_save_point());
    engine.execute(move_to(1.0)).unwrap();
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
    engine.redo().unwrap();
    assert!(!engine.is_at_save_point());

    engine.mark_saved();
    assert!(engine.is_at_save_point());
    engine.execute(move_to(2.0)).unwrap();
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
    // Below the point and a new edit: the point is gone for good.
    engine.undo().unwrap();
    engine.execute(move_to(3.0)).unwrap();
    engine.undo().unwrap();
    assert!(!engine.is_at_save_point());
}

#[test]
fn an_unsaved_board_is_dirty_until_it_is_saved() {
    let mut engine = BoardEngine::unsaved(board(), 1);
    assert!(!engine.is_at_save_point());
    engine.mark_saved();
    assert!(engine.is_at_save_point());
    engine.forget_save_point();
    assert!(!engine.is_at_save_point());
}

#[test]
fn the_depth_caps_the_steps_and_lowering_it_trims_the_oldest() {
    let mut engine = engine();
    engine.set_undo_depth(3);
    for i in 0..5 {
        engine.execute(move_to(i as f64 + 1.0)).unwrap();
    }
    assert_eq!(engine.undo_len(), 3);
    assert_eq!(engine.undo_depth(), 3);
    // The save point (the empty history) fell off the bottom: three undos
    // do not reach it, and the board never gets back to the loaded state.
    while engine.undo().unwrap().is_some() {}
    assert!(!engine.is_at_save_point());
    assert_ne!(engine.board().footprints[0].position.x, 10.1);

    let mut engine = BoardEngine::new(board(), 1);
    for i in 0..6 {
        engine.execute(move_to(i as f64 + 1.0)).unwrap();
    }
    engine.set_undo_depth(2);
    assert_eq!(engine.undo_len(), 2);
}

#[test]
fn a_labelled_step_with_a_stale_revision_is_refused_whole() {
    let mut engine = engine();
    let first = engine.revision();
    engine.execute(move_to(1.0)).unwrap();
    let before = snapshot(engine.board());
    let current = engine.revision();
    let refused = engine.execute_labelled(vec![move_to(9.0)], "Agent: nudge", first);
    assert_eq!(
        refused,
        Err(BoardCommandError::StaleRevision {
            expected: first,
            current
        })
    );
    assert_eq!(snapshot(engine.board()), before);
    assert_eq!(engine.revision(), current);
    assert_eq!(engine.undo_len(), 1);
    assert_eq!(engine.undo_label(), None);
}

#[test]
fn a_labelled_step_is_one_undo_entry_that_carries_its_label() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    let at = engine.revision();
    engine
        .execute_labelled(
            vec![
                move_to(1.0),
                BoardCommand::Flip {
                    footprint: id(FP_B),
                },
            ],
            "Agent: tidy",
            at,
        )
        .unwrap();
    assert_eq!(engine.undo_len(), 1);
    assert_eq!(engine.undo_label(), Some("Agent: tidy"));
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
    assert_eq!(engine.undo_label(), None);
    assert_eq!(engine.redo_label(), Some("Agent: tidy"));
    engine.redo().unwrap();
    assert_eq!(engine.undo_label(), Some("Agent: tidy"));
    // A new edit after an undo cuts the redo branch and its label.
    engine.undo().unwrap();
    engine.execute(move_to(7.0)).unwrap();
    assert_eq!(engine.redo_label(), None);
    assert_eq!(engine.undo_label(), None);
}

#[test]
fn a_failed_rollback_takes_a_new_revision_and_forgets_the_save_point() {
    let mut engine = engine();
    let revision = engine.revision();
    // A refusal that rolled back cleanly changes neither.
    engine.note_failure(&BoardCommandError::FootprintNotFound(Uuid::nil()));
    assert_eq!(engine.revision(), revision);
    assert!(engine.is_at_save_point());
    // A rollback that failed left a board no revision describes.
    engine.note_failure(&BoardCommandError::RollbackFailed("x".into()));
    assert!(engine.revision() > revision);
    assert!(!engine.is_at_save_point());
}
