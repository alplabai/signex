use super::*;
use crate::{Command, Engine};
use signex_types::schematic::{
    HAlign, InstancePath, Label, LabelType, Point, SchematicSheet, Symbol, SymbolInstance, VAlign,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn placement(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![id(100), id(n)])
}

fn sheet() -> SchematicSheet {
    let entry = |p: &InstancePath, r: &str| SymbolInstance {
        project: "board".into(),
        path: p.to_string(),
        reference: r.into(),
        unit: 1,
    };
    SchematicSheet {
        labels: vec![Label {
            unknown_columns: Default::default(),
            uuid: id(1),
            text: "GND_RAIL".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: "input".to_string(),
            font_size: 1.27,
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
            font_family: None,
        }],
        symbols: vec![Symbol {
            uuid: id(2),
            reference: "R1".into(),
            value: "10k".into(),
            unit: 1,
            instances: vec![entry(&placement(1), "R1"), entry(&placement(2), "R101")],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn apply(sheet: SchematicSheet, commands: Vec<Command>) -> SchematicSheet {
    let mut engine = Engine::new(sheet).unwrap();
    engine.execute_batch(commands).unwrap();
    engine.document().clone()
}

#[test]
fn an_empty_needle_finds_nothing() {
    assert!(scan_sheet(&sheet(), "  ", &DesignatorScope::default()).is_empty());
}

#[test]
fn scan_finds_label_designator_and_value_case_insensitively() {
    let hits = scan_sheet(&sheet(), "r", &DesignatorScope::default());
    let kinds: Vec<_> = hits.iter().map(|h| h.kind).collect();
    assert_eq!(kinds, [FindKind::NetLabel, FindKind::Designator]);
}

#[test]
fn a_twice_placed_sheet_yields_a_designator_hit_per_placement() {
    let scope = DesignatorScope {
        placements: vec![placement(1), placement(2)],
        single: false,
    };
    let texts: Vec<_> = scan_sheet(&sheet(), "r1", &scope)
        .into_iter()
        .map(|h| (h.placement.unwrap(), h.text))
        .collect();
    assert_eq!(
        texts,
        [
            (placement(1), "R1".to_string()),
            (placement(2), "R101".to_string())
        ]
    );
}

#[test]
fn replace_recomputes_from_the_document_not_the_hit_text() {
    let mut hits = scan_sheet(&sheet(), "rail", &DesignatorScope::default());
    hits[0].text = "STALE_RAIL".into();
    let commands = replace_commands(&sheet(), &hits[0], "rail", "BUS");
    let out = apply(sheet(), commands);
    assert_eq!(out.labels[0].text, "GND_BUS");
}

#[test]
fn replace_is_empty_once_the_needle_is_gone() {
    let hits = scan_sheet(&sheet(), "rail", &DesignatorScope::default());
    let mut edited = sheet();
    edited.labels[0].text = "VCC".into();
    assert!(replace_commands(&edited, &hits[0], "rail", "BUS").is_empty());
}

#[test]
fn a_placement_designator_edit_changes_only_that_placement() {
    let scope = DesignatorScope {
        placements: vec![placement(1), placement(2)],
        single: false,
    };
    let hit = scan_sheet(&sheet(), "r101", &scope).remove(0);
    let out = apply(sheet(), replace_commands(&sheet(), &hit, "r101", "R7"));
    let symbol = &out.symbols[0];
    assert_eq!(symbol.designator_in(&placement(2)), "R7");
    assert_eq!(symbol.designator_in(&placement(1)), "R1");
    assert_eq!(symbol.reference, "R1");
}

#[test]
fn a_single_placement_edit_writes_reference_and_drops_the_entry() {
    let scope = DesignatorScope::at(placement(2), true);
    let hit = scan_sheet(&sheet(), "r101", &scope).remove(0);
    let out = apply(sheet(), replace_commands(&sheet(), &hit, "r101", "R7"));
    assert_eq!(out.symbols[0].reference, "R7");
    assert!(!out.symbols[0].has_instance_entry_in(&placement(2)));
}

#[test]
fn replace_all_ci_matches_the_old_app_behaviour() {
    assert_eq!(replace_all_ci("GND_RAIL", "r", "X"), "GND_XAIL");
    assert_eq!(replace_all_ci("R1_r2_R3", "r", "net"), "net1_net2_net3");
    assert_eq!(replace_all_ci("VCC", "", "X"), "VCC");
    assert_eq!(replace_all_ci("A_B", "_", "$1"), "A$1B");
}

#[test]
fn non_ascii_case_folds_the_same_way_in_scan_and_replace() {
    let mut s = sheet();
    s.labels[0].text = "\u{DC}NIT_\u{FC}nit".into();
    let hits = scan_sheet(&s, "\u{FC}nit", &DesignatorScope::default());
    assert_eq!(hits.len(), 1, "the scan finds it");
    let out = apply(s.clone(), replace_commands(&s, &hits[0], "\u{FC}nit", "X"));
    assert_eq!(out.labels[0].text, "X_X", "and the replace rewrites both");
    assert_eq!(replace_all_ci("STRASSE", "\u{DF}", "x"), "STRASSE");
}
