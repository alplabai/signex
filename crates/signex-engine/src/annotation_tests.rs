//! Annotation follow-ups from the #825 review (#829).

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{Point, ProjectParts, Symbol};

use crate::Engine;
use crate::annotation::AnnotateScope;
use crate::command::{AnnotateMode, Command};
use crate::test_support::test_sheet;

fn unit(reference: &str, unit: u32, x: f64) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "Amp:Dual".to_string(),
        reference: reference.to_string(),
        unit,
        position: Point::new(x, 0.0),
        ..Default::default()
    }
}

/// Item 1: a Phase 1 rewrite that Phase 3 never touches (a locked unit's
/// designator copied onto its sibling) is still an edit, with an undo step.
#[test]
fn a_phase_one_rewrite_records_history() {
    let mut sheet = test_sheet();
    let locked_unit = unit("U1", 1, 0.0);
    let locked: HashSet<_> = [locked_unit.uuid].into();
    sheet.symbols.push(locked_unit);
    sheet.symbols.push(unit("U1 ", 2, 10.0));
    let mut engine = Engine::new(sheet).unwrap();
    let parts = ProjectParts::new(&[&engine.document().symbols], &locked);
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked: &locked,
    };

    let changed = engine
        .annotate_in_project(
            AnnotateMode::ResetAndRenumber,
            &mut HashMap::new(),
            scope,
            &mut HashMap::new(),
        )
        .unwrap();

    assert!(changed, "the sibling's designator was rewritten");
    assert_eq!(engine.document().symbols[1].reference, "U1");
    assert!(engine.can_undo(), "the rewrite must be undoable");
}

/// A designator Phase 1 resets and Phase 3 hands straight back is no edit:
/// no undo step for a pass that changed nothing.
#[test]
fn a_renumber_that_lands_on_the_same_designators_is_no_edit() {
    let mut sheet = test_sheet();
    sheet.symbols.push(unit("U1", 1, 0.0));
    let mut engine = Engine::new(sheet).unwrap();

    let changed = engine
        .annotate_with_seed(AnnotateMode::ResetAndRenumber, &mut HashMap::new())
        .unwrap();

    assert!(!changed);
    assert!(!engine.can_undo());
}

/// Item 4: `Command::AnnotateAll` uses the same part grouping, so the units
/// of a multi-unit part keep one designator.
#[test]
fn annotate_all_keeps_the_units_of_a_part_together() {
    let mut sheet = test_sheet();
    sheet.symbols.push(unit("U?", 1, 0.0));
    sheet.symbols.push(unit("U?", 2, 10.0));
    let mut engine = Engine::new(sheet).unwrap();

    let result = engine
        .execute(Command::AnnotateAll {
            mode: AnnotateMode::ResetAndRenumber,
        })
        .unwrap();

    assert!(result.changed);
    let refs: Vec<_> = engine
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.as_str())
        .collect();
    assert_eq!(refs, ["U1", "U1"]);
}
