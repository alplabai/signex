//! Mockup row 156: the Table object, its typed cells and CSV import, the Style
//! boxes, bound tables that refresh on compile, and the contents sheet.

use signex_app::active_bar::ActiveBarAction;
use signex_app::app::{CompiledDesignMsg, Message, TablesMsg, Tool};
use signex_app::panels::{AnnotationField, AnnotationPanelMsg, PanelMsg};
use signex_types::schematic::{PinSort, Point, SchDrawing, SchTable, SelectedKind, TableSource};
use uuid::Uuid;

use super::support::{
    Bench, ProjectBench, bench, empty_sheet, net_label, project_bench, sheet_symbol,
};

fn typed_table() -> SchTable {
    SchTable::new_typed(Uuid::now_v7(), Point::new(25.4, 25.4))
}

fn table_bench() -> (Bench, Uuid) {
    let table = typed_table();
    let id = table.uuid;
    let mut sheet = empty_sheet();
    sheet.drawings.push(SchDrawing::Table(table));
    (bench(sheet), id)
}

fn table(b: &Bench) -> &SchTable {
    match &b.sheet().drawings[0] {
        SchDrawing::Table(t) => t,
        other => panic!("not a table: {other:?}"),
    }
}

/// The table alone selected, so its Properties take edits.
fn selected_table() -> (Bench, Uuid) {
    let (mut b, id) = table_bench();
    b.select_everything();
    assert_eq!(
        b.app.document_state.panel_ctx.selected_kind,
        Some(SelectedKind::Drawing),
        "the table alone is selected"
    );
    (b, id)
}

fn type_in(b: &mut Bench, field: AnnotationField, text: &str) {
    b.panel(PanelMsg::Annotation(AnnotationPanelMsg::FieldTyped(
        field,
        text.to_string(),
    )));
}

fn draft(b: &Bench) -> Option<String> {
    b.app
        .document_state
        .panel_ctx
        .annotation
        .draft
        .as_ref()
        .map(|(_, _, text)| text.clone())
}

fn cell(b: &mut Bench, table: Uuid, row: usize, column: usize, text: &str) {
    b.post(Message::from(TablesMsg::CellEdited {
        table,
        row,
        column,
        text: text.to_string(),
    }));
}

#[test]
fn m156_one_click_places_a_table_and_returns_to_select() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::DrawTable);
    assert_eq!(b.app.interaction_state.current_tool, Tool::Table);
    b.click(25.4, 25.4);
    assert!(matches!(
        b.sheet().drawings.as_slice(),
        [SchDrawing::Table(_)]
    ));
    assert_eq!(b.app.interaction_state.current_tool, Tool::Select);
    assert!(b.dirty());
    b.undo();
    assert!(b.sheet().drawings.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m156_a_typed_cell_is_stored_and_undo_restores_it() {
    let (mut b, id) = table_bench();
    cell(&mut b, id, 0, 1, "Configuration");
    assert_eq!(table(&b).rows[0][1], "Configuration");
    assert!(b.dirty());
    b.undo();
    assert_eq!(table(&b).rows[0][1], "");
    assert!(!b.dirty());
}

#[test]
fn m156_a_cell_past_the_last_column_is_refused() {
    let (mut b, id) = table_bench();
    let before = table(&b).clone();
    cell(&mut b, id, 0, 9, "lost");
    assert_eq!(*table(&b), before);
    assert!(!b.dirty());
}

#[test]
fn m156_a_bound_table_takes_no_typed_cell() {
    let mut t = typed_table();
    let id = t.uuid;
    t.source = TableSource::ComponentPins {
        designator: "U1".to_string(),
        sort: PinSort::Number,
    };
    t.columns = SchTable::columns_for(&t.source);
    t.rows = vec![vec!["1".to_string(); t.columns.len()]];
    let mut sheet = empty_sheet();
    sheet.drawings.push(SchDrawing::Table(t));
    let mut b = bench(sheet);
    let before = table(&b).clone();
    cell(&mut b, id, 0, 0, "typed");
    assert_eq!(*table(&b), before);
    assert!(!b.dirty());
}

/// The column titles and rows a CSV read gives, or why it could not.
type CsvRead = Result<(Vec<String>, Vec<Vec<String>>), String>;

fn csv_read(b: &mut Bench, table: Uuid, picked: Option<(std::path::PathBuf, CsvRead)>) {
    let sheet = b.path.clone();
    b.post(Message::from(TablesMsg::CsvImportRead {
        sheet,
        table,
        picked,
    }));
}

#[test]
fn m156_a_csv_import_replaces_the_cells_and_undo_restores_them() {
    let (mut b, id) = table_bench();
    let before = table(&b).clone();
    let cells = (
        vec!["Addr".to_string(), "Register".to_string()],
        vec![vec!["0x00".to_string(), "Configuration".to_string()]],
    );
    csv_read(&mut b, id, Some(("map.csv".into(), Ok(cells))));
    assert_eq!(table(&b).columns.len(), 2);
    assert_eq!(table(&b).columns[0].title, "Addr");
    assert_eq!(table(&b).rows, [["0x00", "Configuration"]]);
    assert!(b.dirty());
    b.undo();
    assert_eq!(*table(&b), before);
    assert!(!b.dirty());
}

#[test]
fn m156_a_cancelled_or_failed_csv_import_leaves_the_table() {
    let (mut b, id) = table_bench();
    let before = table(&b).clone();
    csv_read(&mut b, id, None);
    csv_read(
        &mut b,
        id,
        Some(("map.csv".into(), Err("not readable".to_string()))),
    );
    assert_eq!(*table(&b), before);
    assert!(!b.dirty());
}

#[test]
fn m156_text_size_and_cell_padding_restyle_the_table_and_undo_restores() {
    let (mut b, _) = selected_table();
    let before = table(&b).style;
    type_in(&mut b, AnnotationField::TableTextSize, "2.5");
    assert_eq!(table(&b).style.text_size, 2.5);
    assert!(b.dirty());
    b.undo();
    assert_eq!(table(&b).style, before);
    // Undo clears the selection, and the Properties with it.
    b.select_everything();
    type_in(&mut b, AnnotationField::TableCellPadding, "1.5");
    assert_eq!(table(&b).style.cell_padding, 1.5);
    b.undo();
    assert_eq!(table(&b).style, before);
    assert!(!b.dirty());
}

#[test]
fn m156_letters_in_a_style_box_are_refused_with_the_text_kept() {
    let (mut b, _) = selected_table();
    let before = table(&b).style;
    type_in(&mut b, AnnotationField::TableTextSize, "big");
    assert_eq!(table(&b).style, before);
    assert_eq!(draft(&b).as_deref(), Some("big"));
    assert!(!b.dirty());
}

/// DECISIONS 2026-11-22 names a drawing's size: it must be above zero, and a
/// refused value leaves the document. Reading a table's text size as such a
/// size is an inference.
#[test]
fn m156_a_text_size_of_zero_or_less_is_refused_not_clamped() {
    let (mut b, _) = selected_table();
    let before = table(&b).style;
    for bad in ["0", "-3"] {
        type_in(&mut b, AnnotationField::TableTextSize, bad);
        assert_eq!(table(&b).style, before, "{bad:?} changes nothing");
        assert_eq!(draft(&b).as_deref(), Some(bad), "the box keeps it");
    }
    assert!(!b.dirty());
}

/// DECISIONS 2026-11-22 reads a border as zero or more. Holding cell padding
/// to the same rule is an inference.
#[test]
fn m156_a_negative_cell_padding_is_refused_not_clamped() {
    let (mut b, _) = selected_table();
    let before = table(&b).style;
    type_in(&mut b, AnnotationField::TableCellPadding, "-1");
    assert_eq!(table(&b).style, before);
    assert_eq!(draft(&b).as_deref(), Some("-1"));
    assert!(!b.dirty());
}

fn contents_table() -> SchDrawing {
    let mut t = typed_table();
    t.source = TableSource::ProjectSheets { per_channel: true };
    t.columns = SchTable::columns_for(&t.source);
    t.rows = Vec::new();
    SchDrawing::Table(t)
}

fn contents_bench() -> ProjectBench {
    let mut root = empty_sheet();
    root.child_sheets.push(sheet_symbol("Repeat(CH, 1, 2)"));
    root.drawings.push(contents_table());
    project_bench(root, empty_sheet())
}

fn contents_rows(b: &ProjectBench) -> Vec<Vec<String>> {
    let root = b.root.clone();
    b.doc(&root)
        .drawings
        .iter()
        .find_map(|d| match d {
            SchDrawing::Table(t) => Some(t.rows.clone()),
            _ => None,
        })
        .expect("the contents table")
}

#[test]
fn m156_a_bound_table_refreshes_on_compile_and_not_on_an_edit() {
    let mut b = contents_bench();
    let root = b.root.clone();
    let label = net_label("EN", (10.16, 10.16));
    b.app
        .document_state
        .engines
        .get_mut(&root)
        .expect("the root")
        .execute(signex_engine::Command::PlaceLabel { label })
        .expect("an edit");
    b.send(Message::from(CompiledDesignMsg::FindChanged(String::new())));
    assert!(contents_rows(&b).is_empty(), "an edit does not refresh it");
    b.send(Message::from(CompiledDesignMsg::CompileRequested));
    assert!(!contents_rows(&b).is_empty(), "the compile lists the pages");
}

#[test]
fn m156_add_contents_sheet_writes_the_file_and_adds_it_to_the_project() {
    let mut b = contents_bench();
    b.send(Message::from(TablesMsg::TableOfContentsRequested));
    let written = b.root.with_file_name("contents.snxsch");
    assert!(written.is_file(), "contents.snxsch is written");
    assert!(
        b.data()
            .sheets
            .iter()
            .any(|s| s.filename == "contents.snxsch"),
        "and joins the project"
    );
}

#[test]
fn m156_add_contents_sheet_refuses_a_file_that_is_already_there() {
    let mut b = contents_bench();
    let existing = b.root.with_file_name("contents.snxsch");
    std::fs::write(&existing, "keep me").expect("write");
    b.send(Message::from(TablesMsg::TableOfContentsRequested));
    assert_eq!(
        std::fs::read_to_string(&existing).expect("read"),
        "keep me",
        "the file is kept"
    );
    assert!(
        !b.data()
            .sheets
            .iter()
            .any(|s| s.filename == "contents.snxsch")
    );
}
