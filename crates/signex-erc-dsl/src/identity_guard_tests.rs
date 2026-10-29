//! #1285 identity guard for DSL rules: a net rule and a pin rule over one
//! sheet produce several findings, and no two of one rule may share a waiver
//! identity (or one waiver would hide both).

use signex_erc::waivers::violation::waiver_key;
use signex_types::schematic::{
    HAlign, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, Symbol, VAlign, Wire,
};
use uuid::Uuid;

fn pin(number: &str, y: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(0.0, y),
            rotation: 0.0,
            length: 2.54,
            name: String::new(),
            number: number.into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn label(text: &str, at: Point) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: Uuid::now_v7(),
        text: text.into(),
        position: at,
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::default(),
        justify_v: VAlign::default(),
        font_family: None,
        locked: false,
    }
}

/// Two resistors with two unconnected pins each (one pair sharing a number,
/// as an unnumbered library can), and two labelled wire nets N1, N2.
fn sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::from_u128(1),
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.lib_symbols.insert(
        "Device:R".into(),
        LibSymbol {
            id: "Device:R".into(),
            reference: "R".into(),
            value: String::new(),
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
            pins: vec![pin("1", 3.81), pin("1", -3.81)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
    for (i, reference) in ["R1", "R2"].into_iter().enumerate() {
        sheet.symbols.push(Symbol {
            uuid: Uuid::from_u128(10 + i as u128),
            lib_id: "Device:R".into(),
            reference: reference.into(),
            position: Point::new(25.4 + 25.4 * i as f64, 50.8),
            ..Default::default()
        });
    }
    for (i, name) in ["N1", "N2"].into_iter().enumerate() {
        let y = 101.6 + 12.7 * i as f64;
        sheet.wires.push(Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(25.4, y),
            end: Point::new(50.8, y),
            ..Default::default()
        });
        sheet.labels.push(label(name, Point::new(25.4, y)));
    }
    sheet
}

#[test]
fn dsl_net_and_pin_findings_each_get_their_own_identity() {
    let rules = crate::parse_validate_compile_to_eval_fns(
        "rule nets on net when net.name matches \"^N\" then error \"net\"\n\
         rule pins on pin when pin.connected == false then warning \"pin\"",
    )
    .expect("rules compile");
    let sheet = sheet();
    let found: Vec<signex_erc::Violation> = signex_erc::run_with_dsl(&sheet, &rules)
        .into_iter()
        .filter(|v| v.builtin_kind().is_none())
        .collect();
    let count = |id: &str| found.iter().filter(|v| v.rule_id.to_string() == id).count();
    assert_eq!(count("user::nets"), 2, "{found:?}");
    assert_eq!(count("user::pins"), 4, "{found:?}");
    let mut ids: Vec<(String, Vec<String>)> = found
        .iter()
        .map(|v| {
            let key = waiver_key(v, &sheet).expect("waivable");
            (key.rule_id, key.objects)
        })
        .collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "{found:?}");
}
