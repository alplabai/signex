//! #691: units of one multi-unit part share a designator and are one part,
//! not a duplicate-designator error.

use signex_erc::{RuleKind, run};
use signex_types::schematic::{LibSymbol, SchematicSheet, Symbol};
use uuid::Uuid;

fn unit_symbol(reference: &str, lib_id: &str, unit: u32) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        unit,
        ..Default::default()
    }
}

fn lib_symbol(id: &str) -> LibSymbol {
    LibSymbol {
        id: id.to_string(),
        reference: "U".to_string(),
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
    }
}

fn duplicate_count(symbols: Vec<Symbol>) -> usize {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    };
    for id in [
        "Amplifier:LM358",
        "Logic:74HC00",
        "  Amplifier:LM358 ",
        "amplifier:lm358",
    ] {
        sheet.lib_symbols.insert(id.to_string(), lib_symbol(id));
    }
    run(&sheet)
        .iter()
        .filter(|v| v.rule == RuleKind::DuplicateRefDesignator)
        .count()
}

#[test]
fn two_units_of_one_part_are_not_a_duplicate_designator() {
    let symbols = vec![
        unit_symbol("U1", "Amplifier:LM358", 1),
        unit_symbol("U1", "Amplifier:LM358", 2),
    ];
    assert_eq!(duplicate_count(symbols), 0);
}

#[test]
fn the_same_unit_placed_twice_under_one_designator_is_still_a_duplicate() {
    let symbols = vec![
        unit_symbol("U1", "Amplifier:LM358", 1),
        unit_symbol("U1", "Amplifier:LM358", 1),
    ];
    assert_eq!(duplicate_count(symbols), 2);
}

#[test]
fn different_parts_sharing_a_designator_are_still_a_duplicate() {
    let symbols = vec![
        unit_symbol("U1", "Amplifier:LM358", 1),
        unit_symbol("U1", "Logic:74HC00", 2),
    ];
    assert_eq!(duplicate_count(symbols), 2);
}

fn duplicate_messages(symbols: Vec<Symbol>) -> Vec<String> {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    };
    for id in [
        "Amplifier:LM358",
        "Logic:74HC00",
        "  Amplifier:LM358 ",
        "amplifier:lm358",
    ] {
        sheet.lib_symbols.insert(id.to_string(), lib_symbol(id));
    }
    run(&sheet)
        .iter()
        .filter(|v| v.rule == RuleKind::DuplicateRefDesignator)
        .map(|v| v.message.clone())
        .collect()
}

#[test]
fn padded_library_ids_still_pair_as_one_part() {
    let symbols = vec![
        unit_symbol("U1", "Amplifier:LM358", 1),
        unit_symbol("U1", "  Amplifier:LM358 ", 2),
    ];
    assert_eq!(duplicate_count(symbols), 0);
}

#[test]
fn library_ids_that_differ_in_case_are_different_parts() {
    let symbols = vec![
        unit_symbol("U1", "Amplifier:LM358", 1),
        unit_symbol("U1", "amplifier:lm358", 2),
    ];
    assert_eq!(duplicate_count(symbols), 2);
}

#[test]
fn units_from_different_library_symbols_get_a_hint() {
    let messages = duplicate_messages(vec![
        unit_symbol("U1", "Amplifier:LM358", 1),
        unit_symbol("U1", "Logic:74HC00", 2),
    ]);
    assert!(!messages.is_empty());
    assert!(
        messages
            .iter()
            .all(|m| m.contains("units of U1 come from different library symbols")),
        "{messages:?}"
    );
}

#[test]
fn a_plain_duplicate_gets_no_library_hint() {
    let messages = duplicate_messages(vec![
        unit_symbol("U1", "Amplifier:LM358", 1),
        unit_symbol("U1", "Amplifier:LM358", 1),
    ]);
    assert!(!messages.is_empty());
    assert!(
        messages
            .iter()
            .all(|m| !m.contains("different library symbols"))
    );
}
