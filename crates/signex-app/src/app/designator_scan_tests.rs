//! Numbering a new part against the whole project (#1381).

use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;

/// `top` is open; `child` is placed twice by it and is NOT open. The child's
/// one symbol is R1 at placement A and R101 at placement B.
#[test]
fn a_closed_sheets_designators_are_not_reused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    assert!(!app.document_state.engines.contains_key(&child));

    app.scan_designators_blocking(&top);

    assert_eq!(app.next_designator_in_sheet(&top, "R"), "R102");
}

#[test]
fn a_typed_designator_steps_past_one_a_closed_sheet_uses() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    app.scan_designators_blocking(&top);

    assert_eq!(app.free_designator_from(&top, "R101"), "R102");
    assert_eq!(app.free_designator_from(&top, "R50"), "R50");
}

/// An open sheet counts at every placement it is reached through, not just
/// its base reference.
#[test]
fn an_open_sheet_counts_at_each_of_its_placements() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    app.open_or_focus_sheet_path(child.clone());
    assert!(app.document_state.engines.contains_key(&child));
    app.scan_designators_blocking(&top);

    assert_eq!(app.next_designator_in_sheet(&top, "R"), "R102");
}

/// A row dropped on the canvas before the closed sheets were scanned waits
/// for the scan, so it is not numbered R1, which the closed child holds.
#[test]
fn a_dropped_part_waits_for_the_closed_sheets_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    let stock = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/stock-library/Stock.snxlib");
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == "STOCK-R")
        .expect("Stock lists STOCK-R");
    let table = app.library.table_of_row(&stock, row.row_id).expect("table");
    let row_id = signex_library::RowId::from_uuid(row.row_id);
    let before = app.document_state.engines[&top].document().symbols.len();

    let _read = app.handle_place_library_component(stock.clone(), table.clone(), row_id);
    let epoch = app.interaction_state.placement_epoch;
    app.library.components_panel.pending_drop = Some((epoch, (5.0, 5.0)));
    let open: Vec<_> = app
        .library
        .open_libraries
        .iter()
        .map(|l| (l.library_id, l.root.clone()))
        .collect();
    let resolved =
        crate::library::place::resolve_placement(&stock, &table, row_id, &open).map(Box::new);
    let _scan = app.handle_library_component_resolved(top.clone(), epoch, stock, resolved);

    let symbols = &app.document_state.engines[&top].document().symbols;
    assert_eq!(symbols.len(), before, "the drop waits for the scan");
    assert!(app.library.components_panel.pending_drop.is_some());

    app.scan_designators_blocking(&top);
    let _placed = app.commit_pending_drop();

    let symbols = &app.document_state.engines[&top].document().symbols;
    assert_eq!(symbols.len(), before + 1, "placed once the scan landed");
    assert_eq!(symbols[before].reference, "R102");
    assert!(app.interaction_state.placement.is_none(), "the tool ends");
}
