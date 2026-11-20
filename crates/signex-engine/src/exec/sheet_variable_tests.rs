//! Sheet variables (#1987): one undo step each on the PAPER patch, the
//! source text stored as typed, a repeat changes nothing.

use crate::test_support::test_sheet;
use crate::*;

fn set(engine: &mut Engine, name: &str, value: Option<&str>) -> CommandResult {
    engine
        .execute(Command::SetSheetVariable {
            name: name.to_string(),
            value: value.map(str::to_string),
        })
        .expect("set sheet variable")
}

fn variable(engine: &Engine, name: &str) -> Option<String> {
    engine.document().variables.get(name).cloned()
}

#[test]
fn a_sheet_variable_sets_no_ops_removes_and_undoes() {
    let mut engine = Engine::new(test_sheet()).expect("engine");
    let result = set(&mut engine, "VREF", Some("= VCC / 2"));
    assert!(result.changed);
    assert_eq!(
        result.patch_pair.expect("patch").document,
        DocumentPatch::PAPER
    );
    // The source is stored, not an evaluated value.
    assert_eq!(variable(&engine, "VREF").as_deref(), Some("= VCC / 2"));

    assert!(!set(&mut engine, "VREF", Some("= VCC / 2")).changed);
    assert!(set(&mut engine, "VREF", None).changed);
    assert_eq!(variable(&engine, "VREF"), None);
    assert!(!set(&mut engine, "VREF", None).changed);

    engine.undo().expect("undo");
    assert_eq!(variable(&engine, "VREF").as_deref(), Some("= VCC / 2"));
    engine.undo().expect("undo");
    assert!(engine.document().variables.is_empty());
}

#[test]
fn an_empty_value_is_kept_as_a_variable() {
    let mut engine = Engine::new(test_sheet()).expect("engine");
    assert!(set(&mut engine, "NOTE", Some("")).changed);
    assert_eq!(variable(&engine, "NOTE").as_deref(), Some(""));
}
