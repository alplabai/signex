//! Mockup row 171's totals line: sources active in the Max load scenario
//! and the rail with the least headroom.

use std::collections::{BTreeMap, BTreeSet};

use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::*;
use crate::SheetKey;

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

fn tree(parts: Vec<PowerPart>) -> PowerTree {
    let topology = PowerTopology {
        rails: ["VIN_12V", "VOUT_3V3", "VBUS"]
            .iter()
            .map(|net| net.to_string())
            .collect(),
        grounds: BTreeSet::new(),
        parts,
    };
    power_tree(&topology, &BTreeMap::new())
}

#[test]
fn an_alternate_source_with_no_load_is_not_active_and_the_tightest_rail_is_named() {
    let tree = tree(vec![
        part("J1", &["VIN_12V"], &[("Rail Supply VIN_12V", "1.5A")]),
        part("J2", &["VBUS"], &[("Rail Supply VBUS", "1A")]),
        part(
            "U1",
            &["VIN_12V", "VOUT_3V3"],
            &[
                ("Rail Load VIN_12V", "200mA"),
                ("Rail Supply VOUT_3V3", "3A"),
            ],
        ),
        part("U3", &["VOUT_3V3"], &[("Rail Load VOUT_3V3", "2.4A")]),
    ]);
    let summary = scenario_summary(&tree);
    assert_eq!(summary.sources, 2, "J1 and J2; U1 is a converter");
    assert_eq!(summary.active_sources, 1, "VBUS feeds nothing");
    let (rail, share) = summary.worst_rail.expect("rails with limits");
    assert_eq!(rail, "VOUT_3V3");
    assert!((share - 0.2).abs() < 1e-9, "{share}");
}

#[test]
fn a_tree_without_limits_has_no_worst_rail() {
    let tree = tree(vec![part(
        "U3",
        &["VOUT_3V3"],
        &[("Rail Load VOUT_3V3", "1A")],
    )]);
    let summary = scenario_summary(&tree);
    assert_eq!(summary.worst_rail, None);
    assert_eq!((summary.sources, summary.active_sources), (0, 0));
}
