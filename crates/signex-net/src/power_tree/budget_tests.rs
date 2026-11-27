//! #1313: the power budget per sheet and per block, and the budget levels
//! of mockup row 171 (under 50 %, 50 to 90 %, over 90 %).

use std::collections::{BTreeMap, BTreeSet};

use signex_types::block_diagram::{Block, BlockDiagram, BlockShape, LinkState, SheetLink};
use signex_types::document_id::DocumentId;
use signex_types::schematic::Point;
use uuid::Uuid;

use super::topology_tests::{SHEET_ID, buck_sheet, topology_of};
use super::*;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn block(id: u128, sheet: Option<Uuid>) -> Block {
    Block {
        id: Uuid::from_u128(id),
        name: format!("B{id}"),
        function: String::new(),
        note: String::new(),
        shape: BlockShape::Rectangle,
        position: Point::new(0.0, 0.0),
        width_mm: 40.0,
        height_mm: 25.0,
        color: None,
        group: None,
        rails: Vec::new(),
        specs: BTreeMap::new(),
        ports: Vec::new(),
        sheet: sheet.map(|uuid| SheetLink {
            sheet: DocumentId::from_uuid(uuid).expect("not nil"),
            path: "top.snxsch".into(),
            state: LinkState::Linked,
        }),
    }
}

#[test]
fn a_sheet_budget_adds_up_what_its_parts_draw_and_supply_per_rail() {
    let topology = topology_of(buck_sheet(), &BTreeSet::new());
    let tree = power_tree(&topology, &BTreeMap::new());
    let budgets = sheet_budgets(&topology, &tree);
    assert_eq!(budgets.len(), 1);
    let sheet = &budgets[0];
    assert_eq!(sheet.sheet, SHEET_ID);
    let draws: Vec<(&str, bool)> = sheet
        .draws
        .iter()
        .map(|draw| (draw.net.as_str(), draw.is_complete))
        .collect();
    assert_eq!(draws, [("+3V3", true), ("+5V", true), ("VIN_12V", true)]);
    assert!(close(sheet.draws[0].current_a, 0.64));
    assert!(close(sheet.draws[1].current_a, 0.01));
    assert!(close(sheet.draws[2].current_a, 0.2));
    assert_eq!(sheet.feeds.len(), 1);
    assert_eq!(
        (sheet.feeds[0].net.as_str(), sheet.feeds[0].limit_a),
        ("+3V3", Some(3.0))
    );
    // 3.3 x 0.64 + 5 x 0.01 + 12 x 0.2 = 2.112 + 0.05 + 2.4.
    assert!(close(sheet.power_w(&tree).unwrap(), 4.562));
}

#[test]
fn a_block_takes_the_budget_of_its_linked_sheet_and_an_unlinked_block_none() {
    let topology = topology_of(buck_sheet(), &BTreeSet::new());
    let tree = power_tree(&topology, &BTreeMap::new());
    let budgets = sheet_budgets(&topology, &tree);
    let mut diagram = BlockDiagram::new();
    diagram.blocks = vec![
        block(1, Some(SHEET_ID)),
        block(2, None),
        block(3, Some(Uuid::from_u128(0xdead))),
    ];
    let blocks = block_budgets(&diagram, &budgets);
    let found: Vec<(Uuid, Option<Uuid>)> = blocks
        .iter()
        .map(|b| (b.block, b.budget.map(|budget| budget.sheet)))
        .collect();
    assert_eq!(
        found,
        [
            (Uuid::from_u128(1), Some(SHEET_ID)),
            (Uuid::from_u128(2), None),
            (Uuid::from_u128(3), None),
        ]
    );
}

#[test]
fn a_budget_level_is_fine_under_half_close_to_ninety_percent_and_over_above() {
    assert_eq!(BudgetLevel::of(0.2, 1.5), Some(BudgetLevel::Fine));
    assert_eq!(BudgetLevel::of(0.5, 1.0), Some(BudgetLevel::Close));
    assert_eq!(BudgetLevel::of(0.9, 1.0), Some(BudgetLevel::Close));
    assert_eq!(BudgetLevel::of(0.91, 1.0), Some(BudgetLevel::Over));
    assert_eq!(BudgetLevel::of(1.0, 0.0), None);
    assert_eq!(BudgetLevel::of(f64::NAN, 1.0), None);
}
