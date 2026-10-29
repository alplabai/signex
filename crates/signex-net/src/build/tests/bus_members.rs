//! Buses carry members but never merge them (ADR-0002 D5.4).
use super::*;
use signex_types::schematic::Bus;

/// Vertical bus named `D[0..7]`, `count` entries at y = 5, 10, ... each
/// feeding a wire that ends on pin `R<i>.1`; wire `i` carries `labels[i]`.
fn bus_sheet(labels: &[Option<&str>]) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
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
            uuid: Uuid::now_v7(),
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

    let a = analyse(&sheet);
    assert_eq!(a.groups.len(), 1);
    assert!(a.groups[0].carries("D3") && a.groups[0].carries("D4"));
    assert_eq!(a.entries.len(), 1);
    let label_index = a.entries[0].member_labels[0];
    assert_eq!(sheet.labels[label_index].text, "D3");
    assert!(
        a.groups[0].carries(&netlist.nets[0].name),
        "the net is a member"
    );
}

#[test]
fn bus_members_never_merge_with_each_other() {
    let sheet = bus_sheet(&[Some("D1"), Some("D2")]);
    let netlist = build_netlist(&sheet);
    let names: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names.len(), 2, "two members stay two nets: {names:?}");
    assert!(names.contains(&"D1") && names.contains(&"D2"));

    let a = analyse(&sheet);
    assert_eq!(a.entries.len(), 2);
    let carried: Vec<&str> = a
        .entries
        .iter()
        .map(|e| sheet.labels[e.member_labels[0]].text.as_str())
        .collect();
    assert_eq!(carried, ["D1", "D2"]);
    assert!(a.groups[0].carries("D1") && a.groups[0].carries("D2"));
}

#[test]
fn an_unlabeled_entry_wire_is_its_own_unnamed_net_not_a_member() {
    let sheet = bus_sheet(&[None]);
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    assert!(netlist.nets[0].name.starts_with("N$"));
    let a = analyse(&sheet);
    assert!(a.entries[0].member_labels.is_empty());
    assert!(
        a.groups[0].carries("D3"),
        "the bus still carries its members"
    );
}

#[test]
fn naming_a_bus_adds_no_net() {
    let mut sheet = bus_sheet(&[]);
    sheet
        .labels
        .push(label("D[0..7]", pt(0.0, 2.0), LabelType::Net));
    assert!(build_netlist(&sheet).nets.is_empty());
}

fn analyse(sheet: &SchematicSheet) -> crate::BusAnalysis {
    let buses: Vec<_> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
    let entries: Vec<_> = sheet
        .bus_entries
        .iter()
        .map(|e| (e.position, e.size))
        .collect();
    let wires: Vec<_> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    let junctions: Vec<_> = sheet.junctions.iter().map(|j| j.position).collect();
    let labels: Vec<_> = sheet
        .labels
        .iter()
        .map(|l| (l.position, l.text.as_str()))
        .collect();
    crate::analyse_buses(&crate::BusGeometry {
        buses: &buses,
        entries: &entries,
        wires: &wires,
        junctions: &junctions,
        labels: &labels,
    })
}

#[test]
fn a_bus_name_label_on_a_wire_still_names_and_joins_nets_as_before() {
    // Whether a bus-name label may name a net is decided with cross-sheet
    // buses (#1049). Until then connectivity is exactly what it was before
    // buses had members: the label names its net, and two wires carrying the
    // same bus-name label are one net. ERC warns about it (`BusLabelOnWire`).
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    for (i, y) in [0.0, 30.0].into_iter().enumerate() {
        sheet.wires.push(wire(pt(0.0, y), pt(10.0, y)));
        place(&mut sheet, &format!("R{}", i + 1), "R", pt(10.0, y));
        sheet
            .labels
            .push(label("D[0..7]", pt(5.0, y), LabelType::Net));
    }
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1, "same-name labels still join");
    assert_eq!(netlist.nets[0].name, "D[0..7]");
    assert_eq!(netlist.nets[0].terminals.len(), 2);
}
