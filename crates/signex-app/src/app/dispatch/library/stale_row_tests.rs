//! #115 — a row write against a server-backed library is based on the
//! revision the user read. A save after another user's save is refused,
//! and the refusal must reach the user; a save of our own must move the
//! editor's base revision on, or the next save would be refused too.

use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveRef, RowId,
};
use uuid::Uuid;

use crate::app::Signex;
use crate::library::state::{
    ComponentPreviewState, EditorAddress, LibraryDisplaySettings, OpenLibrary,
};
use crate::library::test_adapters::RevisionedRowAdapter;

const TABLE: &str = "R";

fn row_at(revision: u64) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R-115"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "R-115"),
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
        server_revision: Some(revision),
    }
}

/// An app with a server-style library whose row is at `stored` revision,
/// and the user's copy of that row (read at `read`) cached and open in a
/// dirty Component Preview.
fn app_with_row(stored: u64, read: u64) -> (Signex, EditorAddress) {
    let (mut app, _task) = Signex::new();
    let library_id = Uuid::new_v4();
    let root = std::env::temp_dir()
        .join(format!("snx-stale-row-{}", Uuid::new_v4().simple()))
        .join("lib.snxlib");
    app.library.open_libraries.clear();
    app.library
        .set
        .mount(Box::new(RevisionedRowAdapter::new(library_id, stored)))
        .expect("mount stub adapter");
    let row = row_at(read);
    let address = EditorAddress::new(root.clone(), TABLE.into(), RowId::from_uuid(row.row_id));
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "StaleRowLib".into(),
        library_id,
        tables: std::collections::HashMap::from([(TABLE.to_string(), vec![row.clone()])]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    let mut preview = ComponentPreviewState::from_row(root, TABLE.into(), row);
    preview.dirty = true;
    app.library.editors.insert(address.clone(), preview);
    (app, address)
}

fn editor<'a>(app: &'a Signex, address: &EditorAddress) -> &'a ComponentPreviewState {
    app.library.editors.get(address).expect("editor open")
}

#[test]
fn saving_twice_in_a_row_moves_the_base_revision_on() {
    let (mut app, address) = app_with_row(1, 1);

    app.handle_save_row(&address);
    assert!(app.document_state.error_notice.is_none());
    assert_eq!(editor(&app, &address).row.server_revision, Some(2));

    app.library
        .editors
        .get_mut(&address)
        .expect("editor open")
        .dirty = true;
    app.handle_save_row(&address);
    assert!(
        app.document_state.error_notice.is_none(),
        "our own earlier save must not make the next one stale"
    );
    assert_eq!(editor(&app, &address).row.server_revision, Some(3));
}

#[test]
fn a_stale_preview_save_is_refused_visibly_and_keeps_the_edit() {
    let (mut app, address) = app_with_row(2, 1);

    app.handle_save_row(&address);

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("a refused save raises the Save Failed card");
    assert!(
        notice
            .detail
            .contains("reopen the row to load their version"),
        "{}",
        notice.detail
    );
    assert!(editor(&app, &address).dirty, "the edit is kept");
}

#[test]
fn a_stale_browser_delete_reaches_the_messages_panel() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, address) = app_with_row(2, 1);
    let library_path: PathBuf = address.library_path.clone();

    let _ = app.handle_browser_delete_row_confirm(library_path, TABLE.into(), address.row_id);

    let marker = address.row_id.to_string();
    assert!(
        crate::diagnostics::recent_entries().iter().any(|entry| {
            entry.message.contains(&marker)
                && entry
                    .message
                    .contains("reopen the row to load their version")
        }),
        "the refused delete must reach the Messages panel"
    );
}

#[test]
fn a_browser_cell_commit_does_not_make_the_open_preview_save_stale() {
    let (mut app, address) = app_with_row(1, 1);
    let library_path: PathBuf = address.library_path.clone();
    let mut browser = crate::library::state::LibraryBrowserState::new(library_path.clone());
    browser.active_table = Some(TABLE.to_string());
    browser
        .cell_edit
        .insert((address.row_id, "internal_pn".to_string()), "R-115b".into());
    app.library
        .library_browsers
        .insert(library_path.clone(), browser);

    let _ = app.handle_browser_cell_commit(
        library_path,
        TABLE.into(),
        address.row_id,
        "internal_pn".into(),
    );
    assert_eq!(editor(&app, &address).row.server_revision, Some(2));

    app.handle_save_row(&address);
    assert!(
        app.document_state.error_notice.is_none(),
        "the preview must save on top of the cell commit"
    );
    assert_eq!(editor(&app, &address).row.server_revision, Some(3));
}
