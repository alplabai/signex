//! #2797 — length-matching groups: made, edited, renamed and removed as
//! board options (one undo step through `SetBoardOptions`), members moved
//! between groups, and each member reported against its group's band.

use serde_json::json;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_length::LengthTopology;
use uuid::Uuid;

use super::*;
use crate::pcb::{BoardCommand, BoardEngine};

/// DQ0 is 28.40 mm, DQ6 28.91 mm, CLK 10 mm; DQ7 has no copper.
fn board() -> PcbBoard {
    let track = |n: u128, y: f64, length: f64, net: u32| {
        json!({
            "uuid": Uuid::from_u128(n), "start": {"x": 0.0, "y": y},
            "end": {"x": length, "y": y}, "width": 0.2, "layer": "TopCopper", "net": net
        })
    };
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "DDR_DQ0"},
            {"number": 2, "name": "DDR_DQ6"},
            {"number": 3, "name": "DDR_DQ7"},
            {"number": 4, "name": "CLK"}
        ],
        "segments": [
            track(11, 0.0, 28.40, 1),
            track(12, 2.0, 28.91, 2),
            track(13, 4.0, 10.0, 4)
        ]
    }))
    .unwrap()
}

fn ddr() -> GroupEdit {
    GroupEdit {
        name: "DDR_DQ".into(),
        target_mm: Some(28.5),
        tolerance_mm: Some(0.25),
        members: Some(vec!["DDR_DQ0".into(), "DDR_DQ6".into(), "DDR_DQ7".into()]),
        ..GroupEdit::default()
    }
}

fn with(board: &PcbBoard, edit: &GroupEdit, n: u128) -> PcbBoard {
    let mut out = board.clone();
    out.options = set_group(board, edit, || Uuid::from_u128(n)).expect("edit");
    out
}

#[test]
fn a_new_group_reports_each_member_against_its_band() {
    let board = with(&board(), &ddr(), 100);
    let reports = report(&board);
    assert_eq!(reports.len(), 1);
    let group = &reports[0];
    assert_eq!(group.group.rule_name(), "MatchedLengths_DDR_DQ");
    assert_eq!(group.group.topology, LengthTopology::Chain);
    let status: Vec<MemberStatus> = group.members.iter().map(|m| m.status).collect();
    assert_eq!(
        status,
        [
            MemberStatus::InBand,
            MemberStatus::Over,
            MemberStatus::Unrouted
        ]
    );
    let dq6 = &group.members[1];
    assert!((dq6.delta_mm.unwrap() - 0.41).abs() < 1.0e-6, "{dq6:?}");
    assert_eq!(group.members[2].delta_mm, None, "an unrouted net has no Δ");
    assert_eq!(group.out_of_band(), 1);
    assert_eq!(group.unrouted(), 1);
}

#[test]
fn a_short_member_and_a_missing_net_are_told_apart() {
    let mut board = with(&board(), &ddr(), 100);
    board.options.length_groups[0].members.push("GONE".into());
    let clk = GroupEdit {
        name: "CLK".into(),
        target_mm: Some(12.0),
        tolerance_mm: Some(0.5),
        members: Some(vec!["CLK".into()]),
        ..GroupEdit::default()
    };
    let board = with(&board, &clk, 101);
    let reports = report(&board);
    assert_eq!(reports[0].members[3].status, MemberStatus::Missing);
    assert_eq!(reports[1].members[0].status, MemberStatus::Short);
    assert_eq!(MemberStatus::Short.token(), "short");
}

#[test]
fn a_new_group_needs_a_name_and_a_band() {
    let empty = GroupEdit::default();
    assert_eq!(
        set_group(&board(), &empty, Uuid::nil),
        Err(LengthGroupRefusal::NoName)
    );
    let bare = GroupEdit {
        name: "X".into(),
        target_mm: Some(1.0),
        ..GroupEdit::default()
    };
    assert_eq!(
        set_group(&board(), &bare, Uuid::nil),
        Err(LengthGroupRefusal::NewGroupNeedsBand)
    );
    let zero = GroupEdit {
        target_mm: Some(0.0),
        ..ddr()
    };
    assert_eq!(
        set_group(&board(), &zero, Uuid::nil),
        Err(LengthGroupRefusal::TargetNotPositive)
    );
    let unknown = GroupEdit {
        members: Some(vec!["NOPE".into()]),
        ..ddr()
    };
    assert_eq!(
        set_group(&board(), &unknown, Uuid::nil),
        Err(LengthGroupRefusal::UnknownNet("NOPE".into()))
    );
}

#[test]
fn an_edit_keeps_what_it_does_not_name() {
    let board = with(&board(), &ddr(), 100);
    let edit = GroupEdit {
        name: "DDR_DQ".into(),
        tolerance_mm: Some(0.5),
        topology: Some(LengthTopology::FlyBy),
        ..GroupEdit::default()
    };
    let board = with(&board, &edit, 999);
    let group = &board.options.length_groups[0];
    assert_eq!(group.uuid, Uuid::from_u128(100), "the same group");
    assert_eq!(group.target_mm, 28.5);
    assert_eq!(group.tolerance_mm, 0.5);
    assert_eq!(group.topology, LengthTopology::FlyBy);
    assert_eq!(group.members.len(), 3);
    assert_eq!(
        report(&board)[0].out_of_band(),
        0,
        "DQ6 is in the wider band"
    );
}

#[test]
fn listing_a_net_of_another_group_moves_it() {
    let board = with(&board(), &ddr(), 100);
    let clk = GroupEdit {
        name: "CLK".into(),
        target_mm: Some(10.0),
        tolerance_mm: Some(0.1),
        members: Some(vec!["CLK".into(), " DDR_DQ7 ".into(), "CLK".into()]),
        ..GroupEdit::default()
    };
    let board = with(&board, &clk, 101);
    let groups = &board.options.length_groups;
    assert_eq!(groups[0].members, ["DDR_DQ0", "DDR_DQ6"]);
    assert_eq!(groups[1].members, ["CLK", "DDR_DQ7"], "trimmed, once each");
}

#[test]
fn a_rename_may_not_take_another_groups_name() {
    let board = with(&board(), &ddr(), 100);
    let clk = GroupEdit {
        name: "CLK".into(),
        target_mm: Some(10.0),
        tolerance_mm: Some(0.1),
        ..GroupEdit::default()
    };
    let board = with(&board, &clk, 101);
    let rename = GroupEdit {
        name: "CLK".into(),
        rename: Some("DDR_DQ".into()),
        ..GroupEdit::default()
    };
    assert_eq!(
        set_group(&board, &rename, Uuid::nil),
        Err(LengthGroupRefusal::NameTaken("DDR_DQ".into()))
    );
    let rename = GroupEdit {
        rename: Some("SYSCLK".into()),
        ..rename
    };
    let board = with(&board, &rename, 999);
    assert_eq!(board.options.length_groups[1].name, "SYSCLK");
}

#[test]
fn removing_a_group_keeps_its_nets_and_is_one_undo_step() {
    let start = board();
    let made = with(&start, &ddr(), 100);
    let mut engine = BoardEngine::new(made.clone(), 1);
    let options = remove_group(engine.board(), "DDR_DQ").expect("removed");
    assert!(options.length_groups.is_empty());
    engine
        .execute(BoardCommand::SetBoardOptions {
            options: Box::new(options),
        })
        .expect("set");
    assert!(engine.board().options.length_groups.is_empty());
    assert_eq!(engine.board().nets, made.nets, "the nets stay");
    engine.undo().expect("undo");
    assert_eq!(engine.board().options, made.options);
    assert_eq!(
        remove_group(&start, "DDR_DQ"),
        Err(LengthGroupRefusal::NoSuchGroup("DDR_DQ".into()))
    );
}

#[test]
fn the_engine_refuses_a_group_the_file_cannot_hold() {
    let mut options = with(&board(), &ddr(), 100).options;
    options.length_groups[0].tolerance_mm = -1.0;
    let mut engine = BoardEngine::new(board(), 1);
    let refused = engine.execute(BoardCommand::SetBoardOptions {
        options: Box::new(options),
    });
    assert!(refused.is_err(), "{refused:?}");
    assert!(engine.board().options.length_groups.is_empty());
}

/// A target under half a nm is stored as 0 nm, which the file refuses on
/// load: the edit and the options command refuse it, judged on the stored
/// value.
#[test]
fn a_target_that_rounds_to_zero_nm_is_refused() {
    let tiny = GroupEdit {
        target_mm: Some(4e-7),
        ..ddr()
    };
    assert_eq!(
        set_group(&board(), &tiny, Uuid::nil),
        Err(LengthGroupRefusal::TargetNotPositive)
    );
    let mut options = with(&board(), &ddr(), 100).options;
    options.length_groups[0].target_mm = 4e-7;
    let mut engine = BoardEngine::new(board(), 1);
    let refused = engine.execute(BoardCommand::SetBoardOptions {
        options: Box::new(options),
    });
    assert!(refused.is_err(), "{refused:?}");
}
