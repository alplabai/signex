//! Placed-symbol body style (#1233): the alternate (De Morgan) body rides in
//! `[extras.symbols.<uuid>]` as `body_style = 2` only when chosen, so a sheet
//! that never uses it writes the bytes it always did.

use super::*;
use crate::schematic::{BodyStyle, SchematicSheet, Symbol};
use uuid::Uuid;

fn sheet(body_style: BodyStyle) -> SchematicSheet {
    SchematicSheet {
        symbols: vec![Symbol {
            uuid: Uuid::from_u128(7),
            reference: "U1".into(),
            body_style,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn write(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn read(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

#[test]
fn a_normal_symbol_writes_no_body_key_and_reloads_byte_identical() {
    let text = write(&sheet(BodyStyle::Normal));
    assert!(!text.contains("body_style"), "{text}");
    assert_eq!(write(&read(&text)), text);
}

#[test]
fn an_alternate_only_symbol_is_not_elided_and_round_trips() {
    // Everything else on the symbol is default, so only `body_style` keeps its
    // extras table from being dropped as redundant.
    let text = write(&sheet(BodyStyle::Alternate));
    assert!(text.contains("body_style = 2"), "{text}");
    let back = read(&text);
    assert_eq!(back.symbols[0].body_style, BodyStyle::Alternate);
    assert_eq!(write(&back), text);
}

#[test]
fn an_out_of_range_body_style_loads_normal() {
    for bad in ["0", "3", "99"] {
        let text = write(&sheet(BodyStyle::Alternate))
            .replace("body_style = 2", &format!("body_style = {bad}"));
        assert_eq!(
            read(&text).symbols[0].body_style,
            BodyStyle::Normal,
            "body_style = {bad}"
        );
    }
}

#[test]
fn json_without_the_field_loads_normal() {
    let mut json = serde_json::to_value(sheet(BodyStyle::Alternate).symbols[0].clone()).unwrap();
    json.as_object_mut().unwrap().remove("body_style");
    let symbol: Symbol = serde_json::from_value(json).unwrap();
    assert_eq!(symbol.body_style, BodyStyle::Normal);
}

#[test]
fn json_round_trips_the_alternate() {
    let json = serde_json::to_string(&sheet(BodyStyle::Alternate).symbols[0]).unwrap();
    let symbol: Symbol = serde_json::from_str(&json).unwrap();
    assert_eq!(symbol.body_style, BodyStyle::Alternate);
}
