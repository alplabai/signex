//! `Command::RegisterLibSymbol` (#364): insert-if-absent, and one undo
//! step together with the `PlaceSymbol` that references it.

use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::schematic::{LibSymbol, Symbol};

fn lib_symbol(id: &str) -> LibSymbol {
    LibSymbol {
        id: id.to_string(),
        reference: "U".into(),
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
        pins: Vec::new(),
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn placement(id: &str) -> [Command; 2] {
    [
        Command::RegisterLibSymbol {
            lib_symbol: lib_symbol(id),
        },
        Command::PlaceSymbol {
            symbol: Symbol {
                uuid: uuid::Uuid::now_v7(),
                lib_id: id.to_string(),
                ..Default::default()
            },
        },
    ]
}

#[test]
fn registering_an_existing_key_is_unchanged_and_records_nothing() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine
        .execute(Command::RegisterLibSymbol {
            lib_symbol: lib_symbol("L:S"),
        })
        .unwrap();
    assert!(engine.can_undo());

    let again = engine
        .execute(Command::RegisterLibSymbol {
            lib_symbol: lib_symbol("L:S"),
        })
        .unwrap();
    assert!(!again.changed);

    // Only the first registration is on the undo stack.
    engine.undo().unwrap();
    assert!(!engine.document().lib_symbols.contains_key("L:S"));
    assert!(!engine.can_undo());
}

#[test]
fn undo_of_a_batch_that_reuses_an_entry_keeps_the_entry() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine.execute_batch(placement("L:S")).unwrap();
    let symbols = engine.document().symbols.len();

    engine.execute_batch(placement("L:S")).unwrap();
    assert_eq!(engine.document().symbols.len(), symbols + 1);
    assert_eq!(engine.document().lib_symbols.len(), 1);

    engine.undo().unwrap();
    assert_eq!(engine.document().symbols.len(), symbols);
    assert!(engine.document().lib_symbols.contains_key("L:S"));
}

#[test]
fn undo_then_redo_of_a_new_registration_restores_both() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let symbols = engine.document().symbols.len();
    engine.execute_batch(placement("L:S")).unwrap();

    engine.undo().unwrap();
    assert_eq!(engine.document().symbols.len(), symbols);
    assert!(engine.document().lib_symbols.is_empty());

    engine.redo().unwrap();
    assert_eq!(engine.document().symbols.len(), symbols + 1);
    assert!(engine.document().lib_symbols.contains_key("L:S"));
}
