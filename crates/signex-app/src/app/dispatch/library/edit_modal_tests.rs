//! #1500 — the Edit Component Details modal is reached from the browser
//! action row's "Edit Details" button (`BrowserOpenEditModal`), and an edit
//! applied there persists through the adapter's `update_row`.

use std::path::{Path, PathBuf};

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

fn open_modal(app: &mut Signex, root: &Path, row_id: RowId) {
    let _ = app.update(Message::Library(LibraryMessage::BrowserOpenEditModal {
        library_path: root.to_path_buf(),
        table: TABLE.into(),
        row_id,
    }));
}

fn edit(root: &Path, msg: BrowserEditMsg) -> Message {
    Message::Library(LibraryMessage::BrowserEdit {
        library_path: root.to_path_buf(),
        msg,
    })
}

fn stored(app: &Signex, root: &Path, row_id: RowId) -> ComponentRow {
    app.library
        .library_at(root)
        .and_then(|lib| lib.tables.get(TABLE))
        .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
        .cloned()
        .expect("row cached")
}

#[test]
fn escape_resolves_to_cancel_for_the_open_edit_modal() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);

    let esc = app
        .escape_overlay_message()
        .expect("the open edit modal claims Esc");
    assert!(
        matches!(
            &esc,
            Message::Library(LibraryMessage::BrowserEdit {
                library_path,
                msg: BrowserEditMsg::Cancel,
            }) if *library_path == root
        ),
        "Esc must resolve to the edit modal's Cancel"
    );
    let _ = app.update(esc);
    assert!(app.library.library_browsers[&root].edit_modal.is_none());
}

#[test]
fn save_commits_a_parameter_typed_without_pressing_enter() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamValue {
            key: "param_1".into(),
            value: "4.7".into(),
        },
    ));
    let _ = app.update(edit(&root, BrowserEditMsg::Save));

    assert!(app.library.library_browsers[&root].edit_modal.is_none());
    assert_eq!(
        stored(&app, &root, row_id).parameters.get("param_1"),
        Some(&signex_library::ParamValue::Number(4.7))
    );
}

#[test]
fn save_refuses_an_invalid_uncommitted_parameter_and_keeps_the_modal_open() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamValue {
            key: "param_1".into(),
            value: "nan".into(),
        },
    ));
    let _ = app.update(edit(&root, BrowserEditMsg::Save));

    let modal = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .expect("modal stays open");
    assert!(
        modal
            .error
            .as_deref()
            .is_some_and(|e| e.contains("param_1"))
    );
    assert!(stored(&app, &root, row_id).parameters.is_empty());
}

#[test]
fn a_hash_pinned_datasheet_survives_editing_another_field_and_saving() {
    let (mut app, root, row_id) = app_with_browser();
    let pinned = DatasheetRef::hash_pinned("ab".repeat(32), "r1500.pdf");
    {
        let lib = app
            .library
            .open_libraries
            .iter_mut()
            .find(|l| l.root == root)
            .unwrap();
        lib.tables.get_mut(TABLE).unwrap()[0].datasheet = pinned.clone();
    }
    open_modal(&mut app, &root, row_id);
    // A keystroke routed at the URL setter must not replace the pin.
    let _ = app.update(edit(&root, BrowserEditMsg::SetDatasheetUrl("x".into())));
    let _ = app.update(edit(&root, BrowserEditMsg::SetMpn("R-1500-B".into())));
    let _ = app.update(edit(&root, BrowserEditMsg::Save));

    assert!(app.library.library_browsers[&root].edit_modal.is_none());
    let row = stored(&app, &root, row_id);
    assert_eq!(row.primary_mpn.mpn, "R-1500-B");
    assert_eq!(row.datasheet, pinned);
}
