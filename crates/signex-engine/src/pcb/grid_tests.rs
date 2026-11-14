//! #2353: the board's snap grid step as one undoable edit.

use signex_types::pcb::{PcbBoard, PcbSetup};
use uuid::Uuid;

use super::{BoardCommand, BoardCommandError, BoardEngine};

fn board(setup: bool) -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap();
    board.setup = setup.then_some(PcbSetup {
        grid_size: 1.27,
        trace_width: 0.25,
        via_diameter: 0.6,
        via_drill: 0.3,
        clearance: 0.2,
        track_min_width: 0.1,
        via_min_diameter: 0.4,
        via_min_drill: 0.2,
    });
    board
}

fn grid(engine: &BoardEngine) -> f64 {
    engine.board().setup.as_ref().unwrap().grid_size
}

#[test]
fn a_grid_step_is_one_undo_step() {
    let mut engine = BoardEngine::new(board(true), 1);
    engine
        .execute(BoardCommand::SetGridSize { grid_mm: 0.5 })
        .expect("a positive step is set");
    assert_eq!(grid(&engine), 0.5);
    engine.undo().expect("undo runs");
    assert_eq!(grid(&engine), 1.27);
    engine.redo().expect("redo runs");
    assert_eq!(grid(&engine), 0.5);
}

#[test]
fn a_bad_step_or_a_board_without_setup_is_refused() {
    let mut engine = BoardEngine::new(board(true), 1);
    assert_eq!(
        engine.execute(BoardCommand::SetGridSize { grid_mm: -1.0 }),
        Err(BoardCommandError::NegativeGridStep(-1.0))
    );
    assert!(
        engine
            .execute(BoardCommand::SetGridSize { grid_mm: f64::NAN })
            .is_err()
    );
    assert_eq!(grid(&engine), 1.27);
    let mut bare = BoardEngine::new(board(false), 1);
    assert_eq!(
        bare.execute(BoardCommand::SetGridSize { grid_mm: 0.5 }),
        Err(BoardCommandError::NoSetup)
    );
}
