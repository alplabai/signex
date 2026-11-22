//! DECISIONS 2026-11-22 (#2840): a recorded board edit runs again only while
//! its targets are as it found or left them.

use signex_types::pcb::{PcbBoard, Point, Segment};

use super::tests::{FP_A, SEG_1, SEG_2, board, id};
use super::*;

fn track(b: &PcbBoard, uuid: &str) -> Segment {
    b.segments
        .iter()
        .find(|s| s.uuid == id(uuid))
        .unwrap()
        .clone()
}

fn with_width(b: &PcbBoard, uuid: &str, width: f64) -> BoardItem {
    BoardItem::Segment(Segment {
        width,
        ..track(b, uuid)
    })
}

fn moved(b: &PcbBoard, uuid: &str) -> BoardCommand {
    let mut segment = track(b, uuid);
    segment.start.x += 1.0;
    BoardCommand::ReplaceItems {
        items: vec![BoardItem::Segment(segment)],
    }
}

/// Run `commands` on `b`, recording as Replay does.
fn recorded(b: &mut PcbBoard, commands: Vec<BoardCommand>) -> BoardReplayGuard {
    let mut guard = BoardReplayGuard::before(b, &commands);
    for command in commands {
        command.apply(b).unwrap();
    }
    guard.record_after(b);
    guard
}

fn widened(b: &mut PcbBoard, uuid: &str) -> BoardReplayGuard {
    let items = vec![with_width(b, uuid, 0.5)];
    recorded(b, vec![BoardCommand::ReplaceItems { items }])
}

#[test]
fn a_track_width_edit_is_refused_after_a_later_move_of_that_track() {
    let mut b = board();
    let guard = widened(&mut b, SEG_1);
    assert!(guard.holds(&b), "as the edit left it");

    moved(&b, SEG_1).apply(&mut b).unwrap();

    assert!(!guard.holds(&b));
}

#[test]
fn an_edit_may_run_on_its_target_as_it_found_it() {
    let mut b = board();
    let found = b.clone();
    let guard = widened(&mut b, SEG_1);

    assert!(guard.holds(&found), "the edit was undone");
}

#[test]
fn a_change_to_another_item_does_not_refuse_the_edit() {
    let mut b = board();
    let guard = widened(&mut b, SEG_1);

    moved(&b, SEG_2).apply(&mut b).unwrap();

    assert!(guard.holds(&b));
}

#[test]
fn one_changed_target_refuses_the_whole_row() {
    let mut b = board();
    let items = vec![with_width(&b, SEG_1, 0.5), with_width(&b, SEG_2, 0.5)];
    let guard = recorded(&mut b, vec![BoardCommand::ReplaceItems { items }]);

    moved(&b, SEG_2).apply(&mut b).unwrap();

    assert!(!guard.holds(&b));
}

#[test]
fn a_target_that_is_gone_refuses_the_edit() {
    let mut b = board();
    let guard = widened(&mut b, SEG_1);

    BoardCommand::DeleteItems {
        items: vec![BoardItemId::Segment(id(SEG_1))],
    }
    .apply(&mut b)
    .unwrap();

    assert!(!guard.holds(&b));
}

#[test]
fn two_edits_of_one_target_in_a_dispatch_keep_the_first_before_and_last_after() {
    let mut b = board();
    let found = b.clone();
    let first = widened(&mut b, SEG_1);
    let items = vec![with_width(&b, SEG_1, 0.7)];
    let second = recorded(&mut b, vec![BoardCommand::ReplaceItems { items }]);

    let guard = first.merged(second);

    assert!(guard.holds(&b), "as the dispatch left it");
    assert!(guard.holds(&found), "as the dispatch found it");
    let mut between = found.clone();
    let items = vec![with_width(&between, SEG_1, 0.5)];
    BoardCommand::ReplaceItems { items }
        .apply(&mut between)
        .unwrap();
    assert!(!guard.holds(&between), "a width neither end held");
}

/// `first`, then `later` writing the same target to another value.
fn refused_after(first: BoardCommand, later: BoardCommand) {
    let mut b = board();
    let guard = recorded(&mut b, vec![first]);
    assert!(guard.holds(&b));
    later.apply(&mut b).unwrap();
    assert!(!guard.holds(&b));
}

#[test]
fn each_kind_of_absolute_target_refuses_once_it_changed() {
    let fp = id(FP_A);
    let to = |x| Point { x, y: 1.0 };
    refused_after(
        BoardCommand::MoveFootprint {
            footprint: fp,
            to: to(5.0),
        },
        BoardCommand::MoveFootprint {
            footprint: fp,
            to: to(6.0),
        },
    );
    refused_after(
        BoardCommand::Rotate {
            footprint: fp,
            degrees: 10.0,
        },
        BoardCommand::Rotate {
            footprint: fp,
            degrees: 20.0,
        },
    );
    let text = |reference: &str| BoardCommand::SetFootprintText {
        footprint: fp,
        reference: reference.into(),
        value: "10k".into(),
    };
    refused_after(text("R8"), text("R9"));
    refused_after(
        BoardCommand::SetBoardOutline {
            outline: vec![to(0.0)],
        },
        BoardCommand::SetBoardOutline {
            outline: vec![to(1.0)],
        },
    );
    refused_after(
        BoardCommand::SetGridSize { grid_mm: 1.0 },
        BoardCommand::SetGridSize { grid_mm: 2.0 },
    );
}

#[test]
fn a_command_that_cannot_be_checked_is_refused() {
    let b = board();
    let mut guard =
        BoardReplayGuard::before(&b, &[BoardCommand::RestoreItems { items: Vec::new() }]);
    guard.record_after(&b);

    assert!(!guard.holds(&b));
}

#[test]
fn a_command_with_no_recorded_state_is_not_refused() {
    let mut b = board();
    let guard = recorded(
        &mut b,
        vec![BoardCommand::Flip {
            footprint: id(FP_A),
        }],
    );

    assert!(guard.holds(&b));
}
