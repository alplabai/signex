//! #2462 — Compare Board with Schematic: the kind of every Update PCB and
//! Back Annotate row, pairing by footprint and field, the default
//! directions, blocked sides, the shown-rows choice, the ticks Apply hands
//! on, the deletions the confirmation lists and choices kept on a refresh.

use signex_types::pcb::{PadNet, PcbBoard, SymbolLink};

use super::super::fixture::*;
use super::*;
use crate::pcb::eco::{
    BackChange, BackInput, BackPart, BackRow, ChangeReason, EcoChange, EcoGroup, EcoRow,
    diff_schematic,
};

const REVISION: u64 = 3;

fn eco_row(key: EcoKey, group: EcoGroup, change: EcoChange) -> EcoRow {
    EcoRow::new(key, group, change, true)
}

fn eco_list(rows: Vec<EcoRow>) -> EcoChangeList {
    EcoChangeList {
        revision: REVISION,
        rows,
        notes: Vec::new(),
    }
}

fn back_row(key: BackKey, group: BackGroup, change: BackChange) -> BackRow {
    BackRow {
        key,
        group,
        change,
        on: true,
        blocked: None,
    }
}

fn back_list(rows: Vec<BackRow>) -> BackChangeList {
    BackChangeList {
        revision: REVISION,
        rows,
        notes: Vec::new(),
        designators: Vec::new(),
    }
}

fn net(number: u32, name: &str) -> Option<PadNet> {
    Some(PadNet {
        number,
        name: name.into(),
    })
}

fn add_c6() -> EcoRow {
    let footprint = board_fp(0xc6, "C6", "100nF", &[0x301], &r0603(), (0.0, 0.0));
    eco_row(
        EcoKey::AddFootprint {
            sheet: root(),
            part: u(0x301),
        },
        EcoGroup::AddFootprints,
        EcoChange::AddFootprint {
            footprint: Box::new(footprint),
        },
    )
}

fn remove_tp3() -> EcoRow {
    eco_row(
        EcoKey::RemoveFootprint(u(0xa4)),
        EcoGroup::RemoveFootprints,
        EcoChange::RemoveFootprint {
            footprint: u(0xa4),
            reference: "TP3".into(),
        },
    )
}

fn change_l1(reason: ChangeReason) -> EcoRow {
    let lib = r0603();
    eco_row(
        EcoKey::ChangeFootprint(u(0xa2)),
        EcoGroup::ChangeFootprints,
        EcoChange::ChangeFootprint {
            footprint: u(0xa2),
            reference: "L1".into(),
            from: "L_4x4".into(),
            to: lib.source.clone(),
            reason,
            with: Box::new(lib.footprint),
        },
    )
}

fn designator_r1() -> EcoRow {
    eco_row(
        EcoKey::SetDesignator(u(0xa1)),
        EcoGroup::Designators,
        EcoChange::SetDesignator {
            footprint: u(0xa1),
            from: "R9".into(),
            to: "R1".into(),
        },
    )
}

fn value_r4() -> EcoRow {
    eco_row(
        EcoKey::SetValue(u(0xa5)),
        EcoGroup::Values,
        EcoChange::SetValue {
            footprint: u(0xa5),
            reference: "R4".into(),
            from: "12k".into(),
            to: "10k".into(),
        },
    )
}

fn link_r3() -> EcoRow {
    eco_row(
        EcoKey::Link(u(0xa6)),
        EcoGroup::Links,
        EcoChange::Link {
            footprint: u(0xa6),
            reference: "R3".into(),
            link: SymbolLink::new(root(), [u(0x103)]),
        },
    )
}

fn net_rows() -> Vec<EcoRow> {
    vec![
        eco_row(
            EcoKey::AddNet("VBUS".into()),
            EcoGroup::Nets,
            EcoChange::AddNet {
                number: 4,
                name: "VBUS".into(),
            },
        ),
        eco_row(
            EcoKey::RemoveNet(5),
            EcoGroup::Nets,
            EcoChange::RemoveNet {
                number: 5,
                name: "OLD".into(),
            },
        ),
        eco_row(
            EcoKey::RenameNet(1),
            EcoGroup::Nets,
            EcoChange::RenameNet {
                number: 1,
                from: "VIN".into(),
                to: "VIN_12V".into(),
            },
        ),
        pad_row(0xa7, "C2", "1", net(1, "VIN"), net(4, "VBUS")),
        pad_row(0xa8, "R1", "2", None, net(6, "VOUT_3V3")),
        pad_row(0xa9, "U1", "3", net(2, "EN"), None),
    ]
}

fn pad_row(
    fp: u128,
    reference: &str,
    pad: &str,
    from: Option<PadNet>,
    to: Option<PadNet>,
) -> EcoRow {
    eco_row(
        EcoKey::PadNet {
            footprint: u(fp),
            pad: u(fp * 0x100),
        },
        EcoGroup::PadNets,
        EcoChange::PadNet {
            footprint: u(fp),
            pad: u(fp * 0x100),
            reference: reference.into(),
            pad_number: pad.into(),
            from,
            to,
            locked: false,
        },
    )
}

fn back_designator_r1() -> BackRow {
    back_row(
        BackKey::Designator(u(0xa1)),
        BackGroup::Designators,
        BackChange::Designator {
            footprint: u(0xa1),
            sheet: root(),
            units: vec![u(0x101)],
            from: "R1".into(),
            to: "R9".into(),
        },
    )
}

fn back_value_r4() -> BackRow {
    back_row(
        BackKey::Value(u(0xa5)),
        BackGroup::Values,
        BackChange::Value {
            footprint: u(0xa5),
            sheet: root(),
            units: vec![u(0x104)],
            reference: "R4".into(),
            from: "10k".into(),
            to: "12k".into(),
        },
    )
}

fn back_footprint_l1() -> BackRow {
    let mut row = back_row(
        BackKey::Footprint(u(0xa2)),
        BackGroup::Footprints,
        BackChange::Footprint {
            footprint: u(0xa2),
            reference: "L1".into(),
            board: "L_4x4".into(),
            schematic: "L_5x5".into(),
        },
    );
    row.on = false;
    row.blocked = Some(BackBlock::FootprintFromBoard);
    row
}

/// Every kind of Update PCB row, and the Back Annotate rows that pair.
fn sample() -> CompareReport {
    let mut eco = vec![
        add_c6(),
        remove_tp3(),
        change_l1(ChangeReason::Footprint),
        designator_r1(),
        value_r4(),
        link_r3(),
    ];
    eco.extend(net_rows());
    let back = vec![back_designator_r1(), back_value_r4(), back_footprint_l1()];
    compare(&eco_list(eco), &back_list(back))
}

fn by_pcb<'a>(report: &'a CompareReport, key: &EcoKey) -> &'a CompareRow {
    report
        .rows
        .iter()
        .find(|r| r.pcb.as_ref() == Some(key))
        .unwrap_or_else(|| panic!("no row for {key:?}"))
}

fn id(report: &CompareReport, key: &EcoKey) -> CompareRowId {
    by_pcb(report, key).id()
}

#[test]
fn every_update_pcb_row_gets_its_kind_and_action() {
    let report = sample();
    let kinds: Vec<(Option<EcoKey>, CompareKind, Option<CompareAction>)> = report
        .rows
        .iter()
        .map(|r| (r.pcb.clone(), r.kind, r.pcb_action))
        .collect();
    use CompareAction as A;
    use CompareKind as K;
    let pad = |fp: u128| EcoKey::PadNet {
        footprint: u(fp),
        pad: u(fp * 0x100),
    };
    assert_eq!(
        kinds,
        vec![
            (Some(add_c6().key), K::MissingOnBoard, Some(A::AddToBoard)),
            (
                Some(link_r3().key),
                K::MissingOnBoard,
                Some(A::LinkFootprint)
            ),
            (
                Some(remove_tp3().key),
                K::ExtraOnBoard,
                Some(A::RemoveFromBoard)
            ),
            (
                Some(EcoKey::AddNet("VBUS".into())),
                K::NetDifference,
                Some(A::AddNet)
            ),
            (
                Some(EcoKey::RemoveNet(5)),
                K::NetDifference,
                Some(A::RemoveNet)
            ),
            (
                Some(EcoKey::RenameNet(1)),
                K::NetDifference,
                Some(A::RenameNet)
            ),
            (Some(pad(0xa7)), K::NetDifference, Some(A::MovePin)),
            (Some(pad(0xa8)), K::NetDifference, Some(A::ConnectPin)),
            (Some(pad(0xa9)), K::NetDifference, Some(A::DisconnectPin)),
            (
                Some(EcoKey::ChangeFootprint(u(0xa2))),
                K::FootprintMismatch,
                Some(A::SwapFootprint)
            ),
            (
                Some(designator_r1().key),
                K::ValueChange,
                Some(A::SetDesignator)
            ),
            (Some(value_r4().key), K::ValueChange, Some(A::SetValue)),
        ]
    );
}

#[test]
fn the_cells_name_each_side_and_what_is_missing() {
    let report = sample();
    let add = by_pcb(&report, &add_c6().key);
    assert_eq!(add.object, CompareObject::Designator("C6".into()));
    assert_eq!(
        add.schematic,
        Some(CompareValue::Part {
            value: "100nF".into(),
            footprint: "R0603".into()
        })
    );
    assert_eq!(add.board, None, "not on board");
    assert_eq!(
        add.jump.part,
        Some(PartOccurrence {
            sheet: root(),
            symbol: u(0x301)
        })
    );
    let extra = by_pcb(&report, &remove_tp3().key);
    assert_eq!(extra.schematic, None, "not in schematic");
    assert_eq!(extra.jump.footprint, Some(u(0xa4)));
    let rename = by_pcb(&report, &EcoKey::RenameNet(1));
    assert_eq!(rename.object, CompareObject::Net("VIN_12V".into()));
    assert_eq!(
        (rename.schematic.clone(), rename.board.clone()),
        (
            Some(CompareValue::Text("VIN_12V".into())),
            Some(CompareValue::Text("VIN".into()))
        )
    );
    let connect = by_pcb(
        &report,
        &EcoKey::PadNet {
            footprint: u(0xa8),
            pad: u(0xa800),
        },
    );
    assert_eq!(
        connect.object,
        CompareObject::Pin {
            reference: "R1".into(),
            pad: "2".into()
        }
    );
    assert_eq!(connect.board, Some(CompareValue::NoNet));
    assert_eq!(connect.jump.schematic_net.as_deref(), Some("VOUT_3V3"));
    assert_eq!(connect.jump.board_net, None);
}

#[test]
fn rows_of_one_footprint_and_field_pair_into_one_row() {
    let report = sample();
    let designator = by_pcb(&report, &designator_r1().key);
    assert_eq!(designator.back, Some(BackKey::Designator(u(0xa1))));
    assert_eq!(
        designator.schematic_action,
        Some(CompareAction::TakeBoardDesignator)
    );
    assert_eq!(
        designator.jump.part,
        Some(PartOccurrence {
            sheet: root(),
            symbol: u(0x101)
        }),
        "the part comes from the Back Annotate row"
    );
    let value = by_pcb(&report, &value_r4().key);
    assert_eq!(value.back, Some(BackKey::Value(u(0xa5))));
    assert_eq!(value.schematic_action, Some(CompareAction::TakeBoardValue));
    let footprint = by_pcb(&report, &EcoKey::ChangeFootprint(u(0xa2)));
    assert_eq!(footprint.back, Some(BackKey::Footprint(u(0xa2))));
    assert_eq!(report.rows.len(), 12, "no back row is listed twice");
}

#[test]
fn a_back_row_with_no_update_pcb_row_stands_alone() {
    let report = compare(&eco_list(Vec::new()), &back_list(vec![back_value_r4()]));
    assert_eq!(report.rows.len(), 1);
    let row = &report.rows[0];
    assert_eq!(row.kind, CompareKind::ValueChange);
    assert_eq!(row.pcb, None);
    assert_eq!(
        (row.schematic.clone(), row.board.clone()),
        (
            Some(CompareValue::Text("10k".into())),
            Some(CompareValue::Text("12k".into()))
        )
    );
    assert_eq!(
        row.unavailable(Direction::UpdatePcb),
        Some(Unavailable::NoBoardChange)
    );
    assert_eq!(row.direction, Direction::UpdateSchematic, "the board wins");
}

#[test]
fn rel_01_skips_board_originated_rows_and_the_schematic_wins_the_rest() {
    let report = sample();
    let direction = |key: &EcoKey| by_pcb(&report, key).direction;
    assert_eq!(direction(&remove_tp3().key), Direction::Skip);
    assert_eq!(
        direction(&EcoKey::ChangeFootprint(u(0xa2))),
        Direction::Skip
    );
    assert_eq!(direction(&add_c6().key), Direction::UpdatePcb);
    assert_eq!(direction(&link_r3().key), Direction::UpdatePcb);
    assert_eq!(direction(&EcoKey::RenameNet(1)), Direction::UpdatePcb);
    assert_eq!(direction(&designator_r1().key), Direction::UpdatePcb);
    assert_eq!(direction(&value_r4().key), Direction::UpdatePcb);
}

#[test]
fn a_row_update_pcb_starts_unticked_is_skipped() {
    let mut locked = value_r4();
    locked.default_on = false;
    locked.on = false;
    let mut other_version = change_l1(ChangeReason::OtherVersion);
    other_version.default_on = false;
    other_version.on = false;
    let report = compare(
        &eco_list(vec![locked, other_version]),
        &back_list(vec![back_value_r4()]),
    );
    assert!(report.rows.iter().all(|r| r.direction == Direction::Skip));
}

#[test]
fn a_blocked_side_keeps_its_reason_and_is_never_chosen() {
    let blocked = add_c6().blocked(EcoBlock::NeedsAnnotation);
    let mut report = compare(&eco_list(vec![blocked]), &back_list(Vec::new()));
    let row_id = report.rows[0].id();
    let row = &report.rows[0];
    assert_eq!(row.direction, Direction::Skip);
    assert_eq!(
        row.unavailable(Direction::UpdatePcb),
        Some(Unavailable::Board(EcoBlock::NeedsAnnotation))
    );
    assert_eq!(
        row.unavailable(Direction::UpdateSchematic),
        Some(Unavailable::SchematicLater)
    );
    assert!(!report.set_direction(&row_id, Direction::UpdatePcb));
    assert_eq!(report.rows[0].direction, Direction::Skip);
    assert!(!report.eco().rows[0].on);

    // The footprint mismatch's board side is not wired yet (a follow-up issue).
    let report = sample();
    let footprint = by_pcb(&report, &EcoKey::ChangeFootprint(u(0xa2)));
    assert_eq!(
        footprint.unavailable(Direction::UpdateSchematic),
        Some(Unavailable::Schematic(BackBlock::FootprintFromBoard))
    );
}

#[test]
fn a_value_update_pcb_cannot_make_goes_to_the_schematic() {
    let blocked = value_r4().blocked(EcoBlock::PartNotRead);
    let report = compare(&eco_list(vec![blocked]), &back_list(vec![back_value_r4()]));
    assert_eq!(report.rows[0].direction, Direction::UpdateSchematic);
}

#[test]
fn set_shown_leaves_rows_that_do_not_allow_the_direction() {
    let mut report = sample();
    report.set_shown(None, Direction::UpdateSchematic);
    let direction = |report: &CompareReport, key: &EcoKey| by_pcb(report, key).direction;
    assert_eq!(
        direction(&report, &designator_r1().key),
        Direction::UpdateSchematic
    );
    assert_eq!(
        direction(&report, &value_r4().key),
        Direction::UpdateSchematic
    );
    // No schematic side: kept as they were.
    assert_eq!(direction(&report, &add_c6().key), Direction::UpdatePcb);
    assert_eq!(direction(&report, &remove_tp3().key), Direction::Skip);
    assert_eq!(
        direction(&report, &EcoKey::ChangeFootprint(u(0xa2))),
        Direction::Skip,
        "its schematic side is blocked"
    );

    report.set_shown(Some(CompareKind::NetDifference), Direction::Skip);
    assert_eq!(direction(&report, &EcoKey::RenameNet(1)), Direction::Skip);
    assert_eq!(direction(&report, &add_c6().key), Direction::UpdatePcb);
}

/// The net rows are computed for the board the footprint rows make (their
/// designators and values included), so only those rows ask for a refresh.
#[test]
fn a_footprint_row_direction_asks_for_a_net_refresh_and_a_net_row_does_not() {
    let mut report = sample();
    let extra = id(&report, &remove_tp3().key);
    assert!(report.set_direction(&extra, Direction::UpdatePcb));
    let value = id(&report, &value_r4().key);
    assert!(report.set_direction(&value, Direction::UpdateSchematic));
    let rename = id(&report, &EcoKey::RenameNet(1));
    assert!(!report.set_direction(&rename, Direction::Skip));
    assert!(
        !report.set_direction(&rename, Direction::Skip),
        "the same direction again changes nothing"
    );
}

#[test]
fn ticks_turn_on_only_the_chosen_rows() {
    let mut report = sample();
    let value = id(&report, &value_r4().key);
    report.set_direction(&value, Direction::UpdateSchematic);
    let (eco, back) = report.ticks();
    let on: Vec<EcoKey> = eco
        .rows
        .iter()
        .filter(|r| r.on)
        .map(|r| r.key.clone())
        .collect();
    assert!(on.contains(&add_c6().key));
    assert!(on.contains(&designator_r1().key));
    assert!(!on.contains(&value_r4().key), "the board wins the value");
    assert!(!on.contains(&remove_tp3().key), "skipped");
    assert!(!on.contains(&EcoKey::ChangeFootprint(u(0xa2))), "skipped");
    let back_on: Vec<BackKey> = back.rows.iter().filter(|r| r.on).map(|r| r.key).collect();
    assert_eq!(back_on, vec![BackKey::Value(u(0xa5))]);
    assert_eq!((eco.revision, back.revision), (REVISION, REVISION));
    // The report's own lists follow the directions too.
    assert_eq!(report.eco().ticked(), eco.ticked());
}

#[test]
fn counts_say_where_the_rows_go() {
    let mut report = sample();
    let counts = report.counts();
    assert_eq!(counts.total, 12);
    assert_eq!(counts.skipped, 2, "the extra footprint and the swap");
    assert_eq!(counts.to_schematic, 0);
    assert_eq!(counts.to_pcb, 10);
    assert_eq!(counts.applied(), 10);
    let value = id(&report, &value_r4().key);
    report.set_direction(&value, Direction::UpdateSchematic);
    let counts = report.counts();
    assert_eq!((counts.to_pcb, counts.to_schematic), (9, 1));
    assert_eq!(report.kind_count(Some(CompareKind::NetDifference)), 6);
    assert_eq!(report.kind_count(None), 12);
}

#[test]
fn a_chosen_row_held_back_by_a_skipped_row_it_needs_counts_as_skipped() {
    let rename = eco_row(
        EcoKey::RenameNet(1),
        EcoGroup::Nets,
        EcoChange::RenameNet {
            number: 1,
            from: "VIN".into(),
            to: "VIN_12V".into(),
        },
    );
    let pad = pad_row(0xa7, "C2", "1", net(1, "VIN"), net(1, "VIN_12V"))
        .needing(vec![EcoKey::RenameNet(1)]);
    let mut report = compare(&eco_list(vec![rename, pad]), &back_list(Vec::new()));
    let rename_id = id(&report, &EcoKey::RenameNet(1));
    report.set_direction(&rename_id, Direction::Skip);
    let pad = by_pcb(
        &report,
        &EcoKey::PadNet {
            footprint: u(0xa7),
            pad: u(0xa700),
        },
    );
    assert_eq!(pad.direction, Direction::UpdatePcb);
    assert!(report.waits(pad));
    assert_eq!(report.counts().skipped, 2);
    assert_eq!(report.counts().applied(), 0);
}

#[test]
fn board_deletions_list_the_chosen_removals() {
    let mut report = sample();
    let deletions: Vec<CompareObject> = report
        .board_deletions()
        .into_iter()
        .map(|r| r.object.clone())
        .collect();
    assert_eq!(deletions, vec![CompareObject::Net("OLD".into())]);
    let extra = id(&report, &remove_tp3().key);
    report.set_direction(&extra, Direction::UpdatePcb);
    let deletions: Vec<CompareObject> = report
        .board_deletions()
        .into_iter()
        .map(|r| r.object.clone())
        .collect();
    assert_eq!(
        deletions,
        vec![
            CompareObject::Designator("TP3".into()),
            CompareObject::Net("OLD".into())
        ]
    );
    let old = id(&report, &EcoKey::RemoveNet(5));
    report.set_direction(&old, Direction::Skip);
    assert_eq!(report.board_deletions().len(), 1);
}

#[test]
fn keep_choices_from_carries_a_direction_for_the_same_change() {
    let mut previous = sample();
    let value = id(&previous, &value_r4().key);
    previous.set_direction(&value, Direction::UpdateSchematic);
    let extra = id(&previous, &remove_tp3().key);
    previous.set_direction(&extra, Direction::UpdatePcb);

    let mut again = sample();
    again.keep_choices_from(&previous);
    assert_eq!(
        by_pcb(&again, &value_r4().key).direction,
        Direction::UpdateSchematic
    );
    assert_eq!(
        by_pcb(&again, &remove_tp3().key).direction,
        Direction::UpdatePcb
    );
    assert_eq!(again.ticks(), previous.ticks());

    // The board's value moved on: a new choice, so the default again.
    let mut moved = value_r4();
    if let EcoChange::SetValue { from, .. } = &mut moved.change {
        *from = "15k".into();
    }
    let mut changed = compare(&eco_list(vec![moved]), &back_list(vec![back_value_r4()]));
    assert_eq!(changed.rows[0].id(), value, "the same row");
    changed.keep_choices_from(&previous);
    assert_eq!(changed.rows[0].direction, Direction::UpdatePcb);
}

#[test]
fn notes_of_both_lists_are_kept() {
    let mut eco = eco_list(vec![add_c6()]);
    eco.notes.push(EcoNote::KeptUnlinked {
        footprint: u(0xf1),
        reference: "H1".into(),
    });
    let mut back = back_list(Vec::new());
    back.notes.push(BackNote::Unlinked {
        reference: "H1".into(),
    });
    let report = compare(&eco, &back);
    assert_eq!(report.eco_notes().len(), 1);
    assert_eq!(report.back_notes().len(), 1);
    let text = report.report_text();
    assert!(
        text.contains("H1 is not linked to a part and is kept"),
        "{text}"
    );
    assert!(text.contains("H1 is not linked to a part\n"), "{text}");
}

#[test]
fn the_report_text_lists_each_kind_and_choice() {
    let report = sample();
    let text = report.report_text();
    assert!(text.starts_with("Compare Board with Schematic\nBoard revision: 3\n"));
    assert!(text.contains("Differences: 12 (10 to PCB, 0 to schematic, 2 skipped)"));
    assert!(
        text.contains("\nExtra on board\n  TP3: schematic (not in schematic), board TP3 -> Skip\n")
    );
    assert!(
        text.contains(
            "  R1 pin 2: schematic VOUT_3V3, board (no net) -> Update PCB: Connect pin\n"
        )
    );
    assert!(
        text.contains(
            "Update schematic blocked: taking a footprint from the board is not built yet"
        )
    );
    assert_eq!(text, report.report_text(), "the same bytes every time");
}

/// R1 placed with another value, TP3 linked to a part that is gone, R2 not
/// placed: the lists `diff_board` and `diff_schematic` make, compared.
fn read_design() -> (Design, CompareReport) {
    let parts = vec![
        part("R1", &[0x101], "10k", Ok(r0603())),
        part("R2", &[0x102], "4k7", Ok(r0603())),
    ];
    let board: PcbBoard = board(vec![
        board_fp(0xa1, "R1", "22k", &[0x101], &r0603(), (10.0, 10.0)),
        board_fp(0xa4, "TP3", "TestPoint", &[0x999], &r0603(), (20.0, 10.0)),
    ]);
    let design = Design::new(board, parts, netlist(&[]));
    let eco = design.diff();
    let back_parts: Vec<BackPart> = design
        .parts
        .iter()
        .cloned()
        .map(|eco| BackPart {
            eco,
            shared_sheet: false,
        })
        .collect();
    let back = diff_schematic(&BackInput {
        board: &design.board,
        revision: design.revision,
        parts: &back_parts,
        unreadable: &[],
    });
    let mut report = compare(&eco, &back);
    report.refresh_nets(&design.input());
    (design, report)
}

#[test]
fn a_read_design_compares_both_ways() {
    let (_, report) = read_design();
    let value = by_pcb(&report, &EcoKey::SetValue(u(0xa1)));
    assert_eq!(value.back, Some(BackKey::Value(u(0xa1))));
    assert_eq!(value.direction, Direction::UpdatePcb);
    let extra = by_pcb(&report, &EcoKey::RemoveFootprint(u(0xa4)));
    assert_eq!(extra.direction, Direction::Skip);
    assert_eq!(
        extra.board,
        Some(CompareValue::Part {
            value: "TestPoint".into(),
            footprint: "R0603".into()
        }),
        "the board side is described from the board"
    );
    assert!(
        !report
            .eco()
            .row(&EcoKey::RemoveFootprint(u(0xa4)))
            .is_some_and(|r| r.on),
        "the skipped removal is not ticked"
    );
    assert!(
        report
            .rows
            .iter()
            .any(|r| r.kind == CompareKind::MissingOnBoard && r.direction == Direction::UpdatePcb),
        "R2 is added"
    );
}

#[test]
fn a_refresh_keeps_the_directions_chosen() {
    let (design, mut report) = read_design();
    let extra = id(&report, &EcoKey::RemoveFootprint(u(0xa4)));
    assert!(report.set_direction(&extra, Direction::UpdatePcb));
    let value = id(&report, &EcoKey::SetValue(u(0xa1)));
    report.set_direction(&value, Direction::UpdateSchematic);
    let before = report.ticks();
    report.refresh_nets(&design.input());
    assert_eq!(
        by_pcb(&report, &EcoKey::RemoveFootprint(u(0xa4))).direction,
        Direction::UpdatePcb
    );
    assert_eq!(
        by_pcb(&report, &EcoKey::SetValue(u(0xa1))).direction,
        Direction::UpdateSchematic
    );
    assert_eq!(report.ticks().1, before.1);
    assert!(
        report
            .eco()
            .row(&EcoKey::RemoveFootprint(u(0xa4)))
            .is_some_and(|r| r.on)
    );
}

#[test]
fn a_board_in_step_with_the_schematic_is_in_sync() {
    let report = compare(&eco_list(Vec::new()), &back_list(Vec::new()));
    assert!(report.in_sync());
    assert_eq!(report.counts(), CompareCounts::default());
    assert!(report.board_deletions().is_empty());
}
