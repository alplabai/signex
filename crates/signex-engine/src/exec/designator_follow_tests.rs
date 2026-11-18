//! #1986 review: a designator edit renames the field expressions that name
//! the part, in the same undo step.

use crate::test_support::test_sheet;
use crate::*;
use signex_types::schematic::Symbol;

fn part(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        ..Default::default()
    }
}

fn set_reference(engine: &mut Engine, symbol: uuid::Uuid, reference: &str) {
    engine
        .execute(Command::UpdateText {
            target: TextTarget::SymbolReference(symbol),
            value: reference.to_string(),
        })
        .expect("designator edit");
}

#[test]
fn a_designator_edit_renames_the_references_in_one_undo_step() {
    let mut sheet = test_sheet();
    let source = part("R1", "1kΩ");
    let source_id = source.uuid;
    sheet.symbols.push(source);
    sheet.symbols.push(part("R2", "= R1.Value * 2"));
    let mut engine = Engine::new(sheet).expect("engine");

    set_reference(&mut engine, source_id, "R10");
    assert_eq!(engine.document().symbols[1].value, "= R10.Value * 2");

    engine.undo().expect("undo");
    assert_eq!(engine.document().symbols[0].reference, "R1");
    assert_eq!(engine.document().symbols[1].value, "= R1.Value * 2");
}

#[test]
fn a_swap_by_hand_keeps_each_reference_on_its_part() {
    let mut sheet = test_sheet();
    let a = part("R1", "1kΩ");
    let b = part("R2", "2kΩ");
    let (a_id, b_id) = (a.uuid, b.uuid);
    sheet.symbols.extend([a, b, part("R3", "= R1.Value + 2 * R2.Value")]);
    let mut engine = Engine::new(sheet).expect("engine");

    // R1 -> R9, R2 -> R1, R9 -> R2: the second step passes through a name
    // the first part no longer has.
    set_reference(&mut engine, a_id, "R9");
    set_reference(&mut engine, b_id, "R1");
    set_reference(&mut engine, a_id, "R2");
    assert_eq!(
        engine.document().symbols[2].value,
        "= R2.Value + 2 * R1.Value"
    );
}
