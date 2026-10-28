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
fn an_out_of_range_body_style_loads_normal_and_a_re_save_keeps_the_value() {
    for bad in ["0", "3", "99"] {
        let text = write(&sheet(BodyStyle::Alternate))
            .replace("body_style = 2", &format!("body_style = {bad}"));
        let back = read(&text);
        assert_eq!(back.symbols[0].body_style, BodyStyle::Normal, "{bad}");
        let again = write(&back);
        assert_eq!(
            again.matches("body_style").count(),
            1,
            "kept once, not duplicated:\n{again}"
        );
        assert!(again.contains(&format!("body_style = {bad}")), "{again}");
    }
}

#[test]
fn choosing_a_body_replaces_a_kept_unreadable_value() {
    let text = write(&sheet(BodyStyle::Alternate)).replace("body_style = 2", "body_style = 3");
    let mut back = read(&text);
    back.symbols[0].unknown.remove("body_style");
    back.symbols[0].body_style = BodyStyle::Alternate;
    let again = write(&back);
    assert_eq!(again.matches("body_style").count(), 1, "{again}");
    assert!(again.contains("body_style = 2"), "{again}");
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

#[test]
fn a_non_integer_or_negative_body_style_keeps_the_sheet_loadable() {
    for bad in ["-1", "\"2\"", "2.5", "true"] {
        let text = write(&sheet(BodyStyle::Alternate))
            .replace("body_style = 2", &format!("body_style = {bad}"));
        let back = read(&text);
        assert_eq!(back.symbols[0].body_style, BodyStyle::Normal, "{bad}");
        let again = write(&back);
        assert_eq!(again.matches("body_style").count(), 1, "{bad}:\n{again}");
        assert!(
            again.contains(&format!("body_style = {bad}")),
            "{bad}:\n{again}"
        );
    }
}
