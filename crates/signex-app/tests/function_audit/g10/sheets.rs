//! m78 Number Schematic Sheets, m79 Sheet Sync, m80 Assign Footprints.

use std::path::{Path, PathBuf};

use signex_app::app::{
    AssignFilter, AssignFootprintsMsg, Message, PartAccess, SheetNumberingMsg, SheetSyncMsg, Signex,
};
use signex_engine::SheetSyncDirection;
use signex_types::schematic::{
    Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol,
};
use uuid::Uuid;

use super::{Rig, fresh_app, loaded_symbol, menu, part, placement, rig, rig_on, sheet, undo};
use crate::settle_support::settle;

const PAGE_A: Uuid = Uuid::from_u128(0x7801);
const PAGE_C: Uuid = Uuid::from_u128(0x7802);
const SYNC_SYMBOL: Uuid = Uuid::from_u128(0x7901);
const R1: Uuid = Uuid::from_u128(0x8001);
const R2: Uuid = Uuid::from_u128(0x8002);
const R3: Uuid = Uuid::from_u128(0x8003);
const PLACED: Uuid = Uuid::from_u128(0x8004);

// ---- m78 Number Schematic Sheets ----

/// Pages in hierarchy order: row 0 the root, row 1 `/A`, row 2 `/C`.
fn numbering() -> Rig {
    let root = sheet(
        Vec::new(),
        vec![
            placement(PAGE_A, "A", "a.snxsch", 0.0),
            placement(PAGE_C, "C", "c.snxsch", 20.0),
        ],
    );
    let closed = [
        ("a.snxsch", sheet(Vec::new(), Vec::new())),
        ("c.snxsch", sheet(Vec::new(), Vec::new())),
    ];
    let mut rig = rig(root, &closed, None);
    menu(&mut rig.app, "number_schematic_sheets");
    assert_eq!(
        rig.app
            .ui_state
            .sheet_numbering
            .as_ref()
            .and_then(|state| state.page_count()),
        Some(3),
        "the dialog lists the three pages"
    );
    rig
}

fn number(app: &mut Signex, msg: SheetNumberingMsg) {
    settle(app, Message::from(msg));
}

fn type_page(app: &mut Signex, row: usize, text: &str) {
    number(
        app,
        SheetNumberingMsg::NumberEdited {
            row,
            text: text.to_string(),
        },
    );
}

fn can_apply_numbers(app: &Signex) -> bool {
    app.ui_state
        .sheet_numbering
        .as_ref()
        .expect("the dialog is open")
        .can_apply()
}

fn pages(rig: &Rig, symbol: Uuid) -> Vec<String> {
    rig.app.document_state.engines[&rig.root]
        .document()
        .child_sheets
        .iter()
        .find(|child| child.uuid == symbol)
        .expect("sheet symbol")
        .instances
        .iter()
        .map(|instance| instance.page.clone())
        .collect()
}

#[test]
fn m78_apply_writes_the_typed_numbers_and_one_undo_takes_them_back() {
    let mut rig = numbering();
    let root_file = std::fs::read(&rig.root).expect("root file");
    type_page(&mut rig.app, 1, "3");
    type_page(&mut rig.app, 2, "2");
    assert!(can_apply_numbers(&rig.app));

    number(&mut rig.app, SheetNumberingMsg::ApplyRequested);

    assert!(
        rig.app.ui_state.sheet_numbering.is_none(),
        "Apply closes it"
    );
    assert_eq!(pages(&rig, PAGE_A), ["3"]);
    assert_eq!(pages(&rig, PAGE_C), ["2"]);
    assert!(rig.app.document_state.is_document_dirty(&rig.root));
    assert_eq!(
        std::fs::read(&rig.root).expect("root file"),
        root_file,
        "nothing is saved until the user saves"
    );

    undo(&mut rig.app);

    assert!(pages(&rig, PAGE_A).is_empty());
    assert!(pages(&rig, PAGE_C).is_empty());
}

#[test]
fn m78_cancel_drops_the_typed_numbers() {
    let mut rig = numbering();
    type_page(&mut rig.app, 1, "3");
    type_page(&mut rig.app, 2, "2");

    number(&mut rig.app, SheetNumberingMsg::CancelRequested);

    assert!(rig.app.ui_state.sheet_numbering.is_none());
    assert!(pages(&rig, PAGE_A).is_empty());
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m78_a_clash_holds_apply_back_and_apply_pressed_anyway_changes_nothing() {
    let mut rig = numbering();
    type_page(&mut rig.app, 2, "2");
    assert!(!can_apply_numbers(&rig.app), "two pages are numbered 2");

    number(&mut rig.app, SheetNumberingMsg::ApplyRequested);

    assert!(
        rig.app.ui_state.sheet_numbering.is_some(),
        "the dialog stays"
    );
    assert!(pages(&rig, PAGE_C).is_empty());
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m78_a_number_above_the_page_count_or_not_a_number_holds_apply_back() {
    let mut rig = numbering();
    for bad in ["9", "0", "", "x", "+2", "-1"] {
        type_page(&mut rig.app, 2, bad);
        assert!(!can_apply_numbers(&rig.app), "{bad:?}");
    }
}

#[test]
fn m78_the_top_level_page_cannot_be_renumbered() {
    let mut rig = numbering();
    type_page(&mut rig.app, 0, "2");
    assert!(
        !can_apply_numbers(&rig.app),
        "the edit is ignored, so nothing changes"
    );
}

#[test]
fn m78_auto_number_clears_the_hand_numbers() {
    let mut rig = numbering();
    type_page(&mut rig.app, 1, "3");
    type_page(&mut rig.app, 2, "2");
    number(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    menu(&mut rig.app, "number_schematic_sheets");

    number(&mut rig.app, SheetNumberingMsg::AutoNumberRequested);
    assert!(can_apply_numbers(&rig.app));
    number(&mut rig.app, SheetNumberingMsg::ApplyRequested);

    assert!(pages(&rig, PAGE_A).is_empty(), "/A follows its position");
    assert!(pages(&rig, PAGE_C).is_empty(), "/C follows its position");
}

// ---- m79 Synchronize Sheet Entries and Ports ----

fn port(name: &str, y: f64) -> Label {
    Label {
        shape: "input".to_string(),
        ..Label::new(name, Point::new(10.0, y), LabelType::Hierarchical)
    }
}

/// A root placing `sync.snxsch` with no entries; the child has two ports.
fn sync_rig() -> (Rig, PathBuf) {
    let root = sheet(
        Vec::new(),
        vec![placement(SYNC_SYMBOL, "U_Power", "sync.snxsch", 50.0)],
    );
    let child = SchematicSheet {
        labels: vec![port("SDA", 10.0), port("SCL", 15.0)],
        ..sheet(Vec::new(), Vec::new())
    };
    let rig = rig(root, &[("sync.snxsch", child)], None);
    let path = rig.dir.join("sync.snxsch");
    (rig, path)
}

fn open_sync(app: &mut Signex) {
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(SYNC_SYMBOL, SelectedKind::ChildSheet)];
    sync(app, SheetSyncMsg::Requested);
}

fn sync(app: &mut Signex, msg: SheetSyncMsg) {
    settle(app, Message::from(msg));
}

fn entry_names(rig: &Rig) -> Vec<String> {
    let mut names: Vec<String> = rig.app.document_state.engines[&rig.root]
        .document()
        .child_sheets
        .iter()
        .find(|child| child.uuid == SYNC_SYMBOL)
        .expect("sheet symbol")
        .pins
        .iter()
        .map(|pin| pin.name.clone())
        .collect();
    names.sort();
    names
}

#[test]
fn m79_sheet_to_symbol_adds_the_entries_the_preview_lists_and_undo_removes_them() {
    let (mut rig, _child) = sync_rig();
    open_sync(&mut rig.app);
    let state = rig.app.ui_state.sheet_sync.as_ref().expect("dialog open");
    let mut added = state.preview().expect("a preview").added.clone();
    added.sort();
    assert_eq!(added, ["SCL", "SDA"]);
    assert!(state.can_apply());
    assert!(entry_names(&rig).is_empty(), "only a preview");

    sync(&mut rig.app, SheetSyncMsg::ApplyRequested);

    assert!(rig.app.ui_state.sheet_sync.is_none(), "Apply closes it");
    assert_eq!(entry_names(&rig), ["SCL", "SDA"]);
    assert!(rig.app.document_state.is_document_dirty(&rig.root));

    undo(&mut rig.app);

    assert!(entry_names(&rig).is_empty());
}

#[test]
fn m79_symbol_to_sheet_changes_the_closed_child_in_memory_only() {
    let (mut rig, child) = sync_rig();
    let child_file = std::fs::read(&child).expect("child file");
    open_sync(&mut rig.app);

    sync(
        &mut rig.app,
        SheetSyncMsg::DirectionPicked(SheetSyncDirection::SymbolToSheet),
    );
    let state = rig.app.ui_state.sheet_sync.as_ref().expect("dialog open");
    let mut removed = state.preview().expect("a preview").removed.clone();
    removed.sort();
    assert_eq!(removed, ["SCL", "SDA"], "the list follows the direction");

    sync(&mut rig.app, SheetSyncMsg::ApplyRequested);

    assert!(rig.app.ui_state.sheet_sync.is_none());
    let ports = rig.app.document_state.engines[&child]
        .document()
        .labels
        .iter()
        .filter(|label| label.label_type == LabelType::Hierarchical)
        .count();
    assert_eq!(ports, 0, "the child follows the symbol");
    assert!(rig.app.document_state.is_document_dirty(&child));
    assert_eq!(std::fs::read(&child).expect("child file"), child_file);
    assert!(entry_names(&rig).is_empty(), "the symbol is not changed");
}

#[test]
fn m79_cancel_changes_neither_side() {
    let (mut rig, child) = sync_rig();
    open_sync(&mut rig.app);

    sync(&mut rig.app, SheetSyncMsg::CancelRequested);

    assert!(rig.app.ui_state.sheet_sync.is_none());
    assert!(entry_names(&rig).is_empty());
    assert!(!rig.app.document_state.engines.contains_key(&child));
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m79_with_both_sides_in_step_apply_is_off_and_does_nothing() {
    let (mut rig, _child) = sync_rig();
    open_sync(&mut rig.app);
    sync(&mut rig.app, SheetSyncMsg::ApplyRequested);
    open_sync(&mut rig.app);
    let state = rig.app.ui_state.sheet_sync.as_ref().expect("dialog open");
    assert!(state.preview().expect("a preview").is_empty());
    assert!(!state.can_apply());

    sync(&mut rig.app, SheetSyncMsg::ApplyRequested);

    assert!(rig.app.ui_state.sheet_sync.is_some(), "nothing to apply");
    assert_eq!(entry_names(&rig), ["SCL", "SDA"]);
}

#[test]
fn m79_with_no_sheet_symbol_selected_no_dialog_opens() {
    let (mut rig, _child) = sync_rig();
    rig.app.interaction_state.active_canvas_mut().selected = Vec::new();

    sync(&mut rig.app, SheetSyncMsg::Requested);

    assert!(rig.app.ui_state.sheet_sync.is_none());
}

// ---- m80 Assign Footprints ----

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn mount_stock(app: &mut Signex) {
    let stock = stock_file();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
}

/// The Stock resistor row: its library and row ids.
fn stock_resistor(app: &Signex) -> (Uuid, Uuid) {
    let (root, row) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, row)| row.internal_pn.as_str() == "STOCK-R")
        .expect("Stock lists STOCK-R");
    let library = app
        .library
        .open_libraries
        .iter()
        .find(|library| library.root == root)
        .expect("the row's library is open");
    (library.library_id, row.row_id)
}

fn stock_part(id: Uuid, reference: &str, x: f64, (library, row): (Uuid, Uuid)) -> Symbol {
    Symbol {
        library_id: Some(library),
        row_id: Some(row),
        ..part(id, reference, "10k", x)
    }
}

/// R1 and R2 on the open root, R3 on the closed `c.snxsch`, the dialog read.
fn assign_rig() -> (Rig, PathBuf) {
    let mut app = fresh_app();
    mount_stock(&mut app);
    let ids = stock_resistor(&app);
    let root = sheet(
        vec![
            stock_part(R1, "R1", 0.0, ids),
            stock_part(R2, "R2", 20.0, ids),
        ],
        vec![placement(PLACED, "C", "c.snxsch", 60.0)],
    );
    let closed = sheet(vec![stock_part(R3, "R3", 0.0, ids)], Vec::new());
    let mut rig = rig_on(app, root, &[("c.snxsch", closed)], None);
    let path = rig.dir.join("c.snxsch");
    menu(&mut rig.app, "assign_footprints");
    (rig, path)
}

fn assign(app: &mut Signex, msg: AssignFootprintsMsg) {
    settle(app, Message::from(msg));
}

fn row_of(app: &Signex, id: Uuid) -> usize {
    app.ui_state
        .assign_footprints
        .as_ref()
        .expect("dialog open")
        .parts()
        .iter()
        .position(|part| part.symbol_id == id)
        .expect("the part is listed")
}

/// A footprint `row` offers other than the one it has.
fn other_offer(app: &Signex, row: usize) -> String {
    let state = app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("dialog open");
    let part = &state.parts()[row];
    part.offers
        .iter()
        .find(|offer| Some(&offer.binding) != part.current_binding.as_ref())
        .expect("the row offers an alternate")
        .binding
        .clone()
}

fn footprint_on(app: &Signex, path: &Path, id: Uuid) -> String {
    loaded_symbol(app, path, id)
        .expect("the part is loaded")
        .footprint
        .clone()
}

#[test]
fn m80_the_table_lists_open_parts_as_editable_and_a_closed_sheet_as_read_only() {
    let (rig, _closed) = assign_rig();
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.parts().len(), 3);
    let access = |id: Uuid| state.parts()[row_of(&rig.app, id)].access;
    assert_eq!(access(R1), PartAccess::Editable);
    assert_eq!(access(R2), PartAccess::Editable);
    assert_eq!(access(R3), PartAccess::SheetClosed);
}

#[test]
fn m80_apply_sets_the_picked_footprint_and_one_undo_restores_it() {
    let (mut rig, _closed) = assign_rig();
    let root = rig.root.clone();
    let before = footprint_on(&rig.app, &root, R1);
    let untouched = footprint_on(&rig.app, &root, R2);
    let row = row_of(&rig.app, R1);
    let binding = other_offer(&rig.app, row);

    assign(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked { row, binding },
    );
    assert_eq!(
        footprint_on(&rig.app, &root, R1),
        before,
        "a pick changes nothing until Apply"
    );
    assign(&mut rig.app, AssignFootprintsMsg::ApplyRequested);

    assert!(
        rig.app.ui_state.assign_footprints.is_none(),
        "Apply closes it"
    );
    assert_ne!(footprint_on(&rig.app, &root, R1), before);
    assert_eq!(footprint_on(&rig.app, &root, R2), untouched);
    assert!(rig.app.document_state.is_document_dirty(&root));

    undo(&mut rig.app);

    assert_eq!(footprint_on(&rig.app, &root, R1), before);
}

#[test]
fn m80_set_ticked_changes_every_ticked_part_and_leaves_the_closed_sheet() {
    let (mut rig, closed) = assign_rig();
    let root = rig.root.clone();
    let closed_file = std::fs::read(&closed).expect("closed sheet");
    let before = footprint_on(&rig.app, &root, R1);
    assign(&mut rig.app, AssignFootprintsMsg::AllToggled(true));
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    let bar = state.ticked_bar().expect("the ticked bar shows");
    assert_eq!(bar.count, 2, "the closed sheet's part cannot be ticked");
    let binding = other_offer(&rig.app, row_of(&rig.app, R1));

    assign(&mut rig.app, AssignFootprintsMsg::TickedPicked(binding));
    assign(&mut rig.app, AssignFootprintsMsg::ApplyRequested);

    assert_ne!(footprint_on(&rig.app, &root, R1), before);
    assert_eq!(
        footprint_on(&rig.app, &root, R1),
        footprint_on(&rig.app, &root, R2)
    );
    assert_eq!(std::fs::read(&closed).expect("closed sheet"), closed_file);
    assert!(!rig.app.document_state.engines.contains_key(&closed));
}

#[test]
fn m80_cancel_drops_the_picks() {
    let (mut rig, _closed) = assign_rig();
    let root = rig.root.clone();
    let before = footprint_on(&rig.app, &root, R1);
    let row = row_of(&rig.app, R1);
    let binding = other_offer(&rig.app, row);
    assign(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked { row, binding },
    );

    assign(&mut rig.app, AssignFootprintsMsg::CancelRequested);

    assert!(rig.app.ui_state.assign_footprints.is_none());
    assert_eq!(footprint_on(&rig.app, &root, R1), before);
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m80_a_footprint_the_row_does_not_offer_is_refused() {
    let (mut rig, _closed) = assign_rig();
    let row = row_of(&rig.app, R1);

    assign(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row,
            binding: format!("{}:{}", Uuid::from_u128(1), Uuid::from_u128(2)),
        },
    );

    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert!(state.changes().is_empty());
    assert!(!state.can_apply());
}

#[test]
fn m80_a_part_on_a_closed_sheet_takes_no_pick() {
    let (mut rig, _closed) = assign_rig();
    let row = row_of(&rig.app, R3);
    let binding = other_offer(&rig.app, row);

    assign(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked { row, binding },
    );

    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert!(state.changes().is_empty());
}

#[test]
fn m80_the_changed_chip_and_the_filter_list_only_their_rows() {
    let (mut rig, _closed) = assign_rig();
    let row = row_of(&rig.app, R2);
    let binding = other_offer(&rig.app, row);
    assign(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked { row, binding },
    );

    assign(
        &mut rig.app,
        AssignFootprintsMsg::FilterPicked(AssignFilter::Changed),
    );
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.filter_count(AssignFilter::Changed), 1);
    assert_eq!(state.visible_rows(), [row]);

    assign(
        &mut rig.app,
        AssignFootprintsMsg::FilterPicked(AssignFilter::All),
    );
    assign(
        &mut rig.app,
        AssignFootprintsMsg::FilterEdited("R3".to_string()),
    );
    let r3 = row_of(&rig.app, R3);
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.visible_rows(), [r3]);
}

#[test]
fn m80_tick_all_from_this_row_ticks_only_the_parts_that_can_change() {
    let (mut rig, _closed) = assign_rig();
    let row = row_of(&rig.app, R1);

    assign(&mut rig.app, AssignFootprintsMsg::SameRowTicked(row));

    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.ticked_bar().expect("ticked").count, 2);
    assign(&mut rig.app, AssignFootprintsMsg::AllToggled(false));
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert!(state.ticked_bar().is_none(), "Clear ticks hides the bar");
}
