//! #1313: the power topology is read off the stitched netlist: power ports
//! name the rails, a part's units are one part, grounds are no rail, a rail
//! parameter names a rail on its own, and a part that is not fitted or not
//! on a rail is left out.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use signex_types::project::ChannelNaming;
use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use super::*;
use crate::fixtures::multi_channel::{add_lib, place};
use crate::fixtures::{empty_sheet, pt, wire};
use crate::{ProjectGraph, ProjectRoot, SheetKey};

pub(super) const TOP: &str = "top.snxsch";
pub(super) const SHEET_ID: Uuid = Uuid::from_u128(0x5ee7);

/// A wire from `(0, y)` to `(10, y)`, the part unit `reference` at its left
/// end and a power port `rail` at its right end.
fn on_rail(s: &mut SchematicSheet, id: u128, reference: &str, unit: u32, rail: &str, y: f64) {
    s.wires.push(wire(pt(0.0, y), pt(10.0, y)));
    place(s, id, reference, "P", pt(0.0, y));
    s.symbols.last_mut().expect("placed").unit = unit;
    place(s, id + 0x1000, "#PWR", "PWR", pt(10.0, y));
    let port = s.symbols.last_mut().expect("placed");
    port.is_power = true;
    port.value = rail.to_string();
}

fn set(s: &mut SchematicSheet, reference: &str, unit: u32, fields: &[(&str, &str)]) {
    let symbol = s
        .symbols
        .iter_mut()
        .find(|sym| sym.reference == reference && sym.unit == unit)
        .expect("the part");
    for (key, value) in fields {
        symbol.fields.insert(key.to_string(), value.to_string());
    }
}

/// The mockup row 171 buck in one sheet: U1 (two units) from VIN_12V to
/// +3V3 at 88 %, U2 drawing 640 mA from +3V3, U3 naming a +5V load with no
/// pin on it, U4 on +3V3 but not fitted, R9 on GND only.
pub(super) fn buck_sheet() -> SchematicSheet {
    let mut s = empty_sheet();
    s.uuid = SHEET_ID;
    add_lib(&mut s, "P");
    add_lib(&mut s, "PWR");
    on_rail(&mut s, 0x10, "U1", 1, "VIN_12V", 0.0);
    on_rail(&mut s, 0x11, "U1", 2, "+3V3", 10.0);
    on_rail(&mut s, 0x12, "U2", 1, "+3V3", 20.0);
    on_rail(&mut s, 0x13, "R9", 1, "GND", 30.0);
    on_rail(&mut s, 0x14, "U4", 1, "+3V3", 40.0);
    place(&mut s, 0x15, "U3", "P", pt(50.0, 50.0));
    set(&mut s, "U1", 1, &[("Rail Supply +3V3", "3A")]);
    set(&mut s, "U1", 2, &[("Efficiency", "88 %")]);
    set(&mut s, "U2", 1, &[("Rail Load +3V3", "640mA")]);
    set(&mut s, "U3", 1, &[("Rail Load +5V", "10 mA")]);
    set(&mut s, "U4", 1, &[("Rail Load +3V3", "1A")]);
    s
}

/// The topology of a one-sheet project made of `sheet`, U4 not fitted.
pub(super) fn topology_of(sheet: SchematicSheet, listed: &BTreeSet<String>) -> PowerTopology {
    let key = SheetKey::new(TOP);
    let sheets = HashMap::from([(key.clone(), sheet)]);
    let resolved = HashMap::from([(key.clone(), HashMap::new())]);
    let roots = vec![ProjectRoot { key }];
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    let fitted = |units: &[&Symbol]| units.iter().all(|unit| unit.reference != "U4");
    power_topology(&graph, &ChannelNaming::default(), listed, &fitted)
}

fn names(set: &BTreeSet<String>) -> Vec<&str> {
    set.iter().map(String::as_str).collect()
}

#[test]
fn power_ports_and_rail_parameters_name_the_rails_and_grounds_are_kept_apart() {
    let topology = topology_of(buck_sheet(), &BTreeSet::new());
    assert_eq!(names(&topology.rails), ["+3V3", "+5V", "VIN_12V"]);
    assert_eq!(names(&topology.grounds), ["GND"]);
}

#[test]
fn a_parts_units_are_one_part_and_only_fitted_parts_on_a_rail_are_kept() {
    let topology = topology_of(buck_sheet(), &BTreeSet::new());
    let designators: Vec<&str> = topology
        .parts
        .iter()
        .map(|part| part.designator.as_str())
        .collect();
    assert_eq!(
        designators,
        ["U1", "U2", "U3"],
        "U4 is not fitted, R9 is on GND"
    );
    let u1 = &topology.parts[0];
    assert_eq!(names(&u1.pins_on), ["+3V3", "VIN_12V"]);
    assert_eq!(u1.units, [Uuid::from_u128(0x10), Uuid::from_u128(0x11)]);
    assert_eq!(u1.sheet_id, SHEET_ID);
    let fields: BTreeMap<&str, &str> = u1
        .parameters
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        fields,
        BTreeMap::from([("Efficiency", "88 %"), ("Rail Supply +3V3", "3A")])
    );
    let u3 = &topology.parts[2];
    assert!(u3.pins_on.is_empty(), "U3 is on no wire");
    assert!(u3.has_rail_parameter());
}

#[test]
fn a_rail_the_project_lists_counts_without_a_power_port_and_a_listed_ground_does_not() {
    let listed = BTreeSet::from(["+1V8".to_string(), "AGND".to_string()]);
    let topology = topology_of(buck_sheet(), &listed);
    assert_eq!(names(&topology.rails), ["+1V8", "+3V3", "+5V", "VIN_12V"]);
}
