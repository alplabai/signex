//! Two versions of one library symbol on one sheet (#2209): each instance
//! takes its pins from the body it draws with.

use super::*;

fn version(pins: Vec<LibPin>) -> LibSymbol {
    let mut sheet = empty_sheet();
    add_lib(&mut sheet, "R", pins);
    sheet.lib_symbols.values().next().unwrap().clone()
}

fn placed_with(body_ref: &str, reference: &str, origin: Point) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "R".into(),
        body_ref: body_ref.into(),
        reference: reference.into(),
        position: origin,
        ..Default::default()
    }
}

#[test]
fn each_instance_connects_through_the_pins_of_its_own_version() {
    let mut sheet = empty_sheet();
    // v1: pin "1" on the origin. v2: the pin was renumbered "A" and moved 5 mm.
    let v1 = sheet
        .embed_body(version(vec![lib_pin(
            "1",
            pt(0.0, 0.0),
            PinDirection::Passive,
        )]))
        .unwrap();
    let v2 = sheet
        .embed_body(version(vec![lib_pin(
            "A",
            pt(5.0, 0.0),
            PinDirection::Passive,
        )]))
        .unwrap();
    sheet.symbols.push(placed_with(&v1, "R1", pt(0.0, 0.0)));
    sheet.symbols.push(placed_with(&v2, "R2", pt(20.0, 0.0)));
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(25.0, 0.0), pt(30.0, 0.0)));

    let netlist = build_netlist(&sheet);
    let mut terminals: Vec<(&str, &str)> = netlist
        .nets
        .iter()
        .flat_map(|n| n.terminals.iter())
        .map(|t| (t.reference.as_str(), t.pin.as_str()))
        .collect();
    terminals.sort();
    assert_eq!(terminals, [("R1", "1"), ("R2", "A")]);
    assert_eq!(netlist.nets.len(), 2);
}
