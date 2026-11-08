//! The footprint link: sheet path + the part's unit UUIDs, never the
//! designator; and the explicit migration that links old boards.

use super::*;
use signex_types::pcb::SymbolLink;

fn on_sheet(symbol: Uuid, chain: &[Uuid], reference: &str, pin: &str) -> Terminal {
    Terminal {
        symbol,
        reference: reference.to_string(),
        pin: pin.to_string(),
        occurrence: InstancePath::from_chain(chain.to_vec()),
    }
}

fn linked_footprint(reference: &str, pads: &[&str], units: &[Uuid], chain: &[Uuid]) -> Footprint {
    let mut fp = footprint(reference, pads);
    fp.schematic_link = Some(SymbolLink::new(
        InstancePath::from_chain(chain.to_vec()),
        units.iter().copied(),
    ));
    fp
}

fn net_number(pcb: &PcbBoard, index: usize, pad: usize) -> Option<u32> {
    pcb.footprints[index].pads[pad]
        .net
        .as_ref()
        .map(|n| n.number)
}

fn sync(netlist: &Netlist, pcb: &PcbBoard) -> BoardNetAssignment {
    assign_pad_nets(netlist, pcb, &SymbolPadKey::for_board(pcb))
}

#[test]
fn a_linked_footprint_follows_its_symbol_through_a_re_annotation() {
    let symbol = Uuid::now_v7();
    // The schematic now calls the part R9; the footprint still says R1.
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![on_sheet(symbol, &[], "R9", "1")])],
    };
    let mut pcb = board(vec![linked_footprint("R1", &["1"], &[symbol], &[])]);
    let assignment = sync(&netlist, &pcb);
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_number(&pcb, 0, 0), Some(4));
}

#[test]
fn a_matching_designator_does_not_link_a_footprint_that_has_another_link() {
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![terminal("R1", "1")])],
    };
    let mut pcb = board(vec![linked_footprint("R1", &["1"], &[Uuid::now_v7()], &[])]);
    pcb.footprints[0].pads[0].net = Some(PadNet {
        number: 8,
        name: "KEEP".to_string(),
    });
    let assignment = sync(&netlist, &pcb);
    let fp = pcb.footprints[0].uuid;
    assert!(
        assignment
            .issues
            .contains(&PadNetIssue::FootprintNotInSchematic { footprint: fp }),
        "{:?}",
        assignment.issues
    );
    assert!(
        !assignment
            .issues
            .iter()
            .any(|i| matches!(i, PadNetIssue::FootprintNotLinked { .. })),
        "{:?}",
        assignment.issues
    );
    assignment.edit.apply(&mut pcb);
    let pad = &pcb.footprints[0].pads[0];
    assert_eq!(pad.net.as_ref().map(|n| n.number), Some(8), "net kept");
    assert!(!pad.net_locked, "reported, not locked");
}

#[test]
fn an_unnumbered_pad_of_a_footprint_not_in_the_schematic_is_not_locked() {
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![terminal("R1", "1")])],
    };
    let mut pcb = board(vec![linked_footprint(
        "R1",
        &["1", ""],
        &[Uuid::now_v7()],
        &[],
    )]);
    pcb.footprints[0].pads[1].net = Some(PadNet {
        number: 8,
        name: "KEEP".to_string(),
    });
    sync(&netlist, &pcb).edit.apply(&mut pcb);
    assert!(!pcb.footprints[0].pads[1].net_locked);
    assert_eq!(net_number(&pcb, 0, 1), Some(8));
}

#[test]
fn the_sheet_path_tells_two_placements_of_one_symbol_apart() {
    let symbol = Uuid::now_v7();
    let (sheet_a, sheet_b) = (Uuid::now_v7(), Uuid::now_v7());
    let netlist = Netlist {
        nets: vec![
            net(1, "A", vec![on_sheet(symbol, &[sheet_a], "R1", "1")]),
            net(2, "B", vec![on_sheet(symbol, &[sheet_b], "R1", "1")]),
        ],
    };
    let mut pcb = board(vec![
        linked_footprint("R1", &["1"], &[symbol], &[sheet_a]),
        linked_footprint("R1", &["1"], &[symbol], &[sheet_b]),
    ]);
    let assignment = sync(&netlist, &pcb);
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_number(&pcb, 0, 0), Some(1));
    assert_eq!(net_number(&pcb, 1, 0), Some(2));
}

#[test]
fn a_footprint_with_no_link_is_reported_and_left_untouched() {
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![terminal("R1", "1")])],
    };
    let mut pcb = board(vec![footprint("R1", &["1", ""])]);
    let hand = PadNet {
        number: 8,
        name: "KEEP".to_string(),
    };
    pcb.footprints[0].pads[0].net = Some(hand.clone());
    pcb.footprints[0].pads[1].net = Some(hand.clone());
    let before = pcb.footprints.clone();
    let fp = pcb.footprints[0].uuid;

    let assignment = sync(&netlist, &pcb);
    assert!(
        assignment
            .issues
            .contains(&PadNetIssue::FootprintNotLinked { footprint: fp }),
        "{:?}",
        assignment.issues
    );
    assert!(
        assignment.edit.pads.is_empty(),
        "{:?}",
        assignment.edit.pads
    );
    assignment.edit.apply(&mut pcb);
    assert_eq!(pcb.footprints, before, "no net change, no auto-lock");
}

/// `U1`: unit 1 holds pins 1 and 2, unit 2 holds pins 3 and 4; two symbols
/// under one designator.
fn two_unit_netlist() -> (Netlist, [Uuid; 2]) {
    let nets = vec![
        net(1, "A", vec![terminal_unit("U1", "1", 1)]),
        net(2, "B", vec![terminal_unit("U1", "2", 1)]),
        net(3, "C", vec![terminal_unit("U1", "3", 2)]),
        net(4, "D", vec![terminal_unit("U1", "4", 2)]),
    ];
    let units = [symbol_of("U1", 1), symbol_of("U1", 2)];
    (Netlist { nets }, units)
}

#[test]
fn a_two_unit_part_lands_every_pin_of_both_units_on_its_pad() {
    let (netlist, units) = two_unit_netlist();
    let mut pcb = board(vec![linked_footprint(
        "U1",
        &["1", "2", "3", "4"],
        &units,
        &[],
    )]);
    let assignment = sync(&netlist, &pcb);
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);
    for (pad, number) in [(0, 1), (1, 2), (2, 3), (3, 4)] {
        assert_eq!(net_number(&pcb, 0, pad), Some(number), "pad {}", pad + 1);
    }
    assert!(pcb.footprints[0].pads.iter().all(|p| !p.net_locked));
}

#[test]
fn a_two_unit_part_locks_nothing_even_when_pads_already_hold_nets() {
    let (netlist, units) = two_unit_netlist();
    let mut pcb = board(vec![linked_footprint(
        "U1",
        &["1", "2", "3", "4"],
        &units,
        &[],
    )]);
    let old = PadNet {
        number: 9,
        name: "OLD".to_string(),
    };
    for pad in &mut pcb.footprints[0].pads {
        pad.net = Some(old.clone());
    }
    pcb.nets.push(NetDef {
        number: 9,
        name: "OLD".to_string(),
    });
    let assignment = sync(&netlist, &pcb);
    assert!(
        !assignment
            .issues
            .iter()
            .any(|i| matches!(i, PadNetIssue::PadNotOnSymbol { .. })),
        "{:?}",
        assignment.issues
    );
    assignment.edit.apply(&mut pcb);
    assert!(pcb.footprints[0].pads.iter().all(|p| !p.net_locked));
    assert_eq!(net_number(&pcb, 0, 3), Some(4));
}

#[test]
fn a_link_that_names_only_one_unit_leaves_the_other_units_pins_unlinked() {
    let (netlist, units) = two_unit_netlist();
    let mut pcb = board(vec![linked_footprint(
        "U1",
        &["1", "2", "3", "4"],
        &units[..1],
        &[],
    )]);
    let assignment = sync(&netlist, &pcb);
    let missing: Vec<&str> = assignment
        .issues
        .iter()
        .filter_map(|i| match i {
            PadNetIssue::TerminalWithoutPad { terminal, .. } => Some(terminal.pin.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(missing, ["3", "4"]);
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_number(&pcb, 0, 0), Some(1));
    assert_eq!(net_number(&pcb, 0, 2), None);
}

#[test]
fn the_pad_that_fits_no_pin_of_a_two_unit_part_is_reported_and_locked() {
    let (netlist, units) = two_unit_netlist();
    let mut pcb = board(vec![linked_footprint(
        "U1",
        &["1", "2", "3", "4", "EP"],
        &units,
        &[],
    )]);
    pcb.footprints[0].pads[4].net = Some(PadNet {
        number: 9,
        name: "GND".to_string(),
    });
    let (fp, ep) = (pcb.footprints[0].uuid, pcb.footprints[0].pads[4].uuid);
    let assignment = sync(&netlist, &pcb);
    assert!(assignment.issues.contains(&PadNetIssue::PadNotOnSymbol {
        footprint: fp,
        pad: ep
    }));
    assignment.edit.apply(&mut pcb);
    assert!(pcb.footprints[0].pads[4].net_locked);
    assert!(!pcb.footprints[0].pads[0].net_locked);
}

#[test]
fn a_pad_matching_no_pin_of_the_linked_symbol_is_reported_and_locked() {
    let symbol = Uuid::now_v7();
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![on_sheet(symbol, &[], "U1", "1")])],
    };
    let mut pcb = board(vec![linked_footprint("U1", &["1", "EP"], &[symbol], &[])]);
    pcb.footprints[0].pads[1].net = Some(PadNet {
        number: 8,
        name: "GND".to_string(),
    });
    let (fp, ep) = (pcb.footprints[0].uuid, pcb.footprints[0].pads[1].uuid);
    let assignment = sync(&netlist, &pcb);
    assert!(assignment.issues.contains(&PadNetIssue::PadNotOnSymbol {
        footprint: fp,
        pad: ep
    }));
    assignment.edit.apply(&mut pcb);
    assert!(pcb.footprints[0].pads[1].net_locked);
    assert!(!pcb.footprints[0].pads[0].net_locked);
    assert_eq!(net_number(&pcb, 0, 0), Some(4));
    assert_eq!(net_number(&pcb, 0, 1), Some(8));
}

#[test]
fn a_link_shared_by_two_footprints_is_a_collision_even_with_a_locked_pad() {
    let symbol = Uuid::now_v7();
    let netlist = Netlist {
        nets: vec![net(3, "SIG", vec![on_sheet(symbol, &[], "R1", "1")])],
    };
    let mut pcb = board(vec![
        linked_footprint("R1", &["1"], &[symbol], &[]),
        linked_footprint("R1b", &["1"], &[symbol], &[]),
    ]);
    let (fp, pad) = (pcb.footprints[0].uuid, pcb.footprints[0].pads[0].uuid);
    set_pad_net_lock(&pcb, fp, pad, true).apply(&mut pcb);
    let assignment = sync(&netlist, &pcb);
    assert!(
        assignment
            .issues
            .iter()
            .any(|i| matches!(i, PadNetIssue::KeyOnSeveralFootprints { .. })),
        "{:?}",
        assignment.issues
    );
    assert!(assignment.edit.pads.is_empty());
}

// ---- link_footprints_by_designator -------------------------------------

use signex_types::schematic::Symbol;

fn unit_symbol(reference: &str, lib_id: &str, unit: u32) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        unit,
        ..Default::default()
    }
}

fn sheet_of(symbols: Vec<Symbol>) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.symbols = symbols;
    sheet
}

fn link_of(pcb: &PcbBoard, index: usize) -> Option<&SymbolLink> {
    pcb.footprints[index].schematic_link.as_ref()
}

#[test]
fn migration_links_a_single_unit_part_and_the_inverse_unlinks_it() {
    let r1 = unit_symbol("R1", "R", 1);
    let uuid = r1.uuid;
    let sheet = sheet_of(vec![r1, unit_symbol("R2", "R", 1)]);
    let mut pcb = board(vec![footprint("R1", &["1"])]);

    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert_eq!(report.linked, vec![pcb.footprints[0].uuid]);
    assert!(report.ambiguous.is_empty() && report.unmatched.is_empty());
    let inverse = edit.apply(&mut pcb);
    assert_eq!(
        link_of(&pcb, 0),
        Some(&SymbolLink::new(InstancePath::root(), [uuid]))
    );
    inverse.apply(&mut pcb);
    assert_eq!(link_of(&pcb, 0), None);
}

#[test]
fn migration_links_a_multi_unit_part_to_every_unit() {
    let (a, b) = (unit_symbol("U1", "OPAMP", 1), unit_symbol("U1", "OPAMP", 2));
    let units = [a.uuid, b.uuid];
    let sheet = sheet_of(vec![a, b]);
    let mut pcb = board(vec![footprint("U1", &["1"])]);
    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert_eq!(report.linked.len(), 1);
    edit.apply(&mut pcb);
    assert_eq!(
        link_of(&pcb, 0),
        Some(&SymbolLink::new(InstancePath::root(), units))
    );
}

#[test]
fn migration_leaves_an_ambiguous_designator_unlinked_and_reports_it() {
    // Unit 1 twice under one designator is two parts, not a second unit.
    let sheet = sheet_of(vec![unit_symbol("R5", "R", 1), unit_symbol("R5", "R", 1)]);
    let mut pcb = board(vec![footprint("R5", &["1"]), footprint("R8", &["1"])]);
    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert!(edit.links.is_empty());
    assert_eq!(report.ambiguous, vec![pcb.footprints[0].uuid]);
    assert_eq!(
        report.unmatched,
        vec![pcb.footprints[1].uuid],
        "R8: no part"
    );
    edit.apply(&mut pcb);
    assert_eq!(link_of(&pcb, 0), None);
}

#[test]
fn migration_reports_two_footprints_that_claim_one_part() {
    let sheet = sheet_of(vec![unit_symbol("R1", "R", 1)]);
    let pcb = board(vec![footprint("R1", &["1"]), footprint("R1", &["1"])]);
    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert!(edit.links.is_empty());
    assert_eq!(report.ambiguous.len(), 2);
}

#[test]
fn migration_never_touches_a_footprint_that_is_already_linked() {
    let r1 = unit_symbol("R1", "R", 1);
    let r2 = unit_symbol("R2", "R", 1);
    let r2_uuid = r2.uuid;
    let sheet = sheet_of(vec![r1, r2]);
    let elsewhere = Uuid::now_v7();
    let mut pcb = board(vec![
        linked_footprint("R1", &["1"], &[elsewhere], &[]),
        footprint("R2", &["1"]),
    ]);
    let kept = link_of(&pcb, 0).cloned();
    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert_eq!(report.linked, vec![pcb.footprints[1].uuid]);
    assert_eq!(edit.links.len(), 1);
    edit.apply(&mut pcb);
    assert_eq!(link_of(&pcb, 0).cloned(), kept);
    assert_eq!(
        link_of(&pcb, 1),
        Some(&SymbolLink::new(InstancePath::root(), [r2_uuid]))
    );
}

#[test]
fn migration_does_not_take_a_part_another_footprint_already_holds() {
    let r1 = unit_symbol("R1", "R", 1);
    let held = r1.uuid;
    let sheet = sheet_of(vec![r1]);
    let pcb = board(vec![
        linked_footprint("OTHER", &["1"], &[held], &[]),
        footprint("R1", &["1"]),
    ]);
    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert!(edit.links.is_empty());
    assert_eq!(report.ambiguous, vec![pcb.footprints[1].uuid]);
}

#[test]
fn migration_skips_unannotated_and_blank_references() {
    let sheet = sheet_of(vec![unit_symbol("R?", "R", 1)]);
    let pcb = board(vec![footprint("R?", &["1"]), footprint("", &["1"])]);
    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert!(edit.links.is_empty());
    assert_eq!(report.unmatched.len(), 2);
}

#[test]
fn a_migrated_board_syncs_with_no_unlinked_footprint() {
    let sheet = divider_sheet();
    let netlist = build_netlist(&sheet);
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    assert!(
        sync(&netlist, &pcb)
            .issues
            .iter()
            .any(|i| matches!(i, PadNetIssue::FootprintNotLinked { .. }))
    );
    let (edit, report) = link_footprints_by_designator(&pcb, &[(InstancePath::root(), &sheet)]);
    assert_eq!(report.linked.len(), 2);
    edit.apply(&mut pcb);
    let assignment = sync(&netlist, &pcb);
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "R1", "2"), Some(&net_of(&netlist, "R1", "2")));
}
