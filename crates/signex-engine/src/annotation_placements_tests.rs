//! #1835: a sheet file placed more than once is numbered per placement.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use signex_types::schematic::{AnnotateOrder, InstancePath, Point, ProjectParts, Symbol};

use crate::annotation::AnnotateScope;
use crate::command::AnnotateMode;
use crate::test_support::test_sheet;
use crate::{AnnotatePlacement, Engine};

fn part(reference: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:Q".to_string(),
        reference: reference.to_string(),
        position: Point::new(x, 0.0),
        ..Default::default()
    }
}

fn placement_path(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![Uuid::from_u128(n)])
}

fn engine_with(symbols: Vec<Symbol>) -> Engine {
    let mut sheet = test_sheet();
    sheet.symbols = symbols;
    Engine::new(sheet).unwrap()
}

/// One pass over `engine`'s sheet placed at each of `paths`, with a fresh counter.
fn annotate_placed(
    engine: &mut Engine,
    paths: &[InstancePath],
    mode: AnnotateMode,
    locked: &HashSet<Uuid>,
) -> bool {
    let views: Vec<Vec<Symbol>> = paths
        .iter()
        .map(|path| {
            let mut view = engine.document().symbols.clone();
            for symbol in &mut view {
                symbol.reference = symbol.designator_in(path).to_string();
            }
            view
        })
        .collect();
    let slices: Vec<&[Symbol]> = views.iter().map(Vec::as_slice).collect();
    let parts = ProjectParts::new(&slices, locked, AnnotateOrder::default());
    let placements: Vec<AnnotatePlacement> = paths
        .iter()
        .enumerate()
        .map(|(sheet, path)| AnnotatePlacement {
            sheet,
            path: path.clone(),
        })
        .collect();
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked,
        undo_group: None,
        reprefix: None,
        placements: &placements,
    };
    engine
        .annotate_in_project(mode, &mut HashMap::new(), scope, &mut HashMap::new())
        .unwrap()
}

fn designators_at(engine: &Engine, path: &InstancePath) -> Vec<String> {
    engine
        .document()
        .symbols
        .iter()
        .map(|s| s.designator_in(path).to_string())
        .collect()
}

#[test]
fn a_reset_gives_each_placement_its_own_designators_from_one() {
    let mut engine = engine_with(vec![part("Q5", 0.0), part("Q6", 10.0)]);
    let (a, b) = (placement_path(1), placement_path(2));

    let changed = annotate_placed(
        &mut engine,
        &[a.clone(), b.clone()],
        AnnotateMode::ResetAndRenumber,
        &HashSet::new(),
    );

    assert!(changed);
    assert!(engine.can_undo(), "per-placement edits are undoable");
    assert_eq!(designators_at(&engine, &a), ["Q1", "Q2"]);
    assert_eq!(designators_at(&engine, &b), ["Q3", "Q4"]);
}

#[test]
fn a_part_added_later_gets_a_designator_per_placement() {
    let mut engine = engine_with(vec![part("Q1", 0.0)]);
    let (a, b) = (placement_path(1), placement_path(2));
    annotate_placed(
        &mut engine,
        &[a.clone(), b.clone()],
        AnnotateMode::ResetAndRenumber,
        &HashSet::new(),
    );
    engine.document.symbols.push(part("Q?", 20.0));

    annotate_placed(
        &mut engine,
        &[a.clone(), b.clone()],
        AnnotateMode::Incremental,
        &HashSet::new(),
    );

    let new = &engine.document().symbols[1];
    assert_ne!(new.designator_in(&a), new.designator_in(&b));
    assert!(!new.designator_in(&a).ends_with('?'));
    assert!(!new.designator_in(&b).ends_with('?'));
}

#[test]
fn a_reset_only_clears_the_placement_entries() {
    let mut engine = engine_with(vec![part("Q1", 0.0)]);
    let (a, b) = (placement_path(1), placement_path(2));
    annotate_placed(
        &mut engine,
        &[a.clone(), b.clone()],
        AnnotateMode::ResetAndRenumber,
        &HashSet::new(),
    );

    annotate_placed(
        &mut engine,
        &[a, b],
        AnnotateMode::ResetOnly,
        &HashSet::new(),
    );

    let symbol = &engine.document().symbols[0];
    assert_eq!(symbol.reference, "Q?");
    assert!(symbol.instances.is_empty());
}

#[test]
fn a_locked_part_keeps_its_designator_in_every_placement() {
    let locked_part = part("Q7", 0.0);
    let locked: HashSet<_> = [locked_part.uuid].into();
    let mut engine = engine_with(vec![locked_part, part("Q8", 10.0)]);
    let (a, b) = (placement_path(1), placement_path(2));

    annotate_placed(
        &mut engine,
        &[a.clone(), b.clone()],
        AnnotateMode::ResetAndRenumber,
        &locked,
    );

    let locked_symbol = &engine.document().symbols[0];
    assert_eq!(locked_symbol.designator_in(&a), "Q7");
    assert_eq!(locked_symbol.designator_in(&b), "Q7");
    assert!(locked_symbol.instances.is_empty());
}

#[test]
fn an_unchanged_placed_sheet_records_nothing() {
    let mut engine = engine_with(vec![part("Q1", 0.0)]);
    let (a, b) = (placement_path(1), placement_path(2));
    annotate_placed(
        &mut engine,
        &[a.clone(), b.clone()],
        AnnotateMode::Incremental,
        &HashSet::new(),
    );
    let history_before = engine.can_undo();

    let changed = annotate_placed(
        &mut engine,
        &[a, b],
        AnnotateMode::Incremental,
        &HashSet::new(),
    );

    assert!(!changed);
    assert_eq!(engine.can_undo(), history_before);
}
