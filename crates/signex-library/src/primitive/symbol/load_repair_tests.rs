//! #3129: control characters in pin cells are refused at save, and a file a
//! build before #3129 wrote with one opens repaired.

use super::*;

/// A `.snxsym` written before #3129: pin 1's name is `OUT<CR>A`.
const PIN_NAME_CR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/snxsym/pin_name_cr.snxsym"
));

fn file_with_pin(name: &str, description: &str) -> SymbolFile {
    let mut pin = SymbolPin::new("7", name);
    pin.description = description.to_string();
    let mut symbol = Symbol::empty("OPAMP");
    symbol.pins.push(SymbolPin::new("1", "OUT"));
    symbol.pins.push(pin);
    SymbolFile::from_symbol(symbol)
}

fn refusal(file: &SymbolFile) -> (String, String, &'static str, char, String) {
    let error = match file.to_toml_string() {
        Err(error) => error,
        Ok(_) => panic!("expected the save to be refused"),
    };
    match &error {
        SymbolFileError::ControlCharInPinCell {
            symbol,
            pin,
            column,
            character,
        } => (
            symbol.clone(),
            pin.clone(),
            *column,
            *character,
            error.to_string(),
        ),
        other => panic!("expected ControlCharInPinCell, got {other:?}"),
    }
}

#[test]
fn a_save_refuses_cr_nul_and_del_naming_the_symbol_the_pin_and_the_character() {
    for (c, code) in [('\r', "U+000D"), ('\0', "U+0000"), ('\u{7f}', "U+007F")] {
        let (symbol, pin, column, character, text) =
            refusal(&file_with_pin(&format!("IN{c}B"), ""));
        assert_eq!((symbol.as_str(), pin.as_str()), ("OPAMP", "7"));
        assert_eq!((column, character), ("name", c));
        for part in ["\"OPAMP\"", "\"7\"", code] {
            assert!(text.contains(part), "{part} missing from {text}");
        }
    }
}

#[test]
fn a_mix_is_refused_at_its_first_character() {
    let (_, _, column, character, _) = refusal(&file_with_pin("A\u{1}B", "x\ry\0"));
    assert_eq!((column, character), ("name", '\u{1}'));
    let (_, _, column, character, _) = refusal(&file_with_pin("AB", "x\ry\0"));
    assert_eq!((column, character), ("description", '\r'));
}

#[test]
fn every_text_cell_is_checked_and_the_editor_check_agrees_with_the_save() {
    let mut file = file_with_pin("B", "");
    file.symbols[0].pins[1].function = vec!["SPI".into(), "MO\u{1b}SI".into()];
    let (_, _, column, character, _) = refusal(&file);
    assert_eq!((column, character), ("function", '\u{1b}'));
    assert_eq!(
        file.symbols[0].first_unstorable_pin_text(),
        Some(UnstorablePinText {
            pin_index: 1,
            column: "function",
            character: '\u{1b}',
        })
    );
    file.symbols[0].pins[1].function.clear();
    file.symbols[0].pins[1].number = "7\r".into();
    assert_eq!(
        file.symbols[0]
            .first_unstorable_pin_text()
            .map(|t| (t.column, t.character)),
        Some(("number", '\r'))
    );
    assert_eq!(
        file_with_pin("V\u{2212} \u{85}", "ok").symbols[0].first_unstorable_pin_text(),
        None,
        "text outside the ASCII controls is stored as it is"
    );
}

#[test]
fn the_fixture_does_not_open_strictly_and_says_where_the_character_is() {
    match SymbolFile::from_bytes(PIN_NAME_CR) {
        Err(e @ SymbolFileError::RepairableControlChars { .. }) => {
            let text = e.to_string();
            for part in ["\"OPAMP\"", "\"1\"", "name", "U+000D", "Symbol Editor"] {
                assert!(text.contains(part), "{part} missing from {text}");
            }
        }
        other => panic!("expected RepairableControlChars, got {other:?}"),
    }
}

#[test]
fn the_fixture_opens_repaired_and_reports_what_was_removed() {
    let (file, repairs) = SymbolFile::from_bytes_repairing(PIN_NAME_CR).expect("opens");
    let symbol = &file.symbols[0];
    assert_eq!(symbol.pins[0].name, "OUTA");
    assert_eq!(symbol.pins[1].name, "IN", "the other pin is untouched");
    assert_eq!(
        repairs,
        vec![SymbolLoadRepair {
            symbol: "OPAMP".into(),
            symbol_uuid: Some(symbol.uuid),
            pin: "1".into(),
            column: "name",
            character: '\r',
        }]
    );
    let saved = file.to_toml_string().expect("the repaired file saves");
    assert!(!saved.contains('\r'));
    let (reopened, again) = SymbolFile::from_bytes_repairing(saved.as_bytes()).expect("reopens");
    assert!(again.is_empty(), "nothing left to repair");
    assert_eq!(reopened, file);
}

#[test]
fn a_clean_file_reads_the_same_both_ways_and_round_trips_byte_for_byte() {
    let mut file = file_with_pin("IN\u{2212}", "tab-free text");
    file.symbols[0].pins[0].function = vec!["GPIO".into(), "ADC".into()];
    let text = file.to_toml_string().expect("serialise");
    let (repairing, repairs) = SymbolFile::from_bytes_repairing(text.as_bytes()).expect("read");
    assert!(repairs.is_empty());
    let strict = SymbolFile::from_bytes(text.as_bytes()).expect("strict read");
    assert_eq!(repairing, strict);
    assert_eq!(repairing.to_toml_string().expect("re-save"), text);
}

#[test]
fn a_control_character_in_a_second_symbol_names_that_symbol() {
    let mut file = file_with_pin("A", "");
    let mut second = Symbol::empty("SECOND");
    second.pins.push(SymbolPin::new("9", "Z"));
    file.symbols.push(second);
    let text = file.to_toml_string().expect("serialise");
    let broken = text.replacen("9\tZ\t", "9\tZ\u{7f}\t", 1);
    assert_ne!(broken, text, "the fixture edit applied");
    let (_, repairs) = SymbolFile::from_bytes_repairing(broken.as_bytes()).expect("opens");
    let named: Vec<_> = repairs
        .iter()
        .map(|r| (r.symbol.as_str(), r.pin.as_str(), r.column, r.character))
        .collect();
    assert_eq!(named, vec![("SECOND", "9", "name", '\u{7f}')]);
}
