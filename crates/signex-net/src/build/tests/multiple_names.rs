//! #1043 — a net with several names: which one the netlist shows, and that the
//! others stay aliases that still join nets. Rule: `docs/NET_NAMING.md`.

use super::*;

/// A sheet with one wire `(0,y)-(10,y)` per entry of `ys`, each carrying a
/// one-pin resistor at its left end.
fn wires_with_resistors(ys: &[f64]) -> SchematicSheet {
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    for (i, &y) in ys.iter().enumerate() {
        sheet.wires.push(wire(pt(0.0, y), pt(10.0, y)));
        place(&mut sheet, &format!("R{}", i + 1), "R", pt(0.0, y));
    }
    sheet
}

fn names(netlist: &Netlist) -> Vec<&str> {
    netlist.nets.iter().map(|n| n.name.as_str()).collect()
}

#[test]
fn global_beats_local_and_the_local_alias_still_joins() {
    // Wire 0 carries local `SDA` and global `I2C_SDA`; wire 1 only local `SDA`.
    let mut sheet = wires_with_resistors(&[0.0, 20.0]);
    sheet
        .labels
        .push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    sheet
        .labels
        .push(label("I2C_SDA", pt(10.0, 0.0), LabelType::Global));
    sheet
        .labels
        .push(label("SDA", pt(0.0, 20.0), LabelType::Net));

    let netlist = build_netlist(&sheet);

    assert_eq!(names(&netlist), ["I2C_SDA"], "{netlist:?}");
    assert_eq!(
        netlist.nets[0].terminals.len(),
        2,
        "alias SDA joined wire 1"
    );
}

#[test]
fn power_beats_global_whatever_the_text() {
    let mut sheet = wires_with_resistors(&[0.0]);
    sheet
        .labels
        .push(label("AAA", pt(0.0, 0.0), LabelType::Global));
    sheet
        .labels
        .push(label("ZZZ", pt(10.0, 0.0), LabelType::Power));

    assert_eq!(names(&build_netlist(&sheet)), ["ZZZ"]);
}

#[test]
fn hierarchical_beats_local() {
    let mut sheet = wires_with_resistors(&[0.0]);
    sheet
        .labels
        .push(label("AAA", pt(0.0, 0.0), LabelType::Net));
    sheet
        .labels
        .push(label("ZZZ", pt(10.0, 0.0), LabelType::Hierarchical));

    assert_eq!(names(&build_netlist(&sheet)), ["ZZZ"]);
}

#[test]
fn equal_priority_names_pick_the_smallest_text_in_any_order() {
    for flip in [false, true] {
        let mut sheet = wires_with_resistors(&[0.0]);
        let mut pair = [
            label("VDD", pt(0.0, 0.0), LabelType::Global),
            label("+3V3", pt(10.0, 0.0), LabelType::Global),
        ];
        if flip {
            pair.reverse();
        }
        sheet.labels.extend(pair);

        assert_eq!(names(&build_netlist(&sheet)), ["+3V3"], "flip={flip}");
    }
}

#[test]
fn two_power_ports_with_different_values_on_one_wire_get_one_name() {
    // Ports `VDD` and `+3V3` at the two ends of one wire; a second `VDD` port
    // on a separate wire joins by value, so `VDD` stays an alias.
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "P",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::PowerInput)],
    );
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    place_power(&mut sheet, "#PWR1", "P", "VDD", pt(0.0, 0.0));
    place_power(&mut sheet, "#PWR2", "P", "+3V3", pt(10.0, 0.0));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    sheet.wires.push(wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place(&mut sheet, "R1", "R", pt(0.0, 20.0));
    place_power(&mut sheet, "#PWR3", "P", "VDD", pt(10.0, 20.0));

    let netlist = build_netlist(&sheet);

    assert_eq!(names(&netlist), ["+3V3"], "{netlist:?}");
    let refs: Vec<&str> = netlist.nets[0]
        .terminals
        .iter()
        .map(|t| t.reference.as_str())
        .collect();
    assert!(refs.contains(&"R1"), "R1 joined via alias VDD: {refs:?}");
}
