//! Buses carry members but never merge them (ADR-0002 D5.4).
use super::*;
use signex_types::schematic::Bus;

/// Vertical bus named `D[0..7]`, `count` entries at y = 5, 10, ... each
/// feeding a wire that ends on pin `R<i>.1`; wire `i` carries `labels[i]`.
fn bus_sheet(labels: &[Option<&str>]) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.buses.push(Bus {
        uuid: Uuid::new_v4(),
        start: pt(0.0, 0.0),
        end: pt(0.0, 40.0),
    });
    sheet
        .labels
        .push(label("D[0..7]", pt(0.0, 35.0), LabelType::Net));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    for (i, text) in labels.iter().enumerate() {
        let y = 5.0 + 10.0 * i as f64;
        sheet.bus_entries.push(signex_types::schematic::BusEntry {
            uuid: Uuid::new_v4(),
            position: pt(0.0, y),
            size: (2.54, 2.54),
        });
        sheet
            .wires
            .push(wire(pt(2.54, y + 2.54), pt(12.54, y + 2.54)));
        place(&mut sheet, &format!("R{}", i + 1), "R", pt(12.54, y + 2.54));
        if let Some(text) = text {
            sheet
                .labels
                .push(label(text, pt(6.0, y + 2.54), LabelType::Net));
        }
    }
    sheet
}

#[test]
fn an_entry_wire_joins_its_member_net_through_the_member_label() {
    let sheet = bus_sheet(&[Some("D3")]);
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    assert_eq!(netlist.nets[0].name, "D3");
    assert_eq!(netlist.nets[0].terminals.len(), 1);

    let buses = crate::sheet_buses(&sheet, &netlist);
    assert_eq!(buses.len(), 1);
    assert_eq!(buses[0].names, ["D[0..7]"]);
    let d3 = buses[0].members.iter().find(|m| m.name == "D3").unwrap();
    assert_eq!(d3.net, Some(netlist.nets[0].id));
    let d4 = buses[0].members.iter().find(|m| m.name == "D4").unwrap();
    assert_eq!(d4.net, None, "a member with no wire gets no minted net");
}

#[test]
fn bus_members_never_merge_with_each_other() {
    let sheet = bus_sheet(&[Some("D1"), Some("D2")]);
    let netlist = build_netlist(&sheet);
    let names: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names.len(), 2, "two members stay two nets: {names:?}");
    assert!(names.contains(&"D1") && names.contains(&"D2"));

    let buses = crate::sheet_buses(&sheet, &netlist);
    let net_of = |name: &str| {
        buses[0]
            .members
            .iter()
            .find(|m| m.name == name)
            .and_then(|m| m.net)
    };
    assert!(net_of("D1").is_some() && net_of("D2").is_some());
    assert_ne!(net_of("D1"), net_of("D2"));
}

#[test]
fn an_unlabeled_entry_wire_is_its_own_unnamed_net_not_a_member() {
    let sheet = bus_sheet(&[None]);
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    assert!(netlist.nets[0].name.starts_with("N$"));
    let buses = crate::sheet_buses(&sheet, &netlist);
    assert!(buses[0].members.iter().all(|m| m.net.is_none()));
}

#[test]
fn naming_a_bus_adds_no_net() {
    let mut sheet = bus_sheet(&[]);
    sheet
        .labels
        .push(label("D[0..7]", pt(0.0, 2.0), LabelType::Net));
    assert!(build_netlist(&sheet).nets.is_empty());
}
