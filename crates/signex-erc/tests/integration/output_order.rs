//! #1364: ERC output order must not depend on hash iteration order.

use signex_erc::{RuleKind, Violation};
use signex_types::schematic::{
    HAlign, Label, LabelType, LibSymbol, Point, SchematicSheet, Symbol, VAlign, Wire,
};
use uuid::Uuid;

fn duplicated_refs_sheet() -> SchematicSheet {
    let mut symbols = Vec::new();
    for i in (1..=12).rev() {
        for n in 0..2 {
            symbols.push(Symbol {
                uuid: Uuid::new_v4(),
                lib_id: if n == 0 { "Device:R" } else { "Device:C" }.to_string(),
                reference: format!("R{i}"),
                unit: 1,
                position: Point::new(10.0 + f64::from(n) * 5.0, 10.0 + f64::from(i)),
                ..Default::default()
            });
        }
    }
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    };
    for id in ["Device:R", "Device:C"] {
        sheet.lib_symbols.insert(
            id.to_string(),
            LibSymbol {
                id: id.to_string(),
                reference: "R".to_string(),
                value: id.to_string(),
                footprint: String::new(),
                datasheet: String::new(),
                description: String::new(),
                keywords: String::new(),
                fp_filters: String::new(),
                in_bom: true,
                on_board: true,
                in_pos_files: true,
                duplicate_pin_numbers_are_jumpers: false,
                graphics: Vec::new(),
                pins: Vec::new(),
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
    }
    sheet
}

fn conflicting_nets_sheet() -> SchematicSheet {
    let label = |text: &str, pos: Point| Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: pos,
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        font_family: None,
        locked: false,
    };
    let mut wires = Vec::new();
    let mut labels = Vec::new();
    for i in 0..12 {
        let y = 10.0 + f64::from(i) * 5.0;
        wires.push(Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            ..Default::default()
        });
        labels.push(label(&format!("A{i}"), Point::new(1.0, y)));
        labels.push(label(&format!("B{i}"), Point::new(9.0, y)));
    }
    SchematicSheet {
        paper_size: "A4".to_string(),
        wires,
        labels,
        ..Default::default()
    }
}

fn messages_of(sheet: &SchematicSheet, rule: RuleKind) -> Vec<String> {
    signex_erc::run(sheet)
        .iter()
        .filter(|v: &&Violation| v.rule == rule)
        .map(|v| v.message.clone())
        .collect()
}

#[test]
fn duplicate_designator_order_is_stable_across_runs() {
    let sheet = duplicated_refs_sheet();
    let first = messages_of(&sheet, RuleKind::DuplicateRefDesignator);
    assert!(
        first.len() >= 12,
        "fixture must produce duplicates: {first:?}"
    );
    for _ in 0..30 {
        assert_eq!(first, messages_of(&sheet, RuleKind::DuplicateRefDesignator));
    }
}

#[test]
fn net_label_conflict_order_is_stable_across_runs() {
    let sheet = conflicting_nets_sheet();
    let first = messages_of(&sheet, RuleKind::NetLabelConflict);
    assert!(
        first.len() >= 12,
        "fixture must produce conflicts: {first:?}"
    );
    for _ in 0..30 {
        assert_eq!(first, messages_of(&sheet, RuleKind::NetLabelConflict));
    }
}
