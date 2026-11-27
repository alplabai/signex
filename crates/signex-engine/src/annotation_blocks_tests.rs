//! #1292: a page numbering scheme hands each unit the numbers of its page.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use signex_types::designator::{NumberBlocks, NumberingScheme};
use signex_types::schematic::{AnnotateOrder, InstancePath, Point, ProjectParts, Symbol};

use crate::annotation::AnnotateScope;
use crate::command::AnnotateMode;
use crate::test_support::test_sheet;
use crate::{AnnotatePlacement, Engine, EngineError};

fn part(reference: &str, lib_id: &str, unit: u32, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        unit,
        position: Point::new(x, 0.0),
        ..Default::default()
    }
}

fn resistor(reference: &str, x: f64) -> Symbol {
    part(reference, "Device:R", 1, x)
}

fn engine_with(symbols: Vec<Symbol>) -> Engine {
    let mut sheet = test_sheet();
    sheet.symbols = symbols;
    Engine::new(sheet).unwrap()
}

fn refs(engine: &Engine) -> Vec<String> {
    engine
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

/// A pass over `engines` (one unit each, in order) under `blocks`.
fn annotate_paged(
    mode: AnnotateMode,
    engines: &mut [Engine],
    blocks: &NumberBlocks,
) -> Vec<Result<bool, EngineError>> {
    let locked = HashSet::new();
    let sheets: Vec<Vec<Symbol>> = engines
        .iter()
        .map(|e| e.document().symbols.clone())
        .collect();
    let slices: Vec<&[Symbol]> = sheets.iter().map(Vec::as_slice).collect();
    let parts = ProjectParts::new(&slices, &locked, AnnotateOrder::default());
    let mut seed = HashMap::new();
    let mut assigned = HashMap::new();
    engines
        .iter_mut()
        .enumerate()
        .map(|(sheet, engine)| {
            let scope = AnnotateScope {
                parts: &parts,
                sheet,
                locked: &locked,
                undo_group: None,
                reprefix: None,
                placements: &[],
                blocks: Some(blocks),
            };
            engine.annotate_in_project(mode, &mut seed, scope, &mut assigned)
        })
        .collect()
}

#[test]
fn each_sheet_numbers_from_its_own_page_block() {
    let mut engines = vec![
        engine_with(vec![resistor("R?", 0.0), resistor("R?", 10.0)]),
        engine_with(vec![resistor("R?", 0.0), part("C?", "Device:C", 1, 10.0)]),
    ];
    let blocks = NumberBlocks::new(NumberingScheme::PageHundreds, &[Some(1), Some(2)], [])
        .unwrap()
        .unwrap();
    let done = annotate_paged(AnnotateMode::Incremental, &mut engines, &blocks);
    assert!(done.iter().all(|r| matches!(r, Ok(true))), "{done:?}");
    assert_eq!(refs(&engines[0]), ["R101", "R102"]);
    assert_eq!(refs(&engines[1]), ["R201", "C201"]);
}

/// Incremental keeps what is numbered; the next number goes past what the
/// block already holds, wherever the holder sits.
#[test]
fn a_kept_number_in_the_block_is_skipped() {
    let mut engines = vec![
        engine_with(vec![resistor("R7", 0.0), resistor("R?", 10.0)]),
        engine_with(vec![resistor("R105", 0.0)]),
    ];
    let blocks = NumberBlocks::new(
        NumberingScheme::PageHundreds,
        &[Some(1), Some(2)],
        ["R7", "R105"],
    )
    .unwrap()
    .unwrap();
    annotate_paged(AnnotateMode::Incremental, &mut engines, &blocks);
    assert_eq!(refs(&engines[0]), ["R7", "R106"]);
    assert_eq!(refs(&engines[1]), ["R105"]);
}

/// A multi-unit part split over two pages takes the number of the page its
/// first unit is on.
#[test]
fn a_part_split_over_pages_keeps_one_designator() {
    let mut engines = vec![
        engine_with(vec![part("U?", "Amp:Dual", 1, 0.0)]),
        engine_with(vec![part("U?", "Amp:Dual", 2, 0.0)]),
    ];
    let blocks = NumberBlocks::new(NumberingScheme::PageThousands, &[Some(1), Some(2)], [])
        .unwrap()
        .unwrap();
    annotate_paged(AnnotateMode::Incremental, &mut engines, &blocks);
    assert_eq!(refs(&engines[0]), ["U1001"]);
    assert_eq!(refs(&engines[1]), ["U1001"]);
}

/// A full block refuses the sheet whole: nothing half renumbered, no step.
#[test]
fn a_full_block_refuses_the_sheet_and_changes_nothing() {
    let mut engines = vec![engine_with(vec![
        resistor("R199", 0.0),
        resistor("R?", 10.0),
    ])];
    let blocks = NumberBlocks::new(NumberingScheme::PageHundreds, &[Some(1)], ["R199"])
        .unwrap()
        .unwrap();
    let done = annotate_paged(AnnotateMode::Incremental, &mut engines, &blocks);
    assert!(
        matches!(&done[0], Err(EngineError::DesignatorBlock { prefix, .. }) if prefix == "R"),
        "{done:?}"
    );
    assert_eq!(refs(&engines[0]), ["R199", "R?"]);
    assert!(!engines[0].can_undo());
}

/// A unit the blocks give no page refuses rather than number continuously.
#[test]
fn a_unit_without_a_page_is_refused() {
    let mut engines = vec![engine_with(vec![resistor("R?", 0.0)])];
    let blocks = NumberBlocks::new(NumberingScheme::PageHundreds, &[None], [])
        .unwrap()
        .unwrap();
    let done = annotate_paged(AnnotateMode::Incremental, &mut engines, &blocks);
    assert!(matches!(done[0], Err(EngineError::DesignatorBlock { .. })));
    assert_eq!(refs(&engines[0]), ["R?"]);
}

/// A file placed twice (#1835) numbers each placement from its own page.
#[test]
fn each_placement_numbers_from_its_own_page() {
    let mut engine = engine_with(vec![resistor("R?", 0.0)]);
    let paths: Vec<InstancePath> = [1u128, 2]
        .iter()
        .map(|n| InstancePath::from_chain(vec![Uuid::from_u128(*n)]))
        .collect();
    let views: Vec<Vec<Symbol>> = paths
        .iter()
        .map(|_| engine.document().symbols.clone())
        .collect();
    let slices: Vec<&[Symbol]> = views.iter().map(Vec::as_slice).collect();
    let locked = HashSet::new();
    let parts = ProjectParts::new(&slices, &locked, AnnotateOrder::default());
    let placements: Vec<AnnotatePlacement> = paths
        .iter()
        .enumerate()
        .map(|(sheet, path)| AnnotatePlacement {
            sheet,
            path: path.clone(),
        })
        .collect();
    let blocks = NumberBlocks::new(NumberingScheme::PageHundreds, &[Some(3), Some(4)], [])
        .unwrap()
        .unwrap();
    let scope = AnnotateScope {
        parts: &parts,
        sheet: 0,
        locked: &locked,
        undo_group: None,
        reprefix: None,
        placements: &placements,
        blocks: Some(&blocks),
    };
    engine
        .annotate_in_project(
            AnnotateMode::Incremental,
            &mut HashMap::new(),
            scope,
            &mut HashMap::new(),
        )
        .unwrap();
    let symbol = &engine.document().symbols[0];
    assert_eq!(symbol.designator_in(&paths[0]), "R301");
    assert_eq!(symbol.designator_in(&paths[1]), "R401");
}
