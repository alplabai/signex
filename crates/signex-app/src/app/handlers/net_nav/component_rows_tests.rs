//! The By Component rows and the By Net pin names, from one stitched index:
//! `U1` (TPS62130, QFN-16) has pin 14 `VOS` on `VOUT_3V3` with `C7` pin 1,
//! and eight more pins on no net.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::{Point, SchematicSheet, Symbol};

use super::*;
use crate::app::handlers::menu::export::tests::sheet_with_net;
use crate::app::handlers::net_nav::inspector::net_rows;
use crate::panels::net_inspector::{NetInspectorTab, NetMemberRowKind};

/// The sheet: `U1` at the origin with its pin 14 on the wire carrying the
/// global label `VOUT_3V3`, `C7` at the wire's other end.
fn sheet() -> SchematicSheet {
    let mut sheet = sheet_with_net("U1", "VOUT_3V3", &[]);
    let mut capacitor = sheet.lib_symbols["Device:R"].clone();
    capacitor.id = "Device:C".into();
    capacitor.reference = "C".into();
    let lib = sheet
        .lib_symbols
        .get_mut("Device:R")
        .expect("the part's symbol");
    let template = lib.pins[0].clone();
    lib.pins[0].pin.number = "14".into();
    lib.pins[0].pin.name = "VOS".into();
    let spare = ["1", "2", "3", "4", "5", "6", "7", "9"];
    for (k, number) in spare.iter().enumerate() {
        let mut pin = template.clone();
        pin.pin.number = (*number).into();
        pin.pin.name = if *number == "9" {
            "SS/TR".into()
        } else {
            format!("P{number}")
        };
        pin.pin.position = Point::new(0.0, 20.0 + 10.0 * k as f64);
        lib.pins.push(pin);
    }
    sheet.lib_symbols.insert("Device:C".into(), capacitor);
    let u1 = &mut sheet.symbols[0];
    u1.value = "TPS62130".into();
    u1.footprint = "QFN-16".into();
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:C".into(),
        reference: "C7".into(),
        value: "22µ".into(),
        footprint: "CAP_0805".into(),
        position: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet
}

fn data() -> ProjectNetData {
    let dir = Path::new("/proj");
    let top = dir.join("top.snxsch");
    let sheets: HashMap<PathBuf, SchematicSheet> = [(top.clone(), sheet())].into_iter().collect();
    let graph = crate::app::project_sheets::project_graph(&sheets, Some(dir), Some(&top));
    let roots = [signex_net::ProjectRoot {
        key: crate::app::project_sheets::sheet_key(&top, Some(dir)),
    }];
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let (_, index) = signex_net::build_project_netlist_indexed(&project);
    ProjectNetData {
        index,
        paths: graph.paths.clone(),
        base_dir: Some(dir.to_path_buf()),
        fold: Default::default(),
        classes: Vec::new(),
        cross_refs: Default::default(),
    }
}

fn by_component(expanded: &[&str]) -> NetInspectorState {
    NetInspectorState {
        tab: NetInspectorTab::ByComponent,
        expanded_parts: expanded.iter().map(|s| s.to_string()).collect(),
        ..NetInspectorState::default()
    }
}

#[test]
fn parts_list_by_designator_with_value_footprint_and_pin_count() {
    let rows = component_rows(&data(), &by_component(&[]), None).0;
    let listed: Vec<(&str, &str, &str, usize)> = rows
        .iter()
        .map(|p| {
            (
                p.designator.as_str(),
                p.value.as_str(),
                p.footprint.as_str(),
                p.pins,
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("C7", "22µ", "CAP_0805", 1),
            ("U1", "TPS62130", "QFN-16", 9)
        ]
    );
    assert!(rows.iter().all(|p| !p.expanded && p.pin_rows.is_empty()));
}

/// The rows sort by the designator rule of `signex_types::designator`, so
/// numbers compare by value: `C2` before `C10`, pin `9` before `14`.
#[test]
fn designators_and_pin_numbers_sort_by_their_numbers() {
    let mut names = vec!["C10", "R1", "C2", "c3", "U1", "C1"];
    names.sort_by(|a, b| compare_references(a, b));
    assert_eq!(names, ["C1", "C2", "c3", "C10", "R1", "U1"]);
    assert_eq!(compare_references("14", "9"), std::cmp::Ordering::Greater);
}

/// An expanded part lists six pins in number order, then "+ 3 more"; shown
/// in full it lists all nine, pin 14 `VOS` on `VOUT_3V3` and the rest on no
/// net.
#[test]
fn an_expanded_part_lists_its_pins_with_net_and_name_six_at_first() {
    let data = data();
    let mut state = by_component(&["U1"]);
    let u1 = component_rows(&data, &state, None).0.remove(1);
    let numbers: Vec<&str> = u1.pin_rows.iter().map(|p| p.pin.as_str()).collect();
    assert_eq!(numbers, ["1", "2", "3", "4", "5", "6"]);
    assert_eq!(u1.more, 3);
    assert!(u1.pin_rows.iter().all(|p| p.net.is_none()), "unconnected");

    state.show_all("U1".into());
    let u1 = component_rows(&data, &state, None).0.remove(1);
    assert_eq!(u1.more, 0);
    let listed: Vec<(&str, Option<&str>, &str)> = u1
        .pin_rows
        .iter()
        .map(|p| (p.pin.as_str(), p.net.as_deref(), p.name.as_str()))
        .collect();
    assert_eq!(listed.len(), 9);
    assert_eq!(listed[7], ("9", None, "SS/TR"));
    assert_eq!(listed[8], ("14", Some("VOUT_3V3"), "VOS"));
}

/// By Net names U1's pin 14 the same way: `U1.14`, pin name `VOS`, on the
/// net By Component lists it on.
#[test]
fn by_net_and_by_component_agree_on_a_pins_net_and_name() {
    let data = data();
    let state = NetInspectorState {
        expanded: ["VOUT_3V3".to_string()].into_iter().collect(),
        ..NetInspectorState::default()
    };
    let nets = net_rows(&data, &state);
    let vout = nets.iter().find(|n| n.name == "VOUT_3V3").expect("the net");
    let pin = vout
        .members
        .iter()
        .find(|m| m.kind == NetMemberRowKind::Pin && m.text == "U1.14")
        .expect("U1 pin 14");
    assert_eq!(pin.pin_name, "VOS");

    let mut state = by_component(&["U1"]);
    state.show_all("U1".into());
    let u1 = component_rows(&data, &state, None).0.remove(1);
    let fourteen = u1.pin_rows.iter().find(|p| p.pin == "14").expect("pin 14");
    assert_eq!(fourteen.net.as_deref(), Some(vout.name.as_str()));
    assert_eq!(fourteen.anchor, pin.anchor, "both lead to the same pin");
}

#[test]
fn the_filter_matches_designator_value_or_footprint() {
    let data = data();
    let filtered = |text: &str| -> Vec<String> {
        let state = NetInspectorState {
            filter: text.into(),
            ..by_component(&[])
        };
        component_rows(&data, &state, None)
            .0
            .into_iter()
            .map(|p| p.designator)
            .collect()
    };
    assert_eq!(filtered("cap_0805"), ["C7"]);
    assert_eq!(filtered("tps"), ["U1"]);
    assert_eq!(filtered(" u1 "), ["U1"]);
    assert!(filtered("VREF").is_empty());
    assert_eq!(
        component_rows(&data, &by_component(&[]), None).1,
        2,
        "the total still counts both"
    );
    assert_eq!(data.index.component_count(), 2);
}
