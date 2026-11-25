use uuid::Uuid;

use super::{NetTieStatus, board_net_ties, net_ties};
use crate::net::{Net, NetId, Netlist, Terminal};
use crate::pcb::{ComponentType, PcbBoard};
use crate::schematic::{InstancePath, SchematicSheet, Symbol};

const TIE: Uuid = Uuid::from_u128(0x1277_0001);

fn sheet_with(symbol: Symbol) -> SchematicSheet {
    SchematicSheet {
        symbols: vec![symbol],
        ..Default::default()
    }
}

fn tie() -> Symbol {
    Symbol {
        uuid: TIE,
        reference: "NT1".to_string(),
        net_tie: true,
        ..Default::default()
    }
}

fn terminal(path: &InstancePath, reference: &str, pin: &str) -> Terminal {
    Terminal {
        symbol: TIE,
        reference: reference.to_string(),
        pin: pin.to_string(),
        occurrence: path.clone(),
    }
}

fn net(id: u32, name: &str, terminals: Vec<Terminal>) -> Net {
    Net {
        id: NetId(id),
        name: name.to_string(),
        class: None,
        wires: Vec::new(),
        junctions: Vec::new(),
        terminals,
    }
}

/// A tie placed in two occurrences of one sheet is two ties, each with the
/// nets and the designator of its own occurrence; other parts' terminals
/// are passed over.
#[test]
fn each_occurrence_of_a_tie_is_its_own_tie() {
    let a = InstancePath::root().child(Uuid::from_u128(1));
    let b = InstancePath::root().child(Uuid::from_u128(2));
    let sheet = sheet_with(tie());
    let other = Terminal {
        symbol: Uuid::from_u128(0x1277_0002),
        ..terminal(&a, "R1", "1")
    };
    let netlist = Netlist {
        nets: vec![
            net(1, "A/GND", vec![terminal(&a, "NT1_A", "1"), other]),
            net(2, "A/AGND", vec![terminal(&a, "NT1_A", "2")]),
            net(3, "B/GND", vec![terminal(&b, "NT1_B", "1")]),
            net(4, "B/AGND", vec![terminal(&b, "NT1_B", "2")]),
        ],
    };
    let ties = net_ties(&netlist, &[(a, &sheet), (b, &sheet)]);
    assert_eq!(ties.len(), 2);
    assert_eq!(ties[0].designator, "NT1_A");
    assert_eq!(ties[0].nets(), ["A/AGND", "A/GND"]);
    assert_eq!(ties[0].pins.len(), 2, "R1's terminal is not the tie's");
    assert_eq!(ties[1].designator, "NT1_B");
    assert_eq!(ties[1].nets(), ["B/AGND", "B/GND"]);
}

#[test]
fn the_status_follows_the_pins_on_a_net() {
    let root = InstancePath::root();
    let sheet = sheet_with(tie());
    let status =
        |nets: Vec<Net>| net_ties(&Netlist { nets }, &[(root.clone(), &sheet)])[0].status();
    assert_eq!(status(vec![]), NetTieStatus::Open);
    assert_eq!(
        status(vec![net(1, "GND", vec![terminal(&root, "NT1", "1")])]),
        NetTieStatus::Open
    );
    assert_eq!(
        status(vec![net(
            1,
            "GND",
            vec![terminal(&root, "NT1", "1"), terminal(&root, "NT1", "2")]
        )]),
        NetTieStatus::OneNet
    );
    assert_eq!(
        status(vec![
            net(1, "GND", vec![terminal(&root, "NT1", "1")]),
            net(2, "AGND", vec![terminal(&root, "NT1", "2")]),
        ]),
        NetTieStatus::Joins
    );
}

#[test]
fn a_power_port_or_an_untied_part_is_never_a_tie() {
    let root = InstancePath::root();
    let nets = Netlist {
        nets: vec![net(1, "GND", vec![terminal(&root, "NT1", "1")])],
    };
    let power = sheet_with(Symbol {
        is_power: true,
        ..tie()
    });
    let untied = sheet_with(Symbol {
        net_tie: false,
        ..tie()
    });
    assert!(net_ties(&nets, &[(root.clone(), &power)]).is_empty());
    assert!(net_ties(&nets, &[(root, &untied)]).is_empty());
}

/// A footprint `reference` with pads `(number, net number, net name)`.
fn footprint(n: u128, reference: &str, pads: &[(&str, u32, &str)]) -> serde_json::Value {
    let pads: Vec<serde_json::Value> = pads
        .iter()
        .enumerate()
        .map(|(i, (number, net, name))| {
            serde_json::json!({
                "uuid": Uuid::from_u128(0x100 * n + i as u128), "number": number,
                "pad_type": "smd", "shape": "rect",
                "position": {"x": i as f64, "y": 0.0}, "size": {"x": 0.6, "y": 1.0},
                "drill": null, "layers": ["TopCopper"],
                "net": if *net == 0 { serde_json::Value::Null } else {
                    serde_json::json!({"number": net, "name": name})
                }
            })
        })
        .collect();
    serde_json::json!({
        "uuid": Uuid::from_u128(n), "reference": reference, "value": "",
        "footprint_id": "NT", "position": {"x": 0.0, "y": 0.0},
        "rotation": 0.0, "layer": "TopCopper", "pads": pads, "properties": []
    })
}

#[test]
fn a_boards_net_ties_are_its_net_tie_footprints_with_their_pad_nets() {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::nil(),
        "footprints": [
            footprint(1, "NT1", &[("2", 2, "AGND"), ("1", 1, "GND")]),
            footprint(2, "R1", &[("1", 1, "GND"), ("2", 3, "SIG")]),
            footprint(3, "NT2", &[("1", 1, "GND"), ("2", 0, "")]),
        ],
    }))
    .unwrap();
    board.footprints[0].component.component_type = ComponentType::NetTie;
    board.footprints[2].component.component_type = ComponentType::NetTieInBom;

    let ties = board_net_ties(&board);
    assert_eq!(ties.len(), 2, "R1 is a standard part");
    assert_eq!(ties[0].designator, "NT1");
    assert_eq!(ties[0].footprint, Uuid::from_u128(1));
    assert_eq!(ties[0].nets(), ["AGND", "GND"]);
    assert_eq!(ties[0].pads[0].pin, "1", "by pad number");
    assert_eq!(ties[0].status(), NetTieStatus::Joins);
    assert_eq!(ties[1].component_type, ComponentType::NetTieInBom);
    assert_eq!(ties[1].pads.len(), 1, "a pad of no net is left out");
    assert_eq!(ties[1].status(), NetTieStatus::Open);
}

#[test]
fn the_status_names_are_stable() {
    assert_eq!(NetTieStatus::Joins.as_str(), "joins");
    assert_eq!(NetTieStatus::OneNet.as_str(), "one_net");
    assert_eq!(NetTieStatus::Open.as_str(), "open");
}
