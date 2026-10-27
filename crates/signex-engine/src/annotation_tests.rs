//! Annotation follow-ups from the #825 review (#829).

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use signex_types::schematic::{AnnotateOrder, Point, ProjectParts, Symbol};

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
    let parts = ProjectParts::new(
        &[&engine.document().symbols],
        &locked,
        AnnotateOrder::default(),
    );
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked: &locked,
        undo_group: None,
        reprefix: None,
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
        .annotate_with_seed(
            AnnotateMode::ResetAndRenumber,
            &mut HashMap::new(),
            AnnotateOrder::default(),
        )
        .unwrap();

    assert!(!changed);
    assert!(!engine.can_undo());
}

/// #873 review: a reset over designators that are already reset changes
/// nothing, so it records no undo step and reports no change.
#[test]
fn a_reset_over_already_reset_designators_is_no_edit() {
    let mut sheet = test_sheet();
    sheet.symbols.push(unit("U?", 1, 0.0));
    let mut engine = Engine::new(sheet).unwrap();

    let changed = engine
        .annotate_with_seed(
            AnnotateMode::ResetOnly,
            &mut HashMap::new(),
            AnnotateOrder::default(),
        )
        .unwrap();

    assert!(!changed);
    assert!(!engine.can_undo());
}

/// The same reset over an annotated designator is an edit, with an undo step.
#[test]
fn a_reset_that_clears_a_designator_records_history() {
    let mut sheet = test_sheet();
    sheet.symbols.push(unit("U1", 1, 0.0));
    let mut engine = Engine::new(sheet).unwrap();

    let changed = engine
        .annotate_with_seed(
            AnnotateMode::ResetOnly,
            &mut HashMap::new(),
            AnnotateOrder::default(),
        )
        .unwrap();

    assert!(changed);
    assert_eq!(engine.document().symbols[0].reference, "U?");
    assert!(engine.can_undo());
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
            order: AnnotateOrder::default(),
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

/// #829 item 2: an annotate pass given an undo group stamps its history
/// entry with it, and the stamp follows the entry through undo and redo.
#[test]
fn an_annotate_undo_group_is_stamped_and_survives_undo_and_redo() {
    let mut sheet = test_sheet();
    sheet.symbols.push(unit("U?", 1, 0.0));
    let mut engine = Engine::new(sheet).unwrap();
    let locked = HashSet::new();
    let parts = ProjectParts::new(
        &[&engine.document().symbols],
        &locked,
        AnnotateOrder::default(),
    );
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked: &locked,
        undo_group: Some(crate::UndoGroupId::new(7)),
        reprefix: None,
    };
    engine
        .annotate_in_project(
            AnnotateMode::ResetAndRenumber,
            &mut HashMap::new(),
            scope,
            &mut HashMap::new(),
        )
        .unwrap();

    assert_eq!(engine.next_undo_group(), Some(crate::UndoGroupId::new(7)));
    engine.undo().unwrap();
    assert_eq!(engine.next_undo_group(), None);
    assert_eq!(engine.next_redo_group(), Some(crate::UndoGroupId::new(7)));
    engine.redo().unwrap();
    assert_eq!(engine.next_undo_group(), Some(crate::UndoGroupId::new(7)));
}

fn refs_of(engine: &Engine) -> Vec<String> {
    engine
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

fn at(x: f64, y: f64) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
        lib_id: "Device:R".to_string(),
        reference: "R?".to_string(),
        position: Point::new(x, y),
        ..Default::default()
    }
}

/// Designators in the input order `[top-left, top-right, bottom-left,
/// bottom-right]` (y grows down) after annotating that 2x2 grid in `order`.
fn grid_designators(order: AnnotateOrder) -> Vec<String> {
    let mut sheet = test_sheet();
    sheet.symbols = vec![at(0.0, 0.0), at(10.0, 0.0), at(0.0, 10.0), at(10.0, 10.0)];
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .annotate_with_seed(AnnotateMode::Incremental, &mut HashMap::new(), order)
        .unwrap();
    refs_of(&engine)
}

#[test]
fn across_then_down_numbers_rows_top_to_bottom() {
    assert_eq!(
        grid_designators(AnnotateOrder::AcrossThenDown),
        ["R1", "R2", "R3", "R4"]
    );
}

#[test]
fn across_then_up_numbers_rows_bottom_to_top() {
    assert_eq!(
        grid_designators(AnnotateOrder::AcrossThenUp),
        ["R3", "R4", "R1", "R2"]
    );
}

#[test]
fn up_then_across_numbers_columns_bottom_to_top() {
    assert_eq!(
        grid_designators(AnnotateOrder::UpThenAcross),
        ["R2", "R4", "R1", "R3"]
    );
}

#[test]
fn down_then_across_numbers_columns_top_to_bottom() {
    assert_eq!(
        grid_designators(AnnotateOrder::DownThenAcross),
        ["R1", "R3", "R2", "R4"]
    );
}

#[test]
fn the_default_order_is_rows_top_to_bottom() {
    assert_eq!(AnnotateOrder::default(), AnnotateOrder::AcrossThenDown);
}

/// The order picks the anchor unit of a multi-unit part: the part is numbered
/// when its first unit is reached, and the sibling follows it.
#[test]
fn the_order_picks_the_anchor_of_a_multi_unit_part() {
    let mut sheet = test_sheet();
    // Unit 1 top-left, unit 2 bottom-right, a lone resistor bottom-left.
    sheet.symbols = vec![
        unit("U?", 1, 0.0),
        {
            let mut r = at(0.0, 10.0);
            r.reference = "U?".to_string();
            r.lib_id = "Other:Part".to_string();
            r
        },
        unit("U?", 2, 10.0),
    ];
    sheet.symbols[0].position = Point::new(0.0, 0.0);
    sheet.symbols[2].position = Point::new(10.0, 10.0);
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .annotate_with_seed(
            AnnotateMode::Incremental,
            &mut HashMap::new(),
            AnnotateOrder::AcrossThenUp,
        )
        .unwrap();
    // Bottom row first: the loner (bottom-left) is U1, then the two units of
    // the multi-unit part share U2.
    assert_eq!(refs_of(&engine), ["U2", "U1", "U2"]);
}

#[test]
fn annotate_all_command_honours_the_order() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![at(0.0, 0.0), at(0.0, 10.0)];
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .execute(Command::AnnotateAll {
            mode: AnnotateMode::Incremental,
            order: AnnotateOrder::AcrossThenUp,
        })
        .unwrap();
    assert_eq!(refs_of(&engine), ["R2", "R1"]);
}

/// Units A(0,0) u1, B(10,0) u2, C(0,10) u2, D(10,10) u1 of one library symbol:
/// the order given to the pass is the one the parts group by, so the pairing
/// has a single source and cannot diverge from the numbering.
#[test]
fn the_part_pairing_follows_the_one_order_of_the_pass() {
    let designators = |order| {
        let mut sheet = test_sheet();
        sheet.symbols = vec![
            unit("U?", 1, 0.0),
            unit("U?", 2, 10.0),
            unit("U?", 2, 0.0),
            unit("U?", 1, 10.0),
        ];
        sheet.symbols[2].position = Point::new(0.0, 10.0);
        sheet.symbols[3].position = Point::new(10.0, 10.0);
        let mut engine = Engine::new(sheet).unwrap();
        engine
            .annotate_with_seed(AnnotateMode::Incremental, &mut HashMap::new(), order)
            .unwrap();
        refs_of(&engine)
    };
    // (A,B)=U1 and (C,D)=U2 across then down.
    assert_eq!(
        designators(AnnotateOrder::AcrossThenDown),
        ["U1", "U1", "U2", "U2"]
    );
    // (A,C)=U1 and (B,D)=U2 down then across.
    assert_eq!(
        designators(AnnotateOrder::DownThenAcross),
        ["U1", "U2", "U1", "U2"]
    );
}

/// One Incremental pass over `engine` that re-prefixes `targets`.
fn reprefix_pass(
    engine: &mut Engine,
    locked: &HashSet<Uuid>,
    targets: &HashMap<Uuid, String>,
) -> Result<bool, crate::EngineError> {
    let parts = ProjectParts::new(
        &[&engine.document().symbols],
        locked,
        AnnotateOrder::default(),
    );
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked,
        undo_group: None,
        reprefix: Some(targets),
    };
    engine.annotate_in_project(
        AnnotateMode::Incremental,
        &mut HashMap::new(),
        scope,
        &mut HashMap::new(),
    )
}

fn targets_of(symbols: &[&Symbol], prefix: &str) -> HashMap<Uuid, String> {
    symbols
        .iter()
        .map(|s| (s.uuid, prefix.to_owned()))
        .collect()
}

#[test]
fn reprefix_and_renumber_record_one_history_entry() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![at_ref("R1", 0.0), at_ref("R2", 10.0), at_ref("V7", 20.0)];
    let targets = targets_of(&[&sheet.symbols[0], &sheet.symbols[1]], "V");
    let mut engine = Engine::new(sheet).unwrap();

    assert!(reprefix_pass(&mut engine, &HashSet::new(), &targets).unwrap());

    assert_eq!(refs_of(&engine), ["V8", "V9", "V7"]);
    engine.undo().unwrap();
    assert_eq!(refs_of(&engine), ["R1", "R2", "V7"]);
    assert!(!engine.can_undo(), "reset and renumber are ONE entry");
}

#[test]
fn reprefix_keeps_units_of_one_part_together() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![unit("R1", 1, 0.0), unit("R1", 2, 10.0), unit("R2", 1, 20.0)];
    let targets = targets_of(&[&sheet.symbols[0], &sheet.symbols[1]], "N");
    let mut engine = Engine::new(sheet).unwrap();

    reprefix_pass(&mut engine, &HashSet::new(), &targets).unwrap();

    assert_eq!(refs_of(&engine), ["N1", "N1", "R2"]);
}

#[test]
fn reprefix_skips_a_part_with_a_locked_unit() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![unit("R5", 1, 0.0), unit("R5", 2, 10.0)];
    let locked: HashSet<_> = [sheet.symbols[0].uuid].into();
    // The map names BOTH units; the lock on one pins the whole part.
    let targets = targets_of(&[&sheet.symbols[0], &sheet.symbols[1]], "N");
    let mut engine = Engine::new(sheet).unwrap();

    assert!(!reprefix_pass(&mut engine, &locked, &targets).unwrap());

    assert_eq!(refs_of(&engine), ["R5", "R5"]);
    assert!(!engine.can_undo());
}

#[test]
fn overflow_after_reprefix_restores_the_sheet() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![at_ref("V4294967295", 0.0), at_ref("R1", 10.0)];
    let targets = targets_of(&[&sheet.symbols[1]], "V");
    let mut engine = Engine::new(sheet).unwrap();

    let result = reprefix_pass(&mut engine, &HashSet::new(), &targets);

    assert!(
        matches!(result, Err(crate::EngineError::DesignatorOverflow { ref prefix }) if prefix == "V"),
        "got {result:?}"
    );
    assert_eq!(
        refs_of(&engine),
        ["V4294967295", "R1"],
        "Phase 0 is undone too"
    );
    assert!(!engine.can_undo());
}

fn at_ref(reference: &str, x: f64) -> Symbol {
    Symbol {
        reference: reference.to_string(),
        ..at(x, 0.0)
    }
}

#[test]
fn overflow_on_one_sheet_does_not_leak_assigned_to_the_next() {
    let mut sheet = test_sheet();
    // `N?` is numbered first (and recorded in `assigned`), then `V?` hits the
    // saturated counter and fails the whole sheet.
    sheet.symbols = vec![
        at_ref("V4294967295", 0.0),
        at_ref("N?", 10.0),
        at_ref("V?", 20.0),
    ];
    let mut engine = Engine::new(sheet).unwrap();
    let locked = HashSet::new();
    let parts = ProjectParts::new(
        &[&engine.document().symbols],
        &locked,
        AnnotateOrder::default(),
    );
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked: &locked,
        undo_group: None,
        reprefix: None,
    };
    let mut counters = HashMap::new();
    let mut assigned = HashMap::new();

    let result = engine.annotate_in_project(
        AnnotateMode::Incremental,
        &mut counters,
        scope,
        &mut assigned,
    );

    assert!(matches!(
        result,
        Err(crate::EngineError::DesignatorOverflow { .. })
    ));
    assert!(assigned.is_empty(), "leaked {assigned:?}");
    assert!(counters.is_empty(), "leaked {counters:?}");
}

#[test]
fn discarding_a_group_entry_leaves_nothing_to_redo() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![at_ref("R1", 0.0)];
    let targets = targets_of(&[&sheet.symbols[0]], "V");
    let mut engine = Engine::new(sheet).unwrap();
    let locked = HashSet::new();
    let parts = ProjectParts::new(
        &[&engine.document().symbols],
        &locked,
        AnnotateOrder::default(),
    );
    let group = crate::UndoGroupId::new(3);
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked: &locked,
        undo_group: Some(group),
        reprefix: Some(&targets),
    };
    engine
        .annotate_in_project(
            AnnotateMode::Incremental,
            &mut HashMap::new(),
            scope,
            &mut HashMap::new(),
        )
        .unwrap();

    // Another group's entry is left alone.
    assert!(
        engine
            .discard_undo_group_top(crate::UndoGroupId::new(4))
            .unwrap()
            .is_none()
    );
    assert_eq!(refs_of(&engine), ["V1"]);

    assert!(engine.discard_undo_group_top(group).unwrap().is_some());
    assert_eq!(refs_of(&engine), ["R1"]);
    assert!(!engine.can_undo() && !engine.can_redo());
}
