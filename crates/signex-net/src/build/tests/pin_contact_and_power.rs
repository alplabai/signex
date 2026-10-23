//! Issue #675 — same-value power ports joining by name, and the pin-to-pin
//! policy (ADR-0002 D5.1): pins that touch tip to tip with no wire between them
//! are NOT connected.

use super::*;

/// A one-pin library symbol with its tip at the local origin.
fn add_one_pin(sheet: &mut SchematicSheet, lib_id: &str, dir: PinDirection) {
    add_lib(sheet, lib_id, vec![lib_pin("1", pt(0.0, 0.0), dir)]);
}

fn net_named<'a>(netlist: &'a Netlist, name: &str) -> Option<&'a Net> {
    netlist.nets.iter().find(|n| n.name == name)
}

#[test]
fn same_value_power_ports_on_disjoint_wires_join_into_one_net() {
    let mut sheet = empty_sheet();
    add_one_pin(&mut sheet, "SRC", PinDirection::PowerOutput);
    add_one_pin(&mut sheet, "LOAD", PinDirection::PowerInput);
    add_one_pin(&mut sheet, "GND", PinDirection::PowerInput);
    // Wire A carries U1.1 and a GND port; wire B carries U2.1 and a GND port.
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place(&mut sheet, "U1", "SRC", pt(0.0, 0.0));
    place_power(&mut sheet, "#PWR1", "GND", "GND", pt(10.0, 0.0));
    place(&mut sheet, "U2", "LOAD", pt(0.0, 20.0));
    place_power(&mut sheet, "#PWR2", "GND", "GND", pt(10.0, 20.0));

    let netlist = build_netlist(&sheet);

    assert_eq!(
        netlist.nets.len(),
        1,
        "one GND net, not GND + GND_<k>: {netlist:?}"
    );
    let gnd = net_named(&netlist, "GND").expect("net named GND");
    let refs: Vec<&str> = gnd.terminals.iter().map(|t| t.reference.as_str()).collect();
    assert!(refs.contains(&"U1") && refs.contains(&"U2"), "{refs:?}");
}

#[test]
fn different_value_power_ports_stay_separate() {
    let mut sheet = empty_sheet();
    add_one_pin(&mut sheet, "P", PinDirection::PowerInput);
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place_power(&mut sheet, "#PWR1", "P", "GND", pt(0.0, 0.0));
    place_power(&mut sheet, "#PWR2", "P", "VCC", pt(0.0, 20.0));

    let netlist = build_netlist(&sheet);

    assert_eq!(netlist.nets.len(), 2, "{netlist:?}");
    assert!(net_named(&netlist, "GND").is_some());
    assert!(net_named(&netlist, "VCC").is_some());
}

#[test]
fn power_port_dropped_on_a_component_pin_does_not_connect_without_a_wire() {
    let mut sheet = empty_sheet();
    add_one_pin(&mut sheet, "C", PinDirection::Passive);
    add_one_pin(&mut sheet, "GND", PinDirection::PowerInput);
    place(&mut sheet, "C1", "C", pt(10.0, 20.0));
    place_power(&mut sheet, "#PWR1", "GND", "GND", pt(10.0, 20.0));

    let netlist = build_netlist(&sheet);

    assert!(
        netlist
            .nets
            .iter()
            .all(|n| n.terminals.iter().all(|t| t.reference != "C1")),
        "pin-to-pin contact is not a connection: {netlist:?}"
    );
}

#[test]
fn two_parts_butted_together_form_no_net() {
    let mut sheet = empty_sheet();
    add_one_pin(&mut sheet, "R", PinDirection::Passive);
    place(&mut sheet, "R1", "R", pt(5.0, 5.0));
    place(&mut sheet, "R2", "R", pt(5.0, 5.0));

    assert!(build_netlist(&sheet).nets.is_empty());
}

#[test]
fn stacked_pins_of_one_symbol_do_not_form_a_net() {
    // Two pins of the same part on one point are not a connection between two
    // parts; with nothing else touching, no net exists.
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "STACK",
        vec![
            lib_pin("1", pt(0.0, 0.0), PinDirection::Passive),
            lib_pin("2", pt(0.0, 0.0), PinDirection::Passive),
        ],
    );
    place(&mut sheet, "U1", "STACK", pt(5.0, 5.0));

    assert!(build_netlist(&sheet).nets.is_empty());
}

#[test]
fn touching_different_value_power_ports_do_not_short_two_rails() {
    // GND and VCC ports share one wireless tip; each rail also has a wired
    // load. The shared point is only contact, so the rails must stay apart.
    let mut sheet = empty_sheet();
    add_one_pin(&mut sheet, "LOAD", PinDirection::PowerInput);
    add_one_pin(&mut sheet, "P", PinDirection::PowerInput);
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place(&mut sheet, "U1", "LOAD", pt(0.0, 0.0));
    place_power(&mut sheet, "#PWR1", "P", "GND", pt(10.0, 0.0));
    place(&mut sheet, "U2", "LOAD", pt(0.0, 20.0));
    place_power(&mut sheet, "#PWR2", "P", "VCC", pt(10.0, 20.0));
    place_power(&mut sheet, "#PWR3", "P", "GND", pt(50.0, 50.0));
    place_power(&mut sheet, "#PWR4", "P", "VCC", pt(50.0, 50.0));

    let netlist = build_netlist(&sheet);

    assert_eq!(netlist.nets.len(), 2, "{netlist:?}");
    let gnd = net_named(&netlist, "GND").expect("GND net");
    let vcc = net_named(&netlist, "VCC").expect("VCC net");
    assert!(gnd.terminals.iter().all(|t| t.reference != "U2"), "{gnd:?}");
    assert!(vcc.terminals.iter().all(|t| t.reference != "U1"), "{vcc:?}");
}

#[test]
fn a_junction_under_touching_ports_anchors_them_into_one_net() {
    // A junction is an anchor, so GND and VCC ports sharing its point are
    // genuinely connected: the short is real and the nets reflect it — one
    // net, never a phantom third net beside the two rails.
    let mut sheet = empty_sheet();
    add_one_pin(&mut sheet, "LOAD", PinDirection::PowerInput);
    add_one_pin(&mut sheet, "P", PinDirection::PowerInput);
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place(&mut sheet, "U1", "LOAD", pt(0.0, 0.0));
    place_power(&mut sheet, "#PWR1", "P", "GND", pt(10.0, 0.0));
    place(&mut sheet, "U2", "LOAD", pt(0.0, 20.0));
    place_power(&mut sheet, "#PWR2", "P", "VCC", pt(10.0, 20.0));
    place_power(&mut sheet, "#PWR3", "P", "GND", pt(50.0, 50.0));
    place_power(&mut sheet, "#PWR4", "P", "VCC", pt(50.0, 50.0));
    sheet.junctions.push(junction(pt(50.0, 50.0)));

    let netlist = build_netlist(&sheet);

    assert_eq!(netlist.nets.len(), 1, "{netlist:?}");
    let refs: Vec<&str> = netlist.nets[0]
        .terminals
        .iter()
        .map(|t| t.reference.as_str())
        .collect();
    assert!(refs.contains(&"U1") && refs.contains(&"U2"), "{refs:?}");
}
