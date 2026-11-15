//! The board footprint swap (#2366): one replace per footprint, pad nets
//! carried by number, the old pads with no match listed, a lock refused.

use std::collections::BTreeSet;

use signex_types::pcb::{Footprint, PcbBoard};
use uuid::Uuid;

use super::*;
use crate::pcb::placement::PlacementRefusal;

const U1: Uuid = Uuid::from_u128(0xa);
const U2: Uuid = Uuid::from_u128(0xb);

fn pad(number: &str, x: f64, net: Option<(u32, &str)>) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::now_v7(), "number": number, "pad_type": "smd", "shape": "rect",
        "position": {"x": x, "y": 0.0}, "size": {"x": 0.6, "y": 1.1}, "drill": null,
        "layers": ["TopCopper"],
        "net": net.map(|(number, name)| serde_json::json!({"number": number, "name": name}))
    })
}

/// U1 and U2 with pads 1 (GND) and 2 (VCC), U2 turned 90° on the bottom.
fn board() -> PcbBoard {
    let footprint = |uuid: Uuid, reference: &str, layer: &str, rotation: f64| {
        serde_json::json!({
            "uuid": uuid, "reference": reference, "value": "10k", "footprint_id": "R_0603",
            "position": {"x": 5.0, "y": 7.5}, "rotation": rotation, "layer": layer,
            "pads": [pad("1", -1.0, Some((1, "GND"))), pad("2", 1.0, Some((2, "VCC")))]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "VCC"}],
        "footprints": [
            footprint(U1, "R1", "TopCopper", 0.0),
            footprint(U2, "R2", "BottomCopper", 90.0),
        ]
    }))
    .unwrap()
}

/// A library footprint with pads 1 and 3.
fn template() -> Footprint {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0x99), "footprint_id": "R_0805", "layer": "TopCopper",
        "position": {"x": 0.0, "y": 0.0},
        "pads": [pad("1", -1.5, None), pad("3", 1.5, None)]
    }))
    .unwrap()
}

fn ids(list: &[Uuid]) -> BTreeSet<Uuid> {
    list.iter().copied().collect()
}

fn fp(board: &PcbBoard, id: Uuid) -> &Footprint {
    board.footprints.iter().find(|f| f.uuid == id).unwrap()
}

#[test]
fn the_swap_keeps_the_place_and_carries_nets_by_pad_number() {
    let mut b = board();
    let swap = swap_commands(&b, &ids(&[U1]), &template()).unwrap();
    assert_eq!(swap.commands.len(), 1);
    for command in swap.commands {
        command.apply(&mut b).unwrap();
    }
    let r1 = fp(&b, U1);
    assert_eq!(r1.footprint_id, "R_0805");
    assert_eq!((r1.position.x, r1.position.y, r1.rotation), (5.0, 7.5, 0.0));
    assert_eq!(r1.reference, "R1");
    let nets: Vec<(&str, Option<&str>)> = r1
        .pads
        .iter()
        .map(|p| (p.number.as_str(), p.net.as_ref().map(|n| n.name.as_str())))
        .collect();
    assert_eq!(nets, [("1", Some("GND")), ("3", None)]);
}

#[test]
fn an_old_pad_with_no_match_is_listed_with_its_net() {
    let b = board();
    let swap = swap_commands(&b, &ids(&[U1]), &template()).unwrap();
    assert_eq!(
        swap.unmatched,
        [UnmatchedPad {
            reference: "R1".into(),
            number: "2".into(),
            net: Some("VCC".into()),
        }]
    );
}

#[test]
fn several_footprints_get_their_own_pad_uuids_and_a_bottom_one_stays_on_the_bottom() {
    let mut b = board();
    let swap = swap_commands(&b, &ids(&[U1, U2]), &template()).unwrap();
    assert_eq!(swap.unmatched.len(), 2);
    for command in swap.commands {
        command.apply(&mut b).unwrap();
    }
    let pads: BTreeSet<Uuid> = b
        .footprints
        .iter()
        .flat_map(|f| f.pads.iter().map(|p| p.uuid))
        .collect();
    assert_eq!(pads.len(), 4, "every pad uuid is its own");
    let r2 = fp(&b, U2);
    assert_eq!(r2.layer, "BottomCopper");
    assert_eq!(r2.rotation, 90.0);
}

#[test]
fn a_locked_footprint_is_refused() {
    let mut b = board();
    b.footprints[0].locked = true;
    let refused = swap_commands(&b, &ids(&[U1]), &template()).unwrap_err();
    assert_eq!(
        refused,
        ComponentRefusal::Placement(PlacementRefusal::Locked(vec!["R1".into()]))
    );
}

#[test]
fn fewer_pads_of_a_number_holding_a_net_are_refused() {
    let b = board();
    let mut one_pad = template();
    one_pad.pads.truncate(1);
    one_pad.pads[0].number = "2".into();
    // R1 with two pads numbered 2, both on VCC: one new pad 2 cannot take both.
    let mut doubled = board();
    let mut extra = doubled.footprints[0].pads[1].clone();
    extra.uuid = Uuid::now_v7();
    doubled.footprints[0].pads.push(extra);
    assert!(swap_commands(&b, &ids(&[U1]), &one_pad).is_ok());
    let refused = swap_commands(&doubled, &ids(&[U1]), &one_pad).unwrap_err();
    assert!(matches!(
        refused,
        ComponentRefusal::Command(crate::pcb::BoardCommandError::PadCountMismatch { .. })
    ));
}
