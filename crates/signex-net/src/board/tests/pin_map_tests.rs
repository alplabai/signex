//! A library part's pin map lands a schematic pin on a differently numbered
//! pad (#663, #1312); with no map the key is 1:1, as before.

use std::collections::HashMap;

use super::*;
use crate::parts::PartAnchor;
use signex_types::part::PinPadMap;
use signex_types::pcb::SymbolLink;

fn part_on_root(symbol: Uuid) -> (Netlist, PcbBoard) {
    let netlist = Netlist {
        nets: vec![net(
            3,
            "SIG",
            vec![Terminal {
                symbol,
                reference: "Q1".to_string(),
                pin: "1".to_string(),
                occurrence: InstancePath::root(),
            }],
        )],
    };
    let mut fp = footprint("Q1", &["A1", "1"]);
    fp.schematic_link = Some(SymbolLink::new(InstancePath::root(), [symbol]));
    (netlist, board(vec![fp]))
}

fn net_on(pcb: &PcbBoard, pad: usize) -> Option<u32> {
    pcb.footprints[0].pads[pad].net.as_ref().map(|n| n.number)
}

#[test]
fn a_pin_map_puts_pin_1_on_pad_a1() {
    let symbol = Uuid::now_v7();
    let (netlist, mut pcb) = part_on_root(symbol);
    let maps = HashMap::from([(
        PartAnchor {
            sheet: InstancePath::root(),
            part: symbol,
        },
        PinPadMap::from_pairs([("1", "A1")]),
    )]);
    let key = SymbolPadKey::for_board(&pcb).with_pin_maps(maps);
    let assignment = assign_pad_nets(&netlist, &pcb, &key);
    assignment.edit.apply(&mut pcb);
    assert_eq!(net_on(&pcb, 0), Some(3), "pin 1 lands on pad A1");
    assert_eq!(net_on(&pcb, 1), None, "pad 1 is not pin 1 any more");
}

#[test]
fn without_a_map_pin_1_stays_on_pad_1() {
    let symbol = Uuid::now_v7();
    let (netlist, pcb) = part_on_root(symbol);
    let other_part = HashMap::from([(
        PartAnchor {
            sheet: InstancePath::root(),
            part: Uuid::now_v7(),
        },
        PinPadMap::from_pairs([("1", "A1")]),
    )]);
    for key in [
        SymbolPadKey::for_board(&pcb),
        SymbolPadKey::for_board(&pcb).with_pin_maps(HashMap::new()),
        SymbolPadKey::for_board(&pcb).with_pin_maps(other_part),
    ] {
        let mut scratch = pcb.clone();
        assign_pad_nets(&netlist, &scratch, &key)
            .edit
            .apply(&mut scratch);
        assert_eq!(net_on(&scratch, 1), Some(3));
        assert_eq!(net_on(&scratch, 0), None);
    }
}
