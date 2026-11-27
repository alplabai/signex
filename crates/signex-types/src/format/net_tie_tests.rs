//! A placed net tie (#1277): `net_tie = true` rides in
//! `[extras.symbols.<uuid>]` only when set, and needs no newer token.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{SchematicSheet, Symbol};

fn part(n: u128, net_tie: bool) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(n),
        reference: format!("NT{n}"),
        net_tie,
        ..Default::default()
    }
}

fn sheet_of(symbols: Vec<Symbol>) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.symbols = symbols;
    sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

#[test]
fn a_net_tie_survives_save_and_reload_byte_identical() {
    let text = save(&sheet_of(vec![part(1, true), part(2, false)]));
    assert!(text.contains("net_tie = true"), "{text}");
    let back = load(&text);
    assert!(back.symbols[0].net_tie);
    assert!(!back.symbols[1].net_tie);
    assert_eq!(save(&back), text, "a second save is byte-identical");
}

#[test]
fn a_symbol_whose_only_extra_is_the_tie_is_not_elided() {
    let text = save(&sheet_of(vec![part(1, true)]));
    assert!(load(&text).symbols[0].net_tie, "{text}");
}

#[test]
fn a_sheet_with_no_tie_writes_no_key() {
    let text = save(&sheet_of(vec![part(1, false), part(2, false)]));
    assert!(!text.contains("net_tie"), "{text}");
}

/// An older build reads the key as an unknown key of the symbol's table and
/// writes it back, so the tie needs no newer token.
#[test]
fn a_net_tie_keeps_the_lowest_schematic_token() {
    let text = save(&sheet_of(vec![part(1, true)]));
    assert!(
        text.starts_with(&format!("format = \"{SNXSCH_FORMAT_V1}\"")),
        "{text}"
    );
}
