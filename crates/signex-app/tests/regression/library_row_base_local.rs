//! #1108 -- on a local (git-backed) library a dirty Component Preview that
//! saves after a browser cell commit to the same row is refused, and a
//! clean preview follows the commit. Real dispatcher, real `LocalGitAdapter`.

use std::path::PathBuf;

use signex_app::app::{Message, Signex};
use signex_app::library::messages::{EditorMsg, LibraryMessage};
use signex_app::library::state::{EditorAddress, LibraryBrowserState};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, ParamValue, PlmReserved, PrimitiveRef, RowId,
};
use uuid::Uuid;

const TABLE: &str = "ICs";
const PARAM: &str = "Supply Voltage";
/// A `.snxlib` with one row whose `Supply Voltage` is a typed
/// measurement. Version control off — `LocalGitAdapter::open` treats a
/// missing `.git/` as "no version control" and every mutation is a
/// best-effort commit that no-ops, which keeps the fixture cheap while
/// still writing real bytes through the real writer.
fn library_with_a_measurement_row(dir: &std::path::Path) -> (PathBuf, RowId) {
    let snxlib = dir.join("cells.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "cells".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        &snxlib,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init library");
    let lib_id = adapter.library_id();

    let mut parameters = ParamMap::new();
    parameters.insert(
        PARAM.into(),
        ParamValue::Measurement {
            value: 3.3,
            unit: "V".into(),
        },
    );

    let now = chrono::Utc::now();
    let row = ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("SNX-IC-00001"),
        class: ComponentClass::new("IC"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib_id, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme Semi", "SNX-IC-00001"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters,
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: None,
    };
    let row_id = RowId::from_uuid(row.row_id);
    adapter.insert_row(TABLE, row, "seed").expect("insert row");
    drop(adapter);

    (snxlib, row_id)
}

fn pn_on_disk(snxlib: &std::path::Path, row_id: RowId) -> String {
    LocalGitAdapter::open(snxlib)
        .expect("re-open library")
        .read_row(TABLE, row_id)
        .expect("read row")
        .internal_pn
        .to_string()
}

fn open_app_with_preview(snxlib: &std::path::Path, row_id: RowId) -> (Signex, EditorAddress) {
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.to_path_buf())
        .expect("mount library");
    let _ = app.update(Message::Library(LibraryMessage::OpenComponentRow {
        library_path: snxlib.to_path_buf(),
        table: TABLE.to_string(),
        row_id,
    }));
    let address = EditorAddress::new(snxlib.to_path_buf(), TABLE.to_string(), row_id);
    assert!(app.library.editors.contains_key(&address), "preview open");
    (app, address)
}

fn browser_commits_pn(app: &mut Signex, snxlib: &std::path::Path, row_id: RowId, value: &str) {
    let mut browser = LibraryBrowserState::new(snxlib.to_path_buf());
    browser.active_table = Some(TABLE.to_string());
    browser
        .cell_edit
        .insert((row_id, "internal_pn".to_string()), value.to_string());
    app.library
        .library_browsers
        .insert(snxlib.to_path_buf(), browser);
    let _ = app.update(Message::Library(LibraryMessage::BrowserCellCommit {
        library_path: snxlib.to_path_buf(),
        table: TABLE.to_string(),
        row_id,
        column: "internal_pn".to_string(),
    }));
}

fn save_preview(app: &mut Signex, address: &EditorAddress) {
    let _ = app.update(Message::Library(LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg: EditorMsg::SaveDraft,
    }));
}

/// The preview's base must be set and equal what a fresh read carries.
fn assert_base_is_current(snxlib: &std::path::Path, app: &Signex, address: &EditorAddress) {
    let held = app
        .library
        .editors
        .get(address)
        .expect("preview")
        .row
        .server_revision;
    let fresh = LocalGitAdapter::open(snxlib)
        .expect("re-open library")
        .read_row(TABLE, address.row_id)
        .expect("read row")
        .server_revision;
    assert!(held.is_some(), "the preview holds a base");
    assert_eq!(held, fresh, "the preview's base is the stored row's");
}

fn make_dirty(app: &mut Signex, address: &EditorAddress, pn: &str) {
    let state = app.library.editors.get_mut(address).expect("preview");
    state.row.internal_pn = InternalPn::new(pn);
    state.dirty = true;
}

#[test]
fn a_dirty_preview_save_after_a_browser_commit_is_refused_and_the_commit_survives() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-1108-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = library_with_a_measurement_row(tmp.path());
    let (mut app, address) = open_app_with_preview(&snxlib, row_id);

    make_dirty(&mut app, &address, "SNX-PREVIEW");
    browser_commits_pn(&mut app, &snxlib, row_id, "SNX-BROWSER");
    save_preview(&mut app, &address);

    assert_eq!(
        pn_on_disk(&snxlib, row_id),
        "SNX-BROWSER",
        "browser value survives"
    );
    let state = app.library.editors.get(&address).expect("preview");
    assert!(state.dirty, "the draft is kept");
    assert!(state.stale_base, "the Reload offer is raised");
}

#[test]
fn a_clean_preview_follows_a_browser_commit_and_its_next_save_is_accepted() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-1108-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = library_with_a_measurement_row(tmp.path());
    let (mut app, address) = open_app_with_preview(&snxlib, row_id);

    browser_commits_pn(&mut app, &snxlib, row_id, "SNX-BROWSER");
    let state = app.library.editors.get(&address).expect("preview");
    assert_eq!(state.row.internal_pn.as_str(), "SNX-BROWSER");

    make_dirty(&mut app, &address, "SNX-OWN-1");
    save_preview(&mut app, &address);
    assert_eq!(pn_on_disk(&snxlib, row_id), "SNX-OWN-1");
    assert_base_is_current(&snxlib, &app, &address);

    make_dirty(&mut app, &address, "SNX-OWN-2");
    save_preview(&mut app, &address);
    assert_base_is_current(&snxlib, &app, &address);
    assert_eq!(
        pn_on_disk(&snxlib, row_id),
        "SNX-OWN-2",
        "own sequential saves pass"
    );
    assert!(
        !app.library
            .editors
            .get(&address)
            .expect("preview")
            .stale_base
    );
}
