//! #1231 — the Properties panel lists the documents of a placed symbol's
//! library row, read from the open library, primary first, without
//! repeating the symbol's own Datasheet field.

use crate::library_place_symbol::{app_with_library, doc, fixture, place};
use crate::library_symbol_drift::{check_drift, select_first};
use signex_app::app::Signex;
use signex_library::{DatasheetDoc, DatasheetRef};

fn set_row_documents(app: &mut Signex, library_id: uuid::Uuid, row_id: uuid::Uuid) {
    let row = app
        .library
        .open_libraries
        .iter_mut()
        .filter(|lib| lib.library_id == library_id)
        .flat_map(|lib| lib.tables.values_mut().flatten())
        .find(|row| row.row_id == row_id)
        .expect("the placed row is cached");
    row.set_documents(vec![
        DatasheetDoc::new("", DatasheetRef::url("https://ti.example/u1.pdf")),
        DatasheetDoc::new("Errata", DatasheetRef::hash_pinned("ab12", "errata.pdf")),
    ]);
}

fn documents(app: &Signex) -> Vec<(String, String)> {
    app.document_state
        .panel_ctx
        .selection_info
        .iter()
        .filter(|(k, _)| k.starts_with("Document: "))
        .cloned()
        .collect()
}

#[test]
fn a_selected_placed_symbol_lists_its_rows_documents() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    set_row_documents(&mut app, f.library_id, f.row_id);
    select_first(&mut app);
    check_drift(&mut app);
    assert_eq!(
        documents(&app),
        vec![
            (
                "Document: Datasheet".to_string(),
                "https://ti.example/u1.pdf".to_string()
            ),
            ("Document: Errata".to_string(), "errata.pdf".to_string()),
        ]
    );
}

#[test]
fn a_document_equal_to_the_symbols_own_datasheet_is_not_repeated() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    set_row_documents(&mut app, f.library_id, f.row_id);
    let mut sheet = doc(&app).clone();
    sheet.symbols[0].datasheet = "https://ti.example/u1.pdf".into();
    app.document_state
        .active_engine_mut()
        .unwrap()
        .set_document(sheet);
    select_first(&mut app);
    check_drift(&mut app);
    assert_eq!(
        documents(&app),
        vec![("Document: Errata".to_string(), "errata.pdf".to_string())]
    );
}
