//! #1312 — Back Annotate's change list and the sheet commands it makes.

use signex_types::pcb::PcbBoard;
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::super::fixture::*;
use super::*;
use crate::Engine;

fn back(part: EcoPart) -> BackPart {
    BackPart {
        eco: part,
        shared_sheet: false,
    }
}

/// R1 (10k), R2 (4k7) and U1 (two units), all on the root sheet.
fn parts() -> Vec<BackPart> {
    vec![
        back(part("R1", &[0x101], "10k", Ok(r0603()))),
        back(part("R2", &[0x102], "4k7", Ok(r0603()))),
        back(part("U1", &[0x201, 0x202], "LM", Ok(sot23()))),
    ]
}

/// The board as Update PCB left it: every part placed and linked.
fn in_step() -> PcbBoard {
    board(vec![
        board_fp(0xa1, "R1", "10k", &[0x101], &r0603(), (10.0, 10.0)),
        board_fp(0xa2, "R2", "4k7", &[0x102], &r0603(), (20.0, 10.0)),
        board_fp(0xa3, "U1", "LM", &[0x201, 0x202], &sot23(), (30.0, 10.0)),
    ])
}

fn diff(board: &PcbBoard, parts: &[BackPart]) -> BackChangeList {
    diff_schematic(&BackInput {
        board,
        revision: 7,
        parts,
        unreadable: &[],
    })
}

fn fp_mut(board: &mut PcbBoard, n: u128) -> &mut signex_types::pcb::Footprint {
    board
        .footprints
        .iter_mut()
        .find(|f| f.uuid == u(n))
        .unwrap()
}

#[test]
fn a_board_in_step_with_the_design_has_no_rows() {
    let list = diff(&in_step(), &parts());
    assert!(list.rows.is_empty(), "{:?}", list.rows);
    assert!(list.notes.is_empty(), "{:?}", list.notes);
    assert_eq!(list.revision, 7);
}

#[test]
fn a_board_designator_and_value_become_ticked_rows() {
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).reference = "R9".into();
    fp_mut(&mut board, 0xa2).value = "1k".into();
    let list = diff(&board, &parts());
    assert_eq!(
        list.rows
            .iter()
            .map(|r| (r.key, r.applies()))
            .collect::<Vec<_>>(),
        vec![
            (BackKey::Designator(u(0xa1)), true),
            (BackKey::Value(u(0xa2)), true)
        ]
    );
    assert_eq!(
        list.rows[0].change,
        BackChange::Designator {
            footprint: u(0xa1),
            sheet: root(),
            units: vec![u(0x101)],
            from: "R1".into(),
            to: "R9".into(),
        }
    );
    assert_eq!(
        list.sheet_edits(),
        vec![
            SheetEdit {
                placement: root(),
                units: vec![u(0x101)],
                designator: Some("R9".into()),
                value: None,
            },
            SheetEdit {
                placement: root(),
                units: vec![u(0x102)],
                designator: None,
                value: Some("1k".into()),
            },
        ]
    );
}

#[test]
fn unannotated_or_duplicate_schematic_designators_block_their_rows() {
    let mut parts = parts();
    parts[0] = back(part("R?", &[0x101], "10k", Ok(r0603())));
    parts[1] = back(part("U1", &[0x102], "4k7", Ok(r0603())));
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).reference = "R5".into();
    fp_mut(&mut board, 0xa2).reference = "R6".into();
    let list = diff(&board, &parts);
    let blocked: Vec<_> = list.rows.iter().map(|r| r.blocked.clone()).collect();
    assert_eq!(
        blocked,
        vec![
            Some(BackBlock::NeedsAnnotation),
            Some(BackBlock::DuplicateDesignator("U1".into())),
        ],
        "{:?}",
        list.rows
    );
    assert!(list.rows.iter().all(|r| !r.on));
    assert!(list.sheet_edits().is_empty());
}

#[test]
fn an_unannotated_board_designator_is_not_carried_back() {
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).reference = "R?".into();
    let list = diff(&board, &parts());
    assert_eq!(list.rows[0].blocked, Some(BackBlock::BoardNotAnnotated));
}

#[test]
fn a_value_on_a_sheet_placed_twice_is_read_only() {
    let mut parts = parts();
    parts[1].shared_sheet = true;
    let mut board = in_step();
    fp_mut(&mut board, 0xa2).value = "1k".into();
    fp_mut(&mut board, 0xa2).reference = "R7".into();
    let list = diff(&board, &parts);
    let value = list.row(&BackKey::Value(u(0xa2))).unwrap();
    assert_eq!(value.blocked, Some(BackBlock::SharedSheet));
    let designator = list.row(&BackKey::Designator(u(0xa2))).unwrap();
    assert!(designator.applies(), "a designator is per placement");
}

#[test]
fn another_footprint_on_the_board_is_listed_read_only() {
    let mut board = in_step();
    *fp_mut(&mut board, 0xa1) = board_fp(
        0xa1,
        "R1",
        "10k",
        &[0x101],
        &resolved("R0805", 0x805, "1.0.0", &["1", "2"]),
        (10.0, 10.0),
    );
    let list = diff(&board, &parts());
    let row = list.row(&BackKey::Footprint(u(0xa1))).unwrap();
    assert_eq!(row.blocked, Some(BackBlock::FootprintFromBoard));
    assert!(!row.on);
    assert!(list.sheet_edits().is_empty());
}

#[test]
fn unlinked_gone_and_unread_parts_are_notes_not_rows() {
    let mut board = in_step();
    board
        .footprints
        .push(board_fp(0xa9, "H1", "", &[], &r0603(), (40.0, 10.0)));
    board
        .footprints
        .push(board_fp(0xaa, "R8", "", &[0x999], &r0603(), (40.0, 20.0)));
    let mut parts = parts();
    parts[2].eco.read = false;
    fp_mut(&mut board, 0xa3).reference = "U7".into();
    let list = diff(&board, &parts);
    assert!(list.rows.is_empty(), "{:?}", list.rows);
    assert_eq!(
        list.notes,
        vec![
            BackNote::PartNotRead {
                designator: "U1".into()
            },
            BackNote::Unlinked {
                reference: "H1".into()
            },
            BackNote::PartGone {
                reference: "R8".into()
            },
        ]
    );
}

#[test]
fn a_designator_the_rows_would_share_gets_a_note() {
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).reference = "R2".into();
    let list = diff(&board, &parts());
    assert!(list.notes.contains(&BackNote::SharedDesignator {
        designator: "R2".into(),
        count: 2
    }));
}

#[test]
fn ticks_follow_the_user_and_survive_a_refresh() {
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).reference = "R9".into();
    fp_mut(&mut board, 0xa2).value = "1k".into();
    let mut list = diff(&board, &parts());
    list.set_on(&BackKey::Designator(u(0xa1)), false);
    assert_eq!(list.group_ticks(BackGroup::Designators).on, 0);
    let mut again = diff(&board, &parts());
    again.keep_choices_from(&list);
    assert!(!again.row(&BackKey::Designator(u(0xa1))).unwrap().on);
    assert!(again.row(&BackKey::Value(u(0xa2))).unwrap().on);
    again.set_group_on(BackGroup::Designators, true);
    assert!(again.group_ticks(BackGroup::Designators).all());
}

fn symbol(uuid: u128, reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: u(uuid),
        reference: reference.into(),
        value: value.into(),
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

fn sheet() -> SchematicSheet {
    SchematicSheet {
        symbols: vec![
            symbol(0x201, "U1", "LM"),
            symbol(0x202, "U1", "LM"),
            symbol(0x101, "R1", "10k"),
        ],
        ..Default::default()
    }
}

fn run(sheet: SchematicSheet, commands: Vec<crate::Command>) -> SchematicSheet {
    let mut engine = Engine::new(sheet).unwrap();
    engine.execute_batch(commands).unwrap();
    engine.document().clone()
}

#[test]
fn sheet_commands_rename_and_revalue_every_unit_of_the_part() {
    let edit = SheetEdit {
        placement: root(),
        units: vec![u(0x201), u(0x202)],
        designator: Some("U5".into()),
        value: Some("LM358".into()),
    };
    let edited = run(sheet(), sheet_commands(&sheet(), &[edit], true));
    for unit in &edited.symbols[..2] {
        assert_eq!(
            (unit.reference.as_str(), unit.value.as_str()),
            ("U5", "LM358")
        );
    }
    assert_eq!(
        edited.symbols[2].reference, "R1",
        "other parts are left alone"
    );
}

#[test]
fn on_a_sheet_placed_twice_the_designator_goes_to_that_placement_only() {
    let placement = InstancePath::root().child(Uuid::from_u128(0x5e));
    let edit = SheetEdit {
        placement: placement.clone(),
        units: vec![u(0x101)],
        designator: Some("R31".into()),
        value: None,
    };
    let edited = run(sheet(), sheet_commands(&sheet(), &[edit], false));
    let r = &edited.symbols[2];
    assert_eq!(r.reference, "R1", "the file's reference stays");
    assert_eq!(r.designator_in(&placement), "R31");
}

/// #1312 review: the note on a designator two parts would share follows the
/// ticks: R1 and R2 swapped on the board share nothing with both rows
/// ticked, and share R2 with only one.
#[test]
fn the_shared_designator_note_follows_the_ticks() {
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).reference = "R2".into();
    fp_mut(&mut board, 0xa2).reference = "R1".into();
    let mut list = diff(&board, &parts());
    let shared = |list: &BackChangeList| {
        list.notes
            .iter()
            .filter(|n| matches!(n, BackNote::SharedDesignator { .. }))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert!(shared(&list).is_empty(), "a clean swap shares nothing");
    list.set_on(&BackKey::Designator(u(0xa2)), false);
    assert_eq!(
        shared(&list),
        vec![BackNote::SharedDesignator {
            designator: "R2".into(),
            count: 2
        }]
    );
    list.set_group_on(BackGroup::Designators, true);
    assert!(shared(&list).is_empty());
}

#[test]
fn a_value_of_an_unannotated_part_is_not_carried() {
    let mut parts = parts();
    parts[1] = back(part("R?", &[0x102], "4k7", Ok(r0603())));
    let mut board = in_step();
    fp_mut(&mut board, 0xa2).value = "1k".into();
    let list = diff(&board, &parts);
    let value = list.row(&BackKey::Value(u(0xa2))).unwrap();
    assert_eq!(value.blocked, Some(BackBlock::NeedsAnnotation));
}

#[test]
fn a_second_footprint_linked_to_a_part_is_a_note_not_a_merge() {
    let mut board = in_step();
    board
        .footprints
        .push(board_fp(0xa9, "R9", "1M", &[0x101], &r0603(), (40.0, 10.0)));
    let list = diff(&board, &parts());
    assert!(list.rows.is_empty(), "{:?}", list.rows);
    assert!(list.notes.contains(&BackNote::ExtraLink {
        reference: "R9".into(),
        designator: "R1".into()
    }));
}

#[test]
fn surrounding_spaces_on_the_board_designator_are_not_a_change() {
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).reference = " R1 ".into();
    fp_mut(&mut board, 0xa2).reference = " R7".into();
    let list = diff(&board, &parts());
    assert_eq!(list.rows.len(), 1);
    assert!(matches!(
        &list.rows[0].change,
        BackChange::Designator { to, .. } if to == "R7"
    ));
}

#[test]
fn values_shared_with_another_project_are_blocked_on_request() {
    let mut board = in_step();
    fp_mut(&mut board, 0xa1).value = "1k".into();
    fp_mut(&mut board, 0xa1).reference = "R8".into();
    let mut list = diff(&board, &parts());
    list.block_shared_values(|_, units| units == [u(0x101)]);
    let value = list.row(&BackKey::Value(u(0xa1))).unwrap();
    assert_eq!(value.blocked, Some(BackBlock::SharedSheet));
    assert!(!value.on);
    assert!(list.row(&BackKey::Designator(u(0xa1))).unwrap().applies());
}
