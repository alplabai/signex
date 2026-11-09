//! A library part's pin map lands a schematic pin on a differently numbered
//! pad (#663, #1312); with no map the key is 1:1, as before.

use super::*;
use crate::parts::DesignPart;
use signex_types::part::PinPadMap;
use signex_types::pcb::SymbolLink;

fn terminal(symbol: Uuid, pin: &str) -> Terminal {
    Terminal {
        symbol,
        reference: "Q1".to_string(),
        pin: pin.to_string(),
        occurrence: InstancePath::root(),
    }
}

/// Pin 1 of `on_unit` on net 3; a footprint with pads A1 and 1 linked to
/// `linked`.
fn part_on_root(on_unit: Uuid, linked: &[Uuid]) -> (Netlist, PcbBoard) {
    let netlist = Netlist {
        nets: vec![net(3, "SIG", vec![terminal(on_unit, "1")])],
    };
    let mut fp = footprint("Q1", &["A1", "1"]);
    fp.schematic_link = Some(SymbolLink::new(
        InstancePath::root(),
        linked.iter().copied(),
    ));
    (netlist, board(vec![fp]))
}

fn part(units: &[Uuid]) -> DesignPart {
    DesignPart {
        sheet: InstancePath::root(),
        designator: "Q1".into(),
        units: units.to_vec(),
        needs_annotation: false,
    }
}

fn nets_after(netlist: &Netlist, pcb: &PcbBoard, key: &SymbolPadKey) -> Vec<Option<u32>> {
    let mut pcb = pcb.clone();
    assign_pad_nets(netlist, &pcb, key).edit.apply(&mut pcb);
    pcb.footprints[0]
        .pads
        .iter()
        .map(|p| p.net.as_ref().map(|n| n.number))
        .collect()
}

#[test]
fn a_pin_map_puts_pin_1_on_pad_a1() {
    let symbol = Uuid::now_v7();
    let (netlist, pcb) = part_on_root(symbol, &[symbol]);
    let map = PinPadMap::from_pairs([("1", "A1")]);
    let key = SymbolPadKey::for_board(&pcb).with_pin_maps([(&part(&[symbol]), map)]);
    assert_eq!(nets_after(&netlist, &pcb, &key), [Some(3), None]);
}

#[test]
fn without_a_map_pin_1_stays_on_pad_1() {
    let symbol = Uuid::now_v7();
    let (netlist, pcb) = part_on_root(symbol, &[symbol]);
    let other = part(&[Uuid::now_v7()]);
    let map = PinPadMap::from_pairs([("1", "A1")]);
    for key in [
        SymbolPadKey::for_board(&pcb),
        SymbolPadKey::for_board(&pcb).with_pin_maps(Vec::<(&DesignPart, PinPadMap)>::new()),
        SymbolPadKey::for_board(&pcb).with_pin_maps([(&other, map)]),
    ] {
        assert_eq!(nets_after(&netlist, &pcb, &key), [None, Some(3)]);
    }
}

/// The footprint link lists only unit B, so its anchor is B while the
/// schematic part's smallest unit is A: the map is found by unit anyway.
#[test]
fn the_map_reaches_every_unit_whatever_the_link_lists() {
    let a = Uuid::from_u128(1);
    let b = Uuid::from_u128(2);
    let map = PinPadMap::from_pairs([("1", "A1")]);
    let design = part(&[a, b]);
    for (on_unit, linked) in [(b, vec![b]), (b, vec![a, b]), (a, vec![a, b])] {
        let (netlist, pcb) = part_on_root(on_unit, &linked);
        let key = SymbolPadKey::for_board(&pcb).with_pin_maps([(&design, map.clone())]);
        assert_eq!(
            nets_after(&netlist, &pcb, &key),
            [Some(3), None],
            "pin on {on_unit}, link {linked:?}"
        );
    }
}

#[test]
fn a_mapped_pad_is_cleaned_like_a_pin() {
    let symbol = Uuid::now_v7();
    let (netlist, pcb) = part_on_root(symbol, &[symbol]);
    let padded = PinPadMap::from_pairs([("1", " A1 ")]);
    let key = SymbolPadKey::for_board(&pcb).with_pin_maps([(&part(&[symbol]), padded)]);
    assert_eq!(nets_after(&netlist, &pcb, &key), [Some(3), None]);

    let blank = PinPadMap::from_pairs([("1", "  ")]);
    let key = SymbolPadKey::for_board(&pcb).with_pin_maps([(&part(&[symbol]), blank)]);
    assert!(key.terminal_key(&terminal(symbol, "1")).is_none());
    assert_eq!(nets_after(&netlist, &pcb, &key), [None, None]);
}
