//! #1312 — the change list for "Update PCB from schematic": footprint rows,
//! lowering, Validate and the report.

use signex_types::part::FootprintResolveError;
use signex_types::pcb::PcbBoard;
use signex_types::schematic::InstancePath;

use super::fixture::*;
use super::*;
use crate::pcb::BoardEngine;

/// R1, R2 (0603) and U1 (SOT-23, two units), wired VCC / SIG / GND.
fn design_parts() -> Vec<EcoPart> {
    vec![
        part("R1", &[0x101], "10k", Ok(r0603())),
        part("R2", &[0x102], "4k7", Ok(r0603())),
        part("U1", &[0x201, 0x202], "LM", Ok(sot23())),
    ]
}

fn design_netlist() -> signex_types::net::Netlist {
    netlist(&[
        ("VCC", &[(0x101, "R1", "1"), (0x201, "U1", "3")]),
        ("SIG", &[(0x102, "R2", "1"), (0x101, "R1", "2")]),
        ("GND", &[(0x102, "R2", "2"), (0x202, "U1", "2")]),
    ])
}

/// The board with every part placed and linked, nets not yet synced.
fn placed_board() -> PcbBoard {
    board(vec![
        board_fp(0xa1, "R1", "10k", &[0x101], &r0603(), (10.0, 10.0)),
        board_fp(0xa2, "R2", "4k7", &[0x102], &r0603(), (20.0, 10.0)),
        board_fp(0xa3, "U1", "LM", &[0x201, 0x202], &sot23(), (30.0, 10.0)),
    ])
}

/// Run every ticked row on `design`'s board, as Execute would.
fn execute(design: &mut Design, list: &EcoChangeList) {
    let mut engine = BoardEngine::new(design.board.clone(), design.revision);
    let commands = lower(list, engine.board());
    engine
        .execute_labelled(commands, "Update PCB", list.revision)
        .unwrap();
    design.revision = engine.revision();
    design.board = engine.into_board();
}

/// The design with its board brought fully in line: nets synced too.
fn synced() -> Design {
    let mut design = Design::new(placed_board(), design_parts(), design_netlist());
    let list = design.diff();
    execute(&mut design, &list);
    design
}

fn snapshot(board: &PcbBoard) -> String {
    serde_json::to_string(board).unwrap()
}

fn keys(list: &EcoChangeList) -> Vec<EcoKey> {
    list.rows.iter().map(|r| r.key.clone()).collect()
}

fn add_key(units: &[u128]) -> EcoKey {
    EcoKey::AddFootprint {
        sheet: root(),
        part: units.iter().copied().map(u).min().unwrap(),
    }
}

#[test]
fn a_board_in_line_with_the_design_has_nothing_to_change() {
    let design = synced();
    let list = design.diff();
    assert!(list.rows.is_empty(), "{}", report_text(&list));
}

#[test]
fn a_new_part_is_added_staged_linked_and_named() {
    let mut design = synced();
    design.parts.push(part("C1", &[0x301], "100n", Ok(r0603())));
    let list = design.diff();
    let row = list.row(&add_key(&[0x301])).expect("an add row");
    assert!(row.on && row.default_on && row.blocked.is_none());
    let EcoChange::AddFootprint { footprint } = &row.change else {
        panic!("{row:?}");
    };
    assert_eq!(
        (footprint.reference.as_str(), footprint.value.as_str()),
        ("C1", "100n")
    );
    assert_eq!(
        footprint.schematic_link,
        Some(signex_types::pcb::SymbolLink::new(root(), [u(0x301)]))
    );
    assert_eq!(footprint.library_ref, Some(r0603().source));
    assert!(footprint.position.x > 50.0, "staged right of the outline");
    let texts: Vec<&str> = footprint
        .properties
        .iter()
        .map(|p| p.value.as_str())
        .collect();
    assert_eq!(texts, ["C1", "100n"]);

    // The same list twice: the same uuids and position, so ticks survive.
    let again = design.diff();
    assert_eq!(list, again);

    execute(&mut design, &list);
    assert!(design.board.footprints.iter().any(|f| f.reference == "C1"));
    assert!(design.diff().rows.is_empty(), "applied, the list is empty");
}

#[test]
fn a_part_gone_from_the_schematic_is_removed_but_an_unlinked_footprint_is_kept() {
    let mut design = synced();
    let mut hole = board_fp(0xb1, "H1", "", &[], &r0603(), (2.0, 2.0));
    hole.footprint_id = "MountingHole".into();
    design.board.footprints.push(hole);
    design.parts.retain(|p| p.part.designator != "R2");
    let list = design.diff();
    let remove = list
        .row(&EcoKey::RemoveFootprint(u(0xa2)))
        .expect("remove R2");
    assert!(remove.on);
    assert!(list.row(&EcoKey::RemoveFootprint(u(0xb1))).is_none());
    assert!(list.notes.iter().any(|n| matches!(
        n,
        EcoNote::KeptUnlinked { reference, .. } if reference == "H1"
    )));
}

#[test]
fn a_part_marked_not_on_the_board_is_removed_and_a_dnp_part_is_placed() {
    let mut design = synced();
    design.parts[1].on_board = false;
    design.parts.push(part("C9", &[0x309], "DNP", Ok(r0603())));
    let list = design.diff();
    assert!(list.row(&EcoKey::RemoveFootprint(u(0xa2))).is_some());
    assert!(
        list.row(&add_key(&[0x309])).is_some(),
        "DNP parts are placed"
    );

    let mut off = part("C8", &[0x308], "", Ok(r0603()));
    off.on_board = false;
    design.parts.push(off);
    assert!(design.diff().row(&add_key(&[0x308])).is_none());
}

#[test]
fn design_eco_parts_reads_value_and_on_board_from_the_first_unit_and_ignores_dnp() {
    use signex_types::schematic::{SchematicSheet, Symbol};
    let mut sheet = SchematicSheet::default();
    for (n, reference, dnp, on_board) in [(0x1, "R1", true, true), (0x2, "R2", false, false)] {
        sheet.symbols.push(Symbol {
            uuid: u(n),
            lib_id: "Device:R".into(),
            reference: reference.into(),
            value: format!("v{n}"),
            dnp,
            on_board,
            ..Symbol::default()
        });
    }
    let parts = design_eco_parts(&[(root(), &sheet)], |_, _| Ok(r0603()));
    let summary: Vec<(&str, &str, bool)> = parts
        .iter()
        .map(|p| (p.part.designator.as_str(), p.value.as_str(), p.on_board))
        .collect();
    assert_eq!(summary, [("R1", "v1", true), ("R2", "v2", false)]);
}

#[test]
fn a_different_footprint_is_changed_and_a_newer_version_is_listed_unticked() {
    let mut design = synced();
    design.parts[0].footprint = Ok(resolved("R0805", 0x805, "1.0.0", &["1", "2"]));
    design.parts[1].footprint = Ok(resolved("R0603", 0x603, "1.1.0", &["1", "2"]));
    let list = design.diff();
    let change = list.row(&EcoKey::ChangeFootprint(u(0xa1))).unwrap();
    assert!(change.on);
    let newer = list.row(&EcoKey::ChangeFootprint(u(0xa2))).unwrap();
    assert!(matches!(
        newer.change,
        EcoChange::ChangeFootprint {
            reason: ChangeReason::NewerVersion,
            ..
        }
    ));
    assert!(!newer.on && !newer.default_on);

    // R1's replacement keeps its nets: pad numbers carry them.
    execute(&mut design, &list);
    let r1 = design
        .board
        .footprints
        .iter()
        .find(|f| f.reference == "R1")
        .unwrap();
    assert_eq!(r1.footprint_id, "R0805");
    assert!(r1.pads.iter().all(|p| p.net.is_some()));
    let rest = design.diff();
    assert_eq!(
        keys(&rest),
        [EcoKey::ChangeFootprint(u(0xa2))],
        "only the unticked row is left"
    );
}

#[test]
fn a_locked_footprints_remove_and_change_rows_start_unticked() {
    let mut design = synced();
    for fp in &mut design.board.footprints {
        fp.locked = true;
    }
    design.parts.retain(|p| p.part.designator != "R2");
    design.parts[0].footprint = Ok(resolved("R0805", 0x805, "1.0.0", &["1", "2"]));
    let list = design.diff();
    for key in [
        EcoKey::RemoveFootprint(u(0xa2)),
        EcoKey::ChangeFootprint(u(0xa1)),
    ] {
        let row = list.row(&key).unwrap();
        assert!(!row.on && !row.default_on, "{key:?}");
    }
}

#[test]
fn a_renamed_and_revalued_part_gives_one_text_command() {
    let mut design = synced();
    design.parts[0].part.designator = "R10".into();
    design.parts[0].value = "22k".into();
    let list = design.diff();
    assert!(list.row(&EcoKey::SetDesignator(u(0xa1))).is_some());
    assert!(list.row(&EcoKey::SetValue(u(0xa1))).is_some());
    let commands = lower(&list, &design.board);
    let texts: Vec<_> = commands
        .iter()
        .filter(|c| matches!(c, crate::pcb::BoardCommand::SetFootprintText { .. }))
        .collect();
    assert_eq!(texts.len(), 1);

    // Only the value ticked: the designator stays.
    let mut value_only = list.clone();
    value_only.set_on(&EcoKey::SetDesignator(u(0xa1)), false);
    execute(&mut design, &value_only);
    let r1 = &design.board.footprints[0];
    assert_eq!((r1.reference.as_str(), r1.value.as_str()), ("R1", "22k"));
}

#[test]
fn an_unlinked_footprint_with_the_parts_designator_is_linked_not_replaced() {
    let mut design = synced();
    design.board.footprints[1].schematic_link = None;
    let list = design.diff();
    assert_eq!(
        keys(&list),
        [EcoKey::Link(u(0xa2))],
        "{}",
        report_text(&list)
    );
    execute(&mut design, &list);
    assert!(design.board.footprints[1].schematic_link.is_some());
    assert!(design.diff().rows.is_empty());
}

#[test]
fn a_unit_added_to_a_part_updates_its_link() {
    let mut design = synced();
    design.parts[2].part.units.push(u(0x203));
    let list = design.diff();
    let row = list.row(&EcoKey::Link(u(0xa3))).unwrap();
    let EcoChange::Link { link, .. } = &row.change else {
        panic!()
    };
    assert_eq!(link.units, [u(0x201), u(0x202), u(0x203)]);
    assert!(list.row(&add_key(&[0x201, 0x202, 0x203])).is_none());
}

#[test]
fn duplicate_and_unannotated_designators_block_their_rows() {
    let mut design = synced();
    design.parts.push(part("C2", &[0x302], "1u", Ok(r0603())));
    design.parts.push(part("C2", &[0x303], "1u", Ok(r0603())));
    design.parts.push(part("C?", &[0x304], "1u", Ok(r0603())));
    let list = design.diff();
    for units in [[0x302], [0x303]] {
        assert!(matches!(
            list.row(&add_key(&units)).unwrap().blocked,
            Some(EcoBlock::DuplicateDesignator(ref d)) if d == "C2"
        ));
    }
    let unannotated = list.row(&add_key(&[0x304])).unwrap();
    assert_eq!(unannotated.blocked, Some(EcoBlock::NeedsAnnotation));
    assert!(!unannotated.on);
    // A blocked row stays unticked.
    let mut list = list;
    list.set_on(&add_key(&[0x304]), true);
    assert!(!list.row(&add_key(&[0x304])).unwrap().on);
}

#[test]
fn a_missing_footprint_blocks_its_row_and_the_rest_still_runs() {
    let mut design = synced();
    design.parts.push(part(
        "J1",
        &[0x401],
        "conn",
        Err(FootprintResolveError::NoFootprint),
    ));
    design.parts.push(part("C3", &[0x305], "1u", Ok(r0603())));
    let list = design.diff();
    assert_eq!(
        list.row(&add_key(&[0x401])).unwrap().blocked,
        Some(EcoBlock::MissingFootprint(
            FootprintResolveError::NoFootprint
        ))
    );
    execute(&mut design, &list);
    assert!(design.board.footprints.iter().any(|f| f.reference == "C3"));
    assert!(!design.board.footprints.iter().any(|f| f.reference == "J1"));
}

#[test]
fn nothing_linked_under_an_unreadable_sheet_is_removed() {
    let mut design = synced();
    let child = InstancePath::from_chain(vec![u(0x5e)]);
    let mut under = board_fp(0xc1, "R50", "1k", &[], &r0603(), (5.0, 5.0));
    under.schematic_link = Some(signex_types::pcb::SymbolLink::new(
        child.clone(),
        [u(0x501)],
    ));
    design.board.footprints.push(under);
    design.unreadable = vec![child.clone()];
    let list = design.diff();
    assert!(list.row(&EcoKey::RemoveFootprint(u(0xc1))).is_none());
    assert!(list.notes.contains(&EcoNote::UnreadableSheet(child)));

    design.unreadable.clear();
    assert!(
        design
            .diff()
            .row(&EcoKey::RemoveFootprint(u(0xc1)))
            .is_some()
    );
}

#[test]
fn a_locked_pad_the_new_footprint_lacks_is_reported_and_a_lost_net_blocks() {
    let mut design = synced();
    design.board.footprints[2].pads[2].net_locked = true;
    design.parts[2].footprint = Ok(resolved("SOT-23-2", 0x232, "1.0.0", &["1", "2"]));
    let list = design.diff();
    assert!(list.notes.iter().any(|n| matches!(
        n,
        EcoNote::PadLosesLock { reference, pad_number, .. } if reference == "U1" && pad_number == "3"
    )));
    assert!(
        list.row(&EcoKey::ChangeFootprint(u(0xa3)))
            .unwrap()
            .blocked
            .is_none()
    );

    // Two "1" pads, the second on a net, become one: the net would be lost.
    let mut design = synced();
    design.board.footprints[0].pads[1].number = "1".into();
    let list = design.diff();
    assert!(
        list.row(&EcoKey::ChangeFootprint(u(0xa1))).is_none(),
        "same footprint"
    );
    design.parts[0].footprint = Ok(resolved("R0805", 0x805, "1.0.0", &["1", "2"]));
    let list = design.diff();
    assert_eq!(
        list.row(&EcoKey::ChangeFootprint(u(0xa1))).unwrap().blocked,
        Some(EcoBlock::PadsCannotCarry { number: "1".into() })
    );
}

#[test]
fn lowering_then_validating_on_a_copy_matches_executing() {
    let mut design = synced();
    design.parts.push(part("C1", &[0x301], "100n", Ok(r0603())));
    design.parts.retain(|p| p.part.designator != "R2");
    design.parts[0].value = "22k".into();
    let list = design.diff();
    let engine = BoardEngine::new(design.board.clone(), design.revision);
    check(&list, &engine).unwrap();
    assert_eq!(
        snapshot(engine.board()),
        snapshot(&design.board),
        "Validate changes nothing"
    );

    let mut by_hand = design.board.clone();
    for command in lower(&list, &design.board) {
        command.apply(&mut by_hand).unwrap();
    }
    execute(&mut design, &list);
    assert_eq!(snapshot(&design.board), snapshot(&by_hand));
}

#[test]
fn validate_refuses_a_list_made_for_an_older_board() {
    let mut design = synced();
    design.parts[0].value = "22k".into();
    let list = design.diff();
    let engine = BoardEngine::new(design.board.clone(), design.revision + 1);
    assert_eq!(
        check(&list, &engine),
        Err(EcoRefusal::Stale {
            list: design.revision,
            board: design.revision + 1
        })
    );
}

#[test]
fn the_commands_come_in_the_fixed_order() {
    use crate::pcb::BoardCommand as C;
    let mut design = synced();
    design.parts.push(part("C1", &[0x301], "100n", Ok(r0603())));
    design.parts.retain(|p| p.part.designator != "R2");
    design.parts[0].footprint = Ok(resolved("R0805", 0x805, "1.0.0", &["1", "2"]));
    design.parts[0].value = "22k".into();
    design.board.footprints[2].schematic_link = None;
    design.netlist.nets[1]
        .terminals
        .push(netlist(&[("SIG", &[(0x301, "C1", "1")])]).nets[0].terminals[0].clone());
    let kinds: Vec<&str> = lower(&design.diff(), &design.board)
        .iter()
        .map(|c| match c {
            C::DeleteItems { .. } => "delete",
            C::ReplaceFootprint { .. } => "replace",
            C::SetFootprintText { .. } => "text",
            C::SetFootprintLink { .. } => "link",
            C::AddItems { .. } => "add",
            C::SetNets(_) => "nets",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["delete", "replace", "text", "link", "add", "nets"]);
}

#[test]
fn ticks_survive_a_refresh() {
    let mut design = synced();
    design.parts.push(part("C1", &[0x301], "100n", Ok(r0603())));
    design.parts[0].value = "22k".into();
    let mut list = design.diff();
    list.set_on(&EcoKey::SetValue(u(0xa1)), false);
    let mut refreshed = design.diff();
    refreshed.keep_choices_from(&list);
    assert!(!refreshed.row(&EcoKey::SetValue(u(0xa1))).unwrap().on);
    assert!(refreshed.row(&add_key(&[0x301])).unwrap().on);
}

#[test]
fn new_footprints_are_staged_apart_off_the_board_and_the_same_every_time() {
    let mut design = synced();
    for i in 0..12u128 {
        let footprint = if i % 2 == 0 { r0603() } else { sot23() };
        design.parts.push(part(
            &format!("C{}", i + 1),
            &[0x600 + i],
            "x",
            Ok(footprint),
        ));
    }
    let list = design.diff();
    let staged: Vec<&signex_types::pcb::Footprint> = list
        .rows
        .iter()
        .filter_map(|r| match &r.change {
            EcoChange::AddFootprint { footprint } => Some(&**footprint),
            _ => None,
        })
        .collect();
    assert_eq!(staged.len(), 12);
    let boxes: Vec<pcb_geom::RectNm> = staged
        .iter()
        .map(|f| {
            let e = pcb_geom::footprint_extent(f).unwrap();
            let o = pcb_geom::PointNm::from_mm(f.position, "p").unwrap();
            pcb_geom::RectNm::new(o.plus(e.min), o.plus(e.max))
        })
        .collect();
    let board_right = pcb_geom::mm_to_nm(50.0, "x").unwrap();
    for (i, a) in boxes.iter().enumerate() {
        assert!(a.min.x > board_right, "footprint {i} on the board");
        for b in &boxes[i + 1..] {
            assert!(!a.intersects(b), "two staged footprints overlap");
        }
    }
    let mut pads: Vec<uuid::Uuid> = staged
        .iter()
        .flat_map(|f| f.pads.iter().map(|p| p.uuid))
        .chain(
            design
                .board
                .footprints
                .iter()
                .flat_map(|f| f.pads.iter().map(|p| p.uuid)),
        )
        .collect();
    let total = pads.len();
    pads.sort_unstable();
    pads.dedup();
    assert_eq!(pads.len(), total, "every pad uuid is unique board-wide");
    assert_eq!(list, design.diff(), "the same staging every time");
}
