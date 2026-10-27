//! #1500 — the Edit Component Details modal is reached from the browser
//! action row's "Edit Details" button (`BrowserOpenEditModal`), and an edit
//! applied there persists through the adapter's `update_row`.

use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveRef, RowId,
};
use uuid::Uuid;

use crate::app::{Message, Signex};
use crate::library::messages::{BrowserEditMsg, LibraryMessage};
use crate::library::state::{LibraryBrowserState, LibraryDisplaySettings, OpenLibrary};
use crate::library::test_adapters::RevisionedRowAdapter;

const TABLE: &str = "R";

fn row() -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R-1500"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "R-1500"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: Some(1),
    }
}

fn app_with_browser() -> (Signex, PathBuf, RowId) {
    let (mut app, _task) = Signex::new();
    let library_id = Uuid::new_v4();
    let root = std::env::temp_dir()
        .join(format!("snx-edit-modal-{}", Uuid::new_v4().simple()))
        .join("lib.snxlib");
    app.library.open_libraries.clear();
    let row = row();
    let row_id = RowId::from_uuid(row.row_id);
    let adapter = RevisionedRowAdapter::new(library_id, 1);
    adapter.store(row.clone());
    app.library
        .set
        .mount(Box::new(adapter))
        .expect("mount stub adapter");
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "EditModalLib".into(),
        library_id,
        tables: std::collections::HashMap::from([(TABLE.to_string(), vec![row])]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        pricing: Default::default(),
    });
    app.library
        .library_browsers
        .insert(root.clone(), LibraryBrowserState::new(root.clone()));
    (app, root, row_id)
}

#[test]
fn edit_details_opens_the_modal_for_that_row_and_save_persists() {
    let (mut app, root, row_id) = app_with_browser();
    assert!(app.library.library_browsers[&root].edit_modal.is_none());

    let _ = app.update(Message::Library(LibraryMessage::BrowserOpenEditModal {
        library_path: root.clone(),
        table: TABLE.into(),
        row_id,
    }));
    let modal = app.library.library_browsers[&root]
        .edit_modal
        .clone()
        .expect("the modal opens");
    assert_eq!(modal.address.row_id, row_id);
    assert_eq!(modal.draft.internal_pn.as_str(), "R-1500");

    let edit = |msg| {
        Message::Library(LibraryMessage::BrowserEdit {
            library_path: root.clone(),
            msg,
        })
    };
    let _ = app.update(edit(BrowserEditMsg::SetInternalPn("R-1500-B".into())));
    let _ = app.update(edit(BrowserEditMsg::Save));

    assert!(
        app.library.library_browsers[&root].edit_modal.is_none(),
        "a successful save closes the modal"
    );
    let stored = app
        .library
        .library_at(&root)
        .and_then(|lib| lib.tables.get(TABLE))
        .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
        .expect("row still cached");
    assert_eq!(stored.internal_pn.as_str(), "R-1500-B");
}

#[test]
fn escape_rung_cancels_the_open_edit_modal() {
    let (mut app, root, row_id) = app_with_browser();
    let _ = app.update(Message::Library(LibraryMessage::BrowserOpenEditModal {
        library_path: root.clone(),
        table: TABLE.into(),
        row_id,
    }));
    let _ = app.update(Message::Library(LibraryMessage::BrowserEdit {
        library_path: root.clone(),
        msg: BrowserEditMsg::Cancel,
    }));
    assert!(app.library.library_browsers[&root].edit_modal.is_none());
}
