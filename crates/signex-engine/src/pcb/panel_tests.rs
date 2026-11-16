//! #2410: the board's panel as one undoable edit.

use signex_types::pcb::PcbBoard;
use signex_types::pcb_panel::{BoardPanel, PanelError};
use uuid::Uuid;

use super::super::{BoardCommand, BoardCommandError, BoardEngine, BoardPatchKind};

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap()
}

fn set(panel: Option<BoardPanel>) -> BoardCommand {
    BoardCommand::SetBoardPanel {
        panel: panel.map(Box::new),
    }
}

#[test]
fn a_panel_is_one_undo_step_and_its_removal_another() {
    let mut engine = BoardEngine::new(board(), 1);
    let patch = engine
        .execute(set(Some(BoardPanel::default())))
        .expect("set");
    assert_eq!(patch.kind, BoardPatchKind::PanelChanged);
    assert_eq!(engine.board().options.panel, Some(BoardPanel::default()));
    engine.execute(set(None)).expect("removed");
    assert_eq!(engine.board().options.panel, None);
    engine.undo().expect("undo runs");
    assert_eq!(engine.board().options.panel, Some(BoardPanel::default()));
    engine.undo().expect("undo runs");
    assert_eq!(engine.board().options.panel, None);
    assert!(engine.board().options.is_default());
    engine.redo().expect("redo runs");
    assert_eq!(engine.board().options.panel, Some(BoardPanel::default()));
}

#[test]
fn a_panel_that_breaks_a_rule_is_refused_and_changes_nothing() {
    let mut engine = BoardEngine::new(board(), 1);
    let mut panel = BoardPanel::default();
    panel.rows = 0;
    assert_eq!(
        engine.execute(set(Some(panel))),
        Err(BoardCommandError::Panel(PanelError::Count {
            field: "rows",
            min: 1,
            max: signex_types::pcb_panel::MAX_PANEL_COPIES,
        }))
    );
    assert_eq!(engine.board().options.panel, None);
    assert_eq!(engine.undo_len(), 0);
}
