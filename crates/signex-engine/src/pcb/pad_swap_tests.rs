//! The board half of a pin or gate swap (#2410): pad nets trade in one
//! command whose inverse puts them back; the footprint's switches, its lock
//! and a locked pad net refuse it.

use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::*;

const U1: Uuid = Uuid::from_u128(0xa);

fn pad(number: &str, x: f64, net: Option<(u32, &str)>, locked: bool) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::now_v7(), "number": number, "pad_type": "smd", "shape": "rect",
        "position": {"x": x, "y": 0.0}, "size": {"x": 0.6, "y": 1.1}, "drill": null,
        "layers": ["TopCopper"], "net_locked": locked,
        "net": net.map(|(number, name)| serde_json::json!({"number": number, "name": name}))
    })
}

/// U1, a quad part: pads 1 (A), 2 (B), 3 (C) and 4 (no net), with both
/// swapping switches as given and pad 4's net locked when `lock_4`.
fn board(pins: bool, parts: bool, lock_4: bool) -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "A"}, {"number": 2, "name": "B"}, {"number": 3, "name": "C"}
        ],
        "footprints": [{
            "uuid": U1, "reference": "U1", "value": "QUAD", "footprint_id": "SO8",
            "position": {"x": 5.0, "y": 5.0}, "layer": "TopCopper",
            "component": {"pin_swapping": pins, "part_swapping": parts},
            "pads": [
                pad("1", -1.0, Some((1, "A")), false),
                pad("2", 1.0, Some((2, "B")), false),
                pad("3", 2.0, Some((3, "C")), false),
                pad("4", 3.0, None, lock_4)
            ]
        }]
    }))
    .unwrap()
}

fn nets(board: &PcbBoard) -> Vec<(String, Option<String>)> {
    board.footprints[0]
        .pads
        .iter()
        .map(|p| (p.number.clone(), p.net.as_ref().map(|n| n.name.clone())))
        .collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

fn named(list: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    list.iter()
        .map(|(n, net)| (n.to_string(), net.map(str::to_string)))
        .collect()
}

#[test]
fn two_pins_trade_nets_and_the_inverse_puts_them_back() {
    let mut b = board(true, false, false);
    let before = nets(&b);
    let swap = pad_swap(&b, U1, &[pair("1", "2")], SwapKind::Pins).unwrap();
    assert_eq!(swap.commands.len(), 1, "one command, one undo step");
    assert_eq!(
        swap.changes,
        [
            PadNetChange {
                pad: "1".into(),
                from: Some("A".into()),
                to: Some("B".into())
            },
            PadNetChange {
                pad: "2".into(),
                from: Some("B".into()),
                to: Some("A".into())
            },
        ]
    );
    let applied = swap.commands[0].clone().apply(&mut b).unwrap();
    assert_eq!(
        nets(&b),
        named(&[
            ("1", Some("B")),
            ("2", Some("A")),
            ("3", Some("C")),
            ("4", None)
        ])
    );
    applied.inverse.apply(&mut b).unwrap();
    assert_eq!(nets(&b), before);
}

#[test]
fn a_gate_swap_trades_every_pair_and_a_pad_without_net_takes_none() {
    let mut b = board(false, true, false);
    let swap = pad_swap(&b, U1, &[pair("1", "3"), pair("2", "4")], SwapKind::Gates).unwrap();
    for command in swap.commands {
        command.apply(&mut b).unwrap();
    }
    assert_eq!(
        nets(&b),
        named(&[
            ("1", Some("C")),
            ("2", None),
            ("3", Some("A")),
            ("4", Some("B"))
        ])
    );
}

#[test]
fn pins_on_one_net_need_no_command() {
    let mut b = board(true, false, false);
    b.footprints[0].pads[1].net = b.footprints[0].pads[0].net.clone();
    let swap = pad_swap(&b, U1, &[pair("1", "2")], SwapKind::Pins).unwrap();
    assert!(swap.commands.is_empty());
    assert!(swap.changes.is_empty());
}

#[test]
fn the_switches_the_lock_and_a_locked_net_refuse() {
    let pins = [pair("1", "2")];
    assert_eq!(
        pad_swap(&board(false, true, false), U1, &pins, SwapKind::Pins),
        Err(PadSwapRefusal::PinSwappingOff("U1".into()))
    );
    assert_eq!(
        pad_swap(&board(true, false, false), U1, &pins, SwapKind::Gates),
        Err(PadSwapRefusal::PartSwappingOff("U1".into()))
    );
    let mut locked = board(true, true, false);
    locked.footprints[0].locked = true;
    assert_eq!(
        pad_swap(&locked, U1, &pins, SwapKind::Pins),
        Err(PadSwapRefusal::Locked("U1".into()))
    );
    assert_eq!(
        pad_swap(
            &board(true, true, true),
            U1,
            &[pair("3", "4")],
            SwapKind::Pins
        ),
        Err(PadSwapRefusal::NetLocked {
            designator: "U1".into(),
            pad: "4".into()
        })
    );
}

#[test]
fn a_missing_pad_and_a_pad_named_twice_are_refused() {
    let b = board(true, true, false);
    assert_eq!(
        pad_swap(&b, U1, &[pair("1", "9")], SwapKind::Pins),
        Err(PadSwapRefusal::NoPad {
            designator: "U1".into(),
            pad: "9".into()
        })
    );
    assert_eq!(
        pad_swap(&b, U1, &[pair("1", "2"), pair("2", "3")], SwapKind::Gates),
        Err(PadSwapRefusal::NamedTwice {
            designator: "U1".into(),
            pad: "2".into()
        })
    );
    assert_eq!(
        pad_swap(&b, Uuid::from_u128(0xff), &[pair("1", "2")], SwapKind::Pins),
        Err(PadSwapRefusal::NoFootprint(Uuid::from_u128(0xff)))
    );
}
