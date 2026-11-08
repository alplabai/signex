//! The footprint link: sheet path + symbol UUID, never the designator.

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

fn linked_footprint(reference: &str, pads: &[&str], symbol: Uuid, chain: &[Uuid]) -> Footprint {
    let mut fp = footprint(reference, pads);
    fp.schematic_link = Some(SymbolLink {
        sheet: InstancePath::from_chain(chain.to_vec()),
        symbol,
    });
    fp
}

fn net_number(pcb: &PcbBoard, index: usize, pad: usize) -> Option<u32> {
    pcb.footprints[index].pads[pad]
        .net
        .as_ref()
        .map(|n| n.number)
}

#[test]
fn a_linked_footprint_follows_its_symbol_through_a_re_annotation() {
    let symbol = Uuid::now_v7();
    // The schematic now calls the part R9; the footprint still says R1.
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![on_sheet(symbol, &[], "R9", "1")])],
    };
    let mut pcb = board(vec![linked_footprint("R1", &["1"], symbol, &[])]);
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_netlist(&netlist));
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_number(&pcb, 0, 0), Some(4));
}

#[test]
fn a_matching_designator_does_not_link_a_footprint_that_has_another_link() {
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![terminal("R1", "1")])],
    };
    let mut pcb = board(vec![linked_footprint("R1", &["1"], Uuid::now_v7(), &[])]);
    pcb.footprints[0].pads[0].net = Some(PadNet {
        number: 8,
        name: "KEEP".to_string(),
    });
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_netlist(&netlist));
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
        linked_footprint("R1", &["1"], symbol, &[sheet_a]),
        linked_footprint("R1", &["1"], symbol, &[sheet_b]),
    ]);
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_netlist(&netlist));
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_number(&pcb, 0, 0), Some(1));
    assert_eq!(net_number(&pcb, 1, 0), Some(2));
}

#[test]
fn a_footprint_with_no_link_is_matched_by_designator_and_reported() {
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![terminal("R1", "1")])],
    };
    let mut pcb = board(vec![footprint("R1", &["1"])]);
    let fp = pcb.footprints[0].uuid;
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_netlist(&netlist));
    assert_eq!(
        assignment.issues,
        vec![PadNetIssue::FootprintNotLinked { footprint: fp }]
    );
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_number(&pcb, 0, 0), Some(4));
}

#[test]
fn a_designator_two_symbols_carry_is_not_a_fallback_link() {
    let netlist = Netlist {
        nets: vec![
            net(1, "A", vec![on_sheet(Uuid::now_v7(), &[], "R1", "1")]),
            net(2, "B", vec![on_sheet(Uuid::now_v7(), &[], "R1", "1")]),
        ],
    };
    let mut pcb = board(vec![footprint("R1", &["1"])]);
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_netlist(&netlist));
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_number(&pcb, 0, 0), None);
    assert!(!pcb.footprints[0].pads[0].net_locked);
}

#[test]
fn a_pad_matching_no_pin_of_the_linked_symbol_is_reported_and_locked() {
    let symbol = Uuid::now_v7();
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![on_sheet(symbol, &[], "U1", "1")])],
    };
    let mut pcb = board(vec![linked_footprint("U1", &["1", "EP"], symbol, &[])]);
    pcb.footprints[0].pads[1].net = Some(PadNet {
        number: 8,
        name: "GND".to_string(),
    });
    let (fp, ep) = (pcb.footprints[0].uuid, pcb.footprints[0].pads[1].uuid);
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_netlist(&netlist));
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
        linked_footprint("R1", &["1"], symbol, &[]),
        linked_footprint("R1b", &["1"], symbol, &[]),
    ]);
    let (fp, pad) = (pcb.footprints[0].uuid, pcb.footprints[0].pads[0].uuid);
    set_pad_net_lock(&pcb, fp, pad, true).apply(&mut pcb);
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_netlist(&netlist));
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
