//! `Command::SetSymbolFootprintBinding` (#1234): the name, the stamp and one
//! undo step.

use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::schematic::{RowStamp, Symbol};
use uuid::Uuid;

const ROW: u128 = 9;

fn engine_with_part() -> (Engine, Uuid) {
    let mut sheet = test_sheet();
    let symbol = Uuid::now_v7();
    sheet.symbols.push(Symbol {
        uuid: symbol,
        lib_id: "L:S".into(),
        reference: "R1".into(),
        footprint: "Chip-0805".into(),
        row_id: Some(Uuid::from_u128(ROW)),
        library_version: "1".into(),
        library_stamp: Some(RowStamp {
            footprint: Some("Chip-0805".into()),
            version: Some("1".into()),
            ..RowStamp::new(Uuid::from_u128(ROW))
        }),
        ..Default::default()
    });
    (Engine::new(sheet).unwrap(), symbol)
}

fn switch(symbol_id: Uuid, name: &str, binding: Option<&str>) -> Command {
    Command::SetSymbolFootprintBinding {
        symbol_id,
        footprint: name.into(),
        binding: binding.map(str::to_owned),
    }
}

#[test]
fn switching_the_footprint_sets_name_and_stamp_and_one_undo_restores_both() {
    let (mut engine, id) = engine_with_part();
    let before = engine.document().symbols[0].clone();

    let result = engine
        .execute(switch(id, "Chip-0402", Some("a:b")))
        .unwrap();
    assert!(result.changed);
    let part = &engine.document().symbols[0];
    assert_eq!(part.footprint, "Chip-0402");
    let stamp = part.valid_stamp().expect("stamp stays valid");
    assert_eq!(stamp.footprint.as_deref(), Some("Chip-0402"));
    assert_eq!(stamp.footprint_ref.as_deref(), Some("a:b"));

    engine.undo().unwrap();
    assert_eq!(engine.document().symbols[0], before);
    engine.redo().unwrap();
    assert_eq!(engine.document().symbols[0].footprint, "Chip-0402");
}

#[test]
fn choosing_what_the_part_already_has_changes_nothing() {
    let (mut engine, id) = engine_with_part();
    let result = engine.execute(switch(id, "Chip-0805", None)).unwrap();
    assert!(!result.changed);
    assert!(!engine.can_undo());
}
