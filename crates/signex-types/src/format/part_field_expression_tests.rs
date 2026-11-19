//! Field expressions (#1986) are saved as their source text: a sheet with
//! `=` values and fields round-trips byte-identical.

use super::tests::{empty_sheet, sample_symbol};
use super::*;
use crate::schematic::SchematicSheet;

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

#[test]
fn field_expressions_round_trip_as_their_source() {
    let mut sheet = empty_sheet();
    let mut r1 = sample_symbol();
    r1.uuid = Uuid::from_u128(1);
    r1.reference = "R1".into();
    r1.value = "= Rbase * 2".into();
    r1.fields.insert("Rbase".into(), "4.7kΩ".into());
    let mut r2 = sample_symbol();
    r2.uuid = Uuid::from_u128(2);
    r2.reference = "R2".into();
    r2.value = "= R1.Value / 2\t==literal".into();
    sheet.symbols.push(r1);
    sheet.symbols.push(r2);

    let text = save(&sheet);
    assert!(!text.contains("9.4k"), "the value is never saved: {text}");
    let again = load(&text);
    assert_eq!(again.symbols[0].value, "= Rbase * 2");
    assert_eq!(again.symbols[1].value, "= R1.Value / 2\t==literal");
    assert_eq!(again.symbols[0].fields["Rbase"], "4.7kΩ");
    assert_eq!(save(&again), text, "a second save is byte-identical");
}
