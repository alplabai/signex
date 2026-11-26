//! #2968: generate a block diagram from what the hierarchy says, and
//! regenerate it keeping the person's layout.

use uuid::Uuid;

use super::*;
use crate::object_id::SequentialIds;

fn doc(n: u128) -> DocumentId {
    DocumentId::from_uuid(Uuid::from_u128(0xd000 + n)).unwrap()
}

fn port(name: &str, direction: PortDirection) -> DerivedPort {
    DerivedPort {
        name: name.into(),
        direction,
    }
}

fn sheet(n: u128, path: &str, name: &str, ports: Vec<DerivedPort>) -> DerivedBlock {
    DerivedBlock {
        sheet: doc(n),
        path: path.into(),
        name: name.into(),
        depth: 1,
        rails: Vec::new(),
        ports,
    }
}

fn wire(
    group: &str,
    from: (u128, &str),
    to: (u128, &str),
    interface: Interface,
) -> DerivedConnection {
    DerivedConnection {
        group: group.into(),
        interface,
        label: group.into(),
        from: DerivedEnd {
            sheet: doc(from.0),
            port: Some(from.1.into()),
        },
        to: DerivedEnd {
            sheet: doc(to.0),
            port: Some(to.1.into()),
        },
    }
}

/// Power In feeds VIN_12V to the buck; the buck feeds VOUT_3V3 to the MCU.
fn three_sheets() -> DerivedDiagram {
    DerivedDiagram {
        blocks: vec![
            sheet(
                1,
                "power_in.snxsch",
                "Power In",
                vec![port("VIN_12V", PortDirection::Output)],
            ),
            sheet(
                2,
                "dcdc.snxsch",
                "DC/DC Buck",
                vec![
                    port("VIN_12V", PortDirection::Input),
                    port("VOUT_3V3", PortDirection::Output),
                ],
            ),
            sheet(
                3,
                "mcu.snxsch",
                "MCU",
                vec![port("VOUT_3V3", PortDirection::Input)],
            ),
        ],
        connections: vec![
            wire("VIN_12V", (1, "VIN_12V"), (2, "VIN_12V"), Interface::Power),
            wire(
                "VOUT_3V3",
                (2, "VOUT_3V3"),
                (3, "VOUT_3V3"),
                Interface::Power,
            ),
        ],
        unidentified: Vec::new(),
    }
}

fn generated(derived: &DerivedDiagram, ids: &mut SequentialIds) -> BlockDiagram {
    generate_block_diagram(derived, ids, doc(0x100)).unwrap()
}

fn block_named<'a>(d: &'a BlockDiagram, name: &str) -> &'a Block {
    d.blocks.iter().find(|b| b.name == name).unwrap()
}

#[test]
fn generate_makes_one_linked_block_per_sheet_with_its_ports() {
    let d = generated(&three_sheets(), &mut SequentialIds::default());
    let names: Vec<&str> = d.blocks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["Power In", "DC/DC Buck", "MCU"]);
    let buck = block_named(&d, "DC/DC Buck");
    let link = buck.sheet.as_ref().unwrap();
    assert_eq!(link.sheet, doc(2));
    assert_eq!(link.path, "dcdc.snxsch");
    assert_eq!(link.state, LinkState::Linked);
    let ports: Vec<(&str, PortSide, f64)> = buck
        .ports
        .iter()
        .map(|p| (p.name.as_str(), p.side, p.offset_mm))
        .collect();
    assert_eq!(
        ports,
        [
            ("VIN_12V", PortSide::Left, PORT_PITCH_MM),
            ("VOUT_3V3", PortSide::Right, PORT_PITCH_MM),
        ]
    );
    assert!(d.fault().is_none());
}

#[test]
fn generate_lays_one_depth_out_in_one_row() {
    let d = generated(&three_sheets(), &mut SequentialIds::default());
    let ys: Vec<f64> = d.blocks.iter().map(|b| b.position.y).collect();
    assert!(ys.iter().all(|y| *y == ys[0]));
    let xs: Vec<f64> = d.blocks.iter().map(|b| b.position.x).collect();
    assert!(xs[0] < xs[1] && xs[1] < xs[2]);
}

#[test]
fn generate_joins_ports_and_records_the_group() {
    let d = generated(&three_sheets(), &mut SequentialIds::default());
    let buck = block_named(&d, "DC/DC Buck");
    let mcu = block_named(&d, "MCU");
    let link = d
        .connections
        .iter()
        .find(|c| c.derived_from.as_deref() == Some("VOUT_3V3"))
        .unwrap();
    assert_eq!(link.from.block, buck.id);
    assert_eq!(link.to.block, mcu.id);
    assert_eq!(link.from.port, Some(buck.ports[1].id));
    assert_eq!(link.to.port, Some(mcu.ports[0].id));
    assert_eq!(link.interface, Interface::Power);
    assert_eq!(d.connections.len(), 2);
}

#[test]
fn regenerate_with_nothing_new_changes_nothing() {
    let mut ids = SequentialIds::default();
    let d = generated(&three_sheets(), &mut ids);
    let again = regenerate_block_diagram(&d, &three_sheets(), &mut ids).unwrap();
    assert!(again.changes.is_empty(), "{:?}", again.changes);
    assert_eq!(again.diagram, d);
}

#[test]
fn regenerate_after_adding_a_sheet_adds_one_block_and_keeps_positions() {
    let mut ids = SequentialIds::default();
    let mut d = generated(&three_sheets(), &mut ids);
    // The person moved and renamed a block, and drew one of their own.
    let buck = block_named(&d, "DC/DC Buck").id;
    let patch = BlockPatch {
        position: Some(Point::new(300.0, 120.0)),
        name: Some("Buck".into()),
        ..BlockPatch::default()
    };
    d.set_block(buck, patch).unwrap();
    let manual = d
        .add_block(
            &mut ids,
            BlockFields::named("Notes", Point::new(0.0, 200.0)),
        )
        .unwrap();
    let before: Vec<(Uuid, Point)> = d.blocks.iter().map(|b| (b.id, b.position)).collect();

    let mut derived = three_sheets();
    derived
        .blocks
        .push(sheet(4, "connectors.snxsch", "Connectors", Vec::new()));
    let again = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();

    let added: Vec<&DiagramChange> = again
        .changes
        .iter()
        .filter(|c| matches!(c, DiagramChange::BlockAdded { .. }))
        .collect();
    assert_eq!(added.len(), 1);
    assert_eq!(again.changes.len(), 1, "{:?}", again.changes);
    assert_eq!(again.diagram.blocks.len(), before.len() + 1);
    for (id, at) in before {
        assert_eq!(again.diagram.block(id).unwrap().position, at);
    }
    assert_eq!(again.diagram.block(buck).unwrap().name, "Buck");
    assert!(again.diagram.block(manual).is_some());
    // The new block sits below every block that was there.
    let new = block_named(&again.diagram, "Connectors");
    let lowest = d
        .blocks
        .iter()
        .map(|b| b.position.y + b.height_mm)
        .fold(0.0, f64::max);
    assert!(new.position.y > lowest);
}

#[test]
fn a_renamed_sheet_moves_its_block_link() {
    let mut ids = SequentialIds::default();
    let d = generated(&three_sheets(), &mut ids);
    let mut derived = three_sheets();
    derived.blocks[1].path = "power/buck.snxsch".into();
    let again = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();
    let buck = block_named(&again.diagram, "DC/DC Buck");
    assert_eq!(buck.sheet.as_ref().unwrap().path, "power/buck.snxsch");
    assert_eq!(
        again.changes,
        [DiagramChange::SheetMoved {
            block: buck.id,
            sheet: doc(2),
            from: "dcdc.snxsch".into(),
            to: "power/buck.snxsch".into(),
        }]
    );
}

#[test]
fn a_deleted_sheet_leaves_its_block_unlinked_and_drops_its_generated_connections() {
    let mut ids = SequentialIds::default();
    let d = generated(&three_sheets(), &mut ids);
    let mut derived = three_sheets();
    derived.blocks.remove(2);
    derived.connections.remove(1);
    let again = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();
    let mcu = block_named(&again.diagram, "MCU");
    assert_eq!(mcu.sheet.as_ref().unwrap().state, LinkState::Unlinked);
    assert_eq!(mcu.ports.len(), 1, "an unlinked block keeps its ports");
    assert!(again.diagram.connections_of(mcu.id).next().is_none());
    assert!(again.changes.iter().any(|c| matches!(
        c,
        DiagramChange::SheetRemoved { sheet, .. } if *sheet == doc(3)
    )));
    assert!(again.changes.iter().any(|c| matches!(
        c,
        DiagramChange::ConnectionRemoved { derived_from, .. } if derived_from == "VOUT_3V3"
    )));

    // The sheet comes back: the same block is linked again.
    let back = regenerate_block_diagram(&again.diagram, &three_sheets(), &mut ids).unwrap();
    let mcu_again = block_named(&back.diagram, "MCU");
    assert_eq!(mcu_again.id, mcu.id);
    assert_eq!(mcu_again.sheet.as_ref().unwrap().state, LinkState::Linked);
    assert_eq!(back.diagram.blocks.len(), 3);
    assert_eq!(back.diagram.connections.len(), 2);
}

#[test]
fn a_hand_drawn_connection_survives_and_leaves_a_lost_port_for_the_edge() {
    let mut ids = SequentialIds::default();
    let mut d = generated(&three_sheets(), &mut ids);
    let power = block_named(&d, "Power In").clone();
    let mcu = block_named(&d, "MCU").clone();
    let from = Endpoint {
        block: power.id,
        port: Some(power.ports[0].id),
    };
    let to = Endpoint {
        block: mcu.id,
        port: None,
    };
    let manual = d
        .connect(&mut ids, from, to, Interface::Signal, "sense")
        .unwrap();
    let mut derived = three_sheets();
    derived.blocks[0].ports.clear();
    derived.connections.remove(0);
    let again = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();
    let kept = again.diagram.connection(manual).unwrap();
    assert_eq!(kept.from.port, None);
    assert_eq!(kept.label, "sense");
    assert!(again.changes.iter().any(|c| matches!(
        c,
        DiagramChange::PortRemoved { name, .. } if name == "VIN_12V"
    )));
}

#[test]
fn a_changed_group_updates_its_connection_in_place() {
    let mut ids = SequentialIds::default();
    let d = generated(&three_sheets(), &mut ids);
    let before = d.connections[1].id;
    let mut derived = three_sheets();
    derived.connections[1].label = "VOUT_3V3 3.30 V".into();
    let again = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();
    assert_eq!(again.diagram.connections[1].id, before);
    assert_eq!(again.diagram.connections[1].label, "VOUT_3V3 3.30 V");
    assert!(matches!(
        again.changes.as_slice(),
        [DiagramChange::ConnectionChanged { connection, .. }] if *connection == before
    ));
}

#[test]
fn a_new_entry_adds_a_port_below_the_others_and_grows_the_block() {
    let mut ids = SequentialIds::default();
    let d = generated(&three_sheets(), &mut ids);
    let mut derived = three_sheets();
    for n in 0..6 {
        let name = format!("GPIO{n}");
        derived.blocks[2]
            .ports
            .push(port(&name, PortDirection::Input));
    }
    let again = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();
    let mcu = block_named(&again.diagram, "MCU");
    let offsets: Vec<f64> = mcu.ports.iter().map(|p| p.offset_mm).collect();
    assert_eq!(offsets, [5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0]);
    assert!(mcu.height_mm >= 35.0 + PORT_PITCH_MM);
    let added = again
        .changes
        .iter()
        .filter(|c| matches!(c, DiagramChange::PortAdded { .. }))
        .count();
    assert_eq!(added, 6);
}

#[test]
fn the_change_list_serializes_with_its_kind() {
    let change = DiagramChange::SheetMoved {
        block: Uuid::from_u128(1),
        sheet: doc(2),
        from: "a.snxsch".into(),
        to: "b.snxsch".into(),
    };
    let value = serde_json::to_value(&change).unwrap();
    assert_eq!(value["change"], "sheet_moved");
    assert_eq!(value["from"], "a.snxsch");
    assert_eq!(value["sheet"], doc(2).to_string());
}

#[test]
fn a_generated_diagram_round_trips_through_the_file() {
    let d = generated(&three_sheets(), &mut SequentialIds::default());
    let text = encode_block_diagram(&d).unwrap();
    let read = parse_block_diagram(&text).unwrap();
    assert_eq!(read.diagram, d);
}
