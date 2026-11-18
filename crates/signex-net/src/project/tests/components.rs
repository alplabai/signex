//! The netlist by component (#1274): parts with the net on each pin.
use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn key(name: &str) -> SheetKey {
    SheetKey::new(name)
}

/// One sheet under `root.sch`, indexed.
fn indexed(root: &SchematicSheet) -> NetIndex {
    let sheets: HashMap<SheetKey, SchematicSheet> =
        HashMap::from([(key("root.sch"), root.clone())]);
    let resolved = HashMap::from([(key("root.sch"), HashMap::new())]);
    let roots = [ProjectRoot {
        key: key("root.sch"),
    }];
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    build_project_netlist_indexed(&graph).1
}

/// Place `reference` with `uuid`, value and footprint.
fn part(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, at: Point, uuid: u128) {
    place(sheet, reference, lib_id, at);
    let s = sheet.symbols.last_mut().unwrap();
    s.uuid = id(uuid);
    s.value = format!("{reference} value");
    s.footprint = format!("{reference} footprint");
}

/// `R1` and `R2` on the `VCC` wire, `U1` in two units (pins 1 and 2) on
/// the `SIG` wire, `C1` wired to nothing, two unannotated `R?`, and a power
/// symbol on `VCC`.
fn design() -> SchematicSheet {
    let mut s = empty_sheet();
    add_lib(&mut s, "R");
    add_lib(&mut s, "U");
    let mut second = s.lib_symbols["U"].clone();
    second.id = "U2".to_string();
    second.pins = vec![lib_pin("2", pt(0.0, 0.0))];
    s.lib_symbols.insert("U2".to_string(), second);

    s.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    s.labels.push(label("VCC", pt(0.0, 0.0), LabelType::Global));
    part(&mut s, "R2", "R", pt(5.0, 0.0), 2);
    part(&mut s, "R1", "R", pt(0.0, 0.0), 1);
    place_power(&mut s, "#PWR1", "P", "VCC", pt(0.0, 0.0));
    s.symbols.last_mut().unwrap().uuid = id(9);

    s.wires.push(wire(pt(0.0, 20.0), pt(5.0, 20.0)));
    s.labels.push(label("SIG", pt(0.0, 20.0), LabelType::Global));
    part(&mut s, "U1", "U2", pt(5.0, 20.0), 4);
    part(&mut s, "U1", "U", pt(0.0, 20.0), 3);

    part(&mut s, "C1", "R", pt(50.0, 50.0), 5);
    part(&mut s, "R?", "R", pt(60.0, 50.0), 6);
    part(&mut s, "R?", "R", pt(70.0, 50.0), 7);
    s
}

fn pins(report: &ComponentReport) -> Vec<(&str, &str)> {
    report
        .pins
        .iter()
        .map(|p| (p.pin.as_str(), p.net_name.as_str()))
        .collect()
}

#[test]
fn every_part_is_listed_by_designator_with_the_net_on_each_pin() {
    let parts = indexed(&design()).components();
    let designators: Vec<&str> = parts.iter().map(|p| p.designator.as_str()).collect();
    assert_eq!(designators, ["C1", "R1", "R2", "R?", "R?", "U1"]);

    let r1 = &parts[1];
    assert_eq!(r1.value, "R1 value");
    assert_eq!(r1.footprint, "R1 footprint");
    assert_eq!(pins(r1), [("1", "VCC")]);
    assert_eq!(pins(&parts[2]), [("1", "VCC")]);
}

#[test]
fn the_units_of_one_designator_are_one_part() {
    let parts = indexed(&design()).components();
    let u1 = parts.iter().find(|p| p.designator == "U1").expect("U1");
    assert_eq!(pins(u1), [("1", "SIG"), ("2", "SIG")]);
    assert_eq!(u1.anchor.id, id(4), "the unit placed first");
}

#[test]
fn a_pin_wired_to_nothing_is_not_listed_and_power_symbols_are_no_parts() {
    let parts = indexed(&design()).components();
    let c1 = parts.iter().find(|p| p.designator == "C1").expect("C1");
    assert!(c1.pins.is_empty());
    assert!(parts.iter().all(|p| p.designator != "#PWR1"));
    let unannotated: Vec<Uuid> = parts
        .iter()
        .filter(|p| p.designator == "R?")
        .map(|p| p.anchor.id)
        .collect();
    assert_eq!(unannotated, [id(6), id(7)], "one part per symbol");
}

#[test]
fn a_pin_anchor_finds_its_net_again() {
    let index = indexed(&design());
    let parts = index.components();
    let pin = &parts[1].pins[0];
    assert_eq!(pin.anchor.pin.as_deref(), Some("1"));
    assert_eq!(index.resolve(&pin.net_name, &pin.anchor), Some(pin.net));
}
