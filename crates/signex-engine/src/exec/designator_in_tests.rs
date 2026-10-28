//! `Command::SetSymbolDesignatorIn` (#1151).

use crate::*;
use signex_types::schematic::{InstancePath, Symbol, SymbolInstance};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn placement(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![id(100), id(n)])
}

fn engine(instances: Vec<SymbolInstance>) -> Engine {
    let sheet = SchematicSheet {
        symbols: vec![Symbol {
            uuid: id(1),
            reference: "R1".into(),
            unit: 1,
            instances,
            ..Default::default()
        }],
        ..Default::default()
    };
    Engine::new(sheet).expect("engine")
}

fn entry(path: &InstancePath, reference: &str) -> SymbolInstance {
    SymbolInstance {
        project: "board".into(),
        path: path.to_string(),
        reference: reference.into(),
        unit: 1,
    }
}

fn set(engine: &mut Engine, at: &InstancePath, reference: &str) -> CommandResult {
    engine
        .execute(Command::SetSymbolDesignatorIn {
            symbol: id(1),
            placement: at.clone(),
            reference: reference.into(),
        })
        .expect("execute")
}

fn symbol(engine: &Engine) -> &Symbol {
    &engine.document().symbols[0]
}

#[test]
fn a_new_designator_adds_an_entry_for_that_placement_only() {
    let (a, b) = (placement(1), placement(2));
    let mut engine = engine(vec![entry(&a, "R1")]);

    let result = set(&mut engine, &b, " R101 ");
    assert!(result.changed);
    assert_eq!(result.patch_pair.unwrap().document, DocumentPatch::SYMBOLS);
    assert_eq!(symbol(&engine).designator_in(&b), "R101");
    assert_eq!(symbol(&engine).designator_in(&a), "R1");
    assert_eq!(symbol(&engine).reference, "R1", "the base is untouched");
    let added = symbol(&engine).instances.last().unwrap();
    assert_eq!(added.project, "board", "the project name is carried over");
    assert_eq!(added.path, b.to_string());

    engine.undo().expect("undo");
    assert_eq!(symbol(&engine).designator_in(&b), "R1");
    assert_eq!(symbol(&engine).instances.len(), 1);
}

#[test]
fn an_existing_entry_is_rewritten_and_an_unchanged_value_is_a_no_op() {
    let a = placement(1);
    let mut engine = engine(vec![entry(&a, "R7")]);

    assert!(!set(&mut engine, &a, "R7").changed);
    assert!(set(&mut engine, &a, "R8").changed);
    assert_eq!(symbol(&engine).instances.len(), 1);
    assert_eq!(symbol(&engine).designator_in(&a), "R8");
}

#[test]
fn a_blank_designator_drops_the_entry_so_the_placement_falls_back() {
    let a = placement(1);
    let mut engine = engine(vec![entry(&a, "R7")]);

    assert!(set(&mut engine, &a, "  ").changed);
    assert!(symbol(&engine).instances.is_empty());
    assert_eq!(symbol(&engine).designator_in(&a), "R1");
}

#[test]
fn the_base_designator_at_a_placement_without_an_entry_changes_nothing() {
    let mut engine = engine(Vec::new());
    assert!(!set(&mut engine, &placement(1), "R1").changed);
    assert!(!set(&mut engine, &InstancePath::root(), "R9").changed);
    // Dropping an entry the placement does not have is no change either.
    assert!(!set(&mut engine, &placement(1), "").changed);
    assert!(!engine.can_undo(), "no history step");
    assert!(symbol(&engine).instances.is_empty());
}
