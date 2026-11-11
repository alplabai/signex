use signex_engine::pcb::designators::{ReannotatePlan, Rename};
use uuid::Uuid;

use super::*;

fn state(renames: usize, problem: Option<ReannotateProblem>) -> BoardReannotateState {
    let rename = |n: usize| Rename {
        footprint: Uuid::from_u128(n as u128),
        from: format!("R{}", n + 1),
        to: format!("R{n}"),
    };
    BoardReannotateState {
        board: "board.snxpcb".into(),
        board_name: "board".into(),
        options: ReannotateOptions::default(),
        plan: ReannotatePlan {
            renames: (0..renames).map(rename).collect(),
            ..ReannotatePlan::default()
        },
        revision: 1,
        problem,
    }
}

#[test]
fn execute_needs_a_rename_and_a_board_that_is_not_read_only() {
    assert!(state(1, None).can_execute());
    assert!(!state(0, None).can_execute(), "nothing to rename");
    assert!(!state(1, Some(ReannotateProblem::ReadOnly)).can_execute());
    assert!(
        state(1, Some(ReannotateProblem::Stale)).can_execute(),
        "a remade list can run"
    );
    assert!(state(1, Some(ReannotateProblem::Refused("x".into()))).can_execute());
}
