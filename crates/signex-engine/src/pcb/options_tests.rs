//! #2374: the board options as one undoable edit.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_options::{BoardGrid, BoardGuide, BoardOptions, GuideKind, MARK_COLORS};
use uuid::Uuid;

use super::super::{BoardCommand, BoardCommandError, BoardEngine};

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap()
}

fn with_guide(x: f64) -> BoardOptions {
    BoardOptions {
        guides: vec![BoardGuide::new(
            Uuid::from_u128(1),
            GuideKind::Vertical,
            Point::new(x, 0.0),
        )],
        ..BoardOptions::default()
    }
}

fn set(options: BoardOptions) -> BoardCommand {
    BoardCommand::SetBoardOptions {
        options: Box::new(options),
    }
}

#[test]
fn new_options_are_one_undo_step() {
    let mut engine = BoardEngine::new(board(), 1);
    engine.execute(set(with_guide(4.0))).expect("set");
    assert_eq!(engine.board().options, with_guide(4.0));
    engine.undo().expect("undo runs");
    assert!(engine.board().options.is_default());
    engine.redo().expect("redo runs");
    assert_eq!(engine.board().options, with_guide(4.0));
}

#[test]
fn bad_options_are_refused_and_change_nothing() {
    let mut engine = BoardEngine::new(board(), 1);
    assert_eq!(
        engine.execute(set(with_guide(f64::INFINITY))),
        Err(BoardCommandError::NonFinite { field: "guide x" })
    );
    let mut twice = with_guide(1.0);
    twice.guides.push(twice.guides[0].clone());
    assert_eq!(
        engine.execute(set(twice)),
        Err(BoardCommandError::DuplicateOption(Uuid::from_u128(1)))
    );
    let flat_grid = BoardOptions {
        grids: vec![BoardGrid {
            uuid: Uuid::from_u128(2),
            name: "G".into(),
            step_mm: 0.0,
            origin: Point::ZERO,
            color: MARK_COLORS[0],
            components: true,
            non_components: true,
        }],
        ..BoardOptions::default()
    };
    assert_eq!(
        engine.execute(set(flat_grid)),
        Err(BoardCommandError::NotPositive { field: "grid step" })
    );
    let mut sheet = BoardOptions::default();
    sheet.sheet.height = -1.0;
    assert!(engine.execute(set(sheet)).is_err());
    assert!(engine.board().options.is_default());
}
