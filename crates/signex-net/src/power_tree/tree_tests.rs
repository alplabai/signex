//! #1313: the power tree's budgets. The mockup row 171 buck draws 0.20 A
//! from VIN_12V for 0.64 A at 3.30 V and 88 %; missing data is a problem,
//! never a guess; a loop of converters ends.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use super::topology_tests::{buck_sheet, topology_of};
use super::*;
use crate::SheetKey;
use signex_types::schematic::InstancePath;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn buck_tree() -> (PowerTopology, PowerTree) {
    let topology = topology_of(buck_sheet(), &BTreeSet::new());
    let tree = power_tree(&topology, &BTreeMap::new());
    (topology, tree)
}

/// A part with `pins_on` and `fields`, alone on its own sheet.
fn part(designator: &str, pins_on: &[&str], fields: &[(&str, &str)]) -> PowerPart {
    PowerPart {
        designator: designator.into(),
        value: String::new(),
        sheet: SheetKey::new("a.snxsch"),
        sheet_id: Uuid::from_u128(1),
        occurrence: InstancePath::default(),
        units: vec![Uuid::now_v7()],
        pins_on: pins_on.iter().map(|net| net.to_string()).collect(),
        parameters: fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn topology(rails: &[&str], parts: Vec<PowerPart>) -> PowerTopology {
    PowerTopology {
        rails: rails.iter().map(|net| net.to_string()).collect(),
        grounds: BTreeSet::new(),
        parts,
    }
}

#[test]
fn the_buck_draws_its_output_power_over_its_efficiency_from_its_input() {
    let (topology, tree) = buck_tree();
    let out = tree.rail("+3V3").expect("+3V3");
    assert_eq!(
        out.voltage.map(|v| (v.volts, v.from)),
        Some((3.3, VoltageFrom::NetName))
    );
    assert_eq!(out.sources.len(), 1);
    assert_eq!(topology.parts[out.sources[0].part].designator, "U1");
    assert!(out.sources[0].is_converter);
    assert_eq!(out.limit_a(), Some(3.0));
    assert!(close(out.total_a, 0.64) && out.is_complete, "{out:?}");
    assert!(close(out.headroom_a().unwrap(), 2.36));
    assert_eq!(out.fed_from, BTreeSet::from(["VIN_12V".to_string()]));
    assert!(out.problems.is_empty(), "{:?}", out.problems);

    let input = tree.rail("VIN_12V").expect("VIN_12V");
    let load = &input.loads[0];
    assert_eq!(topology.parts[load.part].designator, "U1");
    assert_eq!(load.from, Some(CurrentFrom::Converter));
    // 3.3 V x 0.64 A = 2.112 W out; / 0.88 = 2.4 W in; / 12 V = 0.2 A.
    assert!(close(load.current_a.unwrap(), 0.2), "{load:?}");
    assert_eq!(input.problems, [PowerProblem::NoSource]);

    let converter = &tree.converters[0];
    assert!(close(converter.output_w.unwrap(), 2.112));
    assert!(close(converter.input_w.unwrap(), 2.4));
    assert!(close(converter.loss_w().unwrap(), 0.288));
    assert_eq!(converter.efficiency, Some(0.88));
}

#[test]
fn the_roots_are_the_rails_no_converter_feeds() {
    let (_, tree) = buck_tree();
    assert_eq!(tree.roots, ["+5V", "VIN_12V"]);
    let five = tree.rail("+5V").expect("+5V");
    assert!(close(five.total_a, 0.01));
    assert_eq!(five.problems, [PowerProblem::NoSource]);
}

#[test]
fn the_project_rail_list_beats_the_net_name_and_a_rail_without_either_says_so() {
    let parts = vec![part("U1", &["VBUS", "+3V3"], &[("Rail Load +3V3", "1")])];
    let listed = BTreeMap::from([(
        "+3V3".to_string(),
        ListedRail {
            voltage: 3.25,
            tolerance_percent: Some(1.0),
        },
    )]);
    let tree = power_tree(&topology(&["+3V3", "VBUS"], parts), &listed);
    let rail = tree.rail("+3V3").unwrap();
    let voltage = rail.voltage.unwrap();
    assert_eq!((voltage.volts, voltage.from), (3.25, VoltageFrom::Project));
    assert_eq!(voltage.tolerance_percent, Some(1.0));
    let vbus = tree.rail("VBUS").unwrap();
    assert_eq!(vbus.voltage, None);
    assert_eq!(
        vbus.problems,
        [
            PowerProblem::NoVoltage,
            PowerProblem::NoSource,
            PowerProblem::UnknownLoad {
                designator: "U1".into()
            }
        ]
    );
    assert!(!vbus.is_complete);
}

#[test]
fn several_sources_have_no_one_limit_and_an_unread_parameter_is_named() {
    let parts = vec![
        part("J1", &["+5V"], &[("Rail Supply +5V", "1A")]),
        part("J2", &["+5V"], &[("Rail Supply +5V", "lots")]),
        part("U1", &["+5V"], &[("Rail Load +5V", "-2")]),
    ];
    let tree = power_tree(&topology(&["+5V"], parts), &BTreeMap::new());
    let rail = tree.rail("+5V").unwrap();
    assert_eq!(rail.limit_a(), None);
    assert_eq!(rail.headroom_a(), None);
    assert_eq!(
        rail.problems,
        [
            PowerProblem::UnreadParameter {
                designator: "J2".into(),
                parameter: "Rail Supply +5V".into(),
                text: "lots".into()
            },
            PowerProblem::SeveralSources(2),
            PowerProblem::UnreadParameter {
                designator: "U1".into(),
                parameter: "Rail Load +5V".into(),
                text: "-2".into()
            },
        ]
    );
    assert!(tree.converters.is_empty(), "sources fed from nothing");
}

#[test]
fn a_converter_without_an_efficiency_draws_an_unknown_current_and_a_typed_one_wins() {
    let untold = vec![
        part("U1", &["VIN", "+3V3"], &[("Rail Supply +3V3", "1")]),
        part("U2", &["+3V3"], &[("Rail Load +3V3", "0.5")]),
    ];
    let tree = power_tree(&topology(&["+3V3", "VIN"], untold), &BTreeMap::new());
    let vin = tree.rail("VIN").unwrap();
    assert_eq!(vin.loads[0].current_a, None);
    assert!(
        vin.problems.contains(&PowerProblem::UnknownInput {
            designator: "U1".into()
        }),
        "{:?}",
        vin.problems
    );

    let typed = vec![
        part(
            "U1",
            &["VIN", "+3V3"],
            &[("Rail Supply +3V3", "1"), ("Rail Load VIN", "0.3")],
        ),
        part("U2", &["+3V3"], &[("Rail Load +3V3", "0.5")]),
    ];
    let tree = power_tree(&topology(&["+3V3", "VIN"], typed), &BTreeMap::new());
    let load = &tree.rail("VIN").unwrap().loads[0];
    assert_eq!(
        (load.current_a, load.from),
        (Some(0.3), Some(CurrentFrom::Parameter))
    );
}

#[test]
fn a_loop_of_converters_ends_with_unknown_currents() {
    let parts = vec![
        part(
            "U1",
            &["+5V", "+3V3"],
            &[("Rail Supply +3V3", "1"), ("Efficiency", "0.9")],
        ),
        part(
            "U2",
            &["+3V3", "+5V"],
            &[("Rail Supply +5V", "1"), ("Efficiency", "0.9")],
        ),
    ];
    let tree = power_tree(&topology(&["+3V3", "+5V"], parts), &BTreeMap::new());
    for rail in &tree.rails {
        assert!(!rail.is_complete, "{rail:?}");
    }
    assert!(tree.roots.is_empty(), "each rail is fed by the other");
}

#[test]
fn an_efficiency_that_is_no_fraction_is_named() {
    let parts = vec![
        part(
            "U1",
            &["VIN", "+3V3"],
            &[("Rail Supply +3V3", "1"), ("Efficiency", "88")],
        ),
        part("U2", &["+3V3"], &[("Rail Load +3V3", "0.5")]),
    ];
    let tree = power_tree(&topology(&["+3V3", "VIN"], parts), &BTreeMap::new());
    assert!(
        tree.rail("VIN")
            .unwrap()
            .problems
            .contains(&PowerProblem::UnreadParameter {
                designator: "U1".into(),
                parameter: "Efficiency".into(),
                text: "88".into()
            })
    );
    assert_eq!(tree.converters[0].efficiency, None);
}
