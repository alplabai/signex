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
use crate::library::messages::EditorMsg;
use crate::library::refresh::RefreshFollowUp;
use crate::library::state::{
    ComponentPreviewState, EditorAddress, LibraryDisplaySettings, OpenLibrary,
};
use crate::library::test_adapters::RevisionedRowAdapter;
use crate::library::writes::test_support::run_write;

const TABLE: &str = "R";

fn row_at(revision: u64) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R-115"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
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
    let row = row_at(read);
    let adapter = RevisionedRowAdapter::new(library_id, stored);
    adapter.store(row.clone());
    app.library
        .set
        .mount(Box::new(adapter))
        .expect("mount stub adapter");
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
        pricing: Default::default(),
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

    let task = app.handle_save_row(&address);

    run_write(&mut app, task);
    assert!(app.document_state.error_notice.is_none());
    assert_eq!(editor(&app, &address).row.server_revision, Some(2));

    app.library
        .editors
        .get_mut(&address)
        .expect("editor open")
        .dirty = true;
    let task = app.handle_save_row(&address);
    run_write(&mut app, task);
    assert!(
        app.document_state.error_notice.is_none(),
        "our own earlier save must not make the next one stale"
    );
    assert_eq!(editor(&app, &address).row.server_revision, Some(3));
}

#[test]
fn a_stale_preview_save_is_refused_visibly_and_keeps_the_edit() {
    let (mut app, address) = app_with_row(2, 1);

    let task = app.handle_save_row(&address);

    run_write(&mut app, task);

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("a refused save raises the Save Failed card");
    assert!(
        notice.detail.contains("reload to see their version"),
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

    let task = app.handle_browser_delete_row_confirm(library_path, TABLE.into(), address.row_id);

    run_write(&mut app, task);

    let marker = address.row_id.to_string();
    assert!(
        crate::diagnostics::recent_entries().iter().any(|entry| {
            entry.message.contains(&marker) && entry.message.contains("reload to see their version")
        }),
        "the refused delete must reach the Messages panel"
    );
}

/// Another surface (here the browser's cell edit) writes the row's
/// internal part number.
fn commit_cell(app: &mut Signex, address: &EditorAddress, value: &str) {
    let library_path: PathBuf = address.library_path.clone();
    let mut browser = crate::library::state::LibraryBrowserState::new(library_path.clone());
    browser.active_table = Some(TABLE.to_string());
    browser
        .cell_edit
        .insert((address.row_id, "internal_pn".to_string()), value.into());
    app.library
        .library_browsers
        .insert(library_path.clone(), browser);
    let task = app.handle_browser_cell_commit(
        library_path,
        TABLE.into(),
        address.row_id,
        "internal_pn".into(),
    );
    run_write(app, task);
}

fn stored_pn(app: &Signex, address: &EditorAddress) -> String {
    let library_id = app
        .library
        .library_at(&address.library_path)
        .unwrap()
        .library_id;
    app.library
        .set
        .get(library_id)
        .unwrap()
        .read_row(TABLE, address.row_id)
        .unwrap()
        .internal_pn
        .to_string()
}

#[test]
fn a_clean_preview_follows_a_browser_cell_commit_in_content_and_revision() {
    let (mut app, address) = app_with_row(1, 1);
    app.library.editors.get_mut(&address).unwrap().dirty = false;

    commit_cell(&mut app, &address, "R-115b");
    drop(app.land_pending_refresh(&address.library_path));

    let preview = editor(&app, &address);
    assert_eq!(preview.row.internal_pn.as_str(), "R-115b");
    assert_eq!(preview.row.server_revision, Some(2));
    assert!(!preview.dirty);
}

#[test]
fn a_dirty_preview_cannot_overwrite_a_browser_cell_commit() {
    let (mut app, address) = app_with_row(1, 1);

    commit_cell(&mut app, &address, "R-115b");
    assert_eq!(editor(&app, &address).row.server_revision, Some(1));

    let task = app.handle_save_row(&address);

    run_write(&mut app, task);

    assert!(app.document_state.error_notice.is_some(), "save refused");
    assert_eq!(stored_pn(&app, &address), "R-115b", "browser edit survives");
    assert!(editor(&app, &address).dirty, "the draft is kept");
    assert!(editor(&app, &address).stale_base);
}

#[test]
fn reload_from_server_confirms_before_it_drops_a_dirty_draft() {
    let (mut app, address) = app_with_row(1, 1);
    commit_cell(&mut app, &address, "R-115b");
    let task = app.handle_save_row(&address);
    run_write(&mut app, task);
    let msg = |m| (address.clone(), m);

    let (a, m) = msg(EditorMsg::ReloadRequested);
    let task = app.handle_editor_event(a, m);
    run_write(&mut app, task);
    let preview = editor(&app, &address);
    assert!(preview.reload_confirming && preview.dirty, "asks first");
    assert_eq!(preview.row.internal_pn.as_str(), "R-115");

    let (a, m) = msg(EditorMsg::ReloadCancelled);
    let task = app.handle_editor_event(a, m);
    run_write(&mut app, task);
    assert!(!editor(&app, &address).reload_confirming);
    assert!(editor(&app, &address).dirty);

    let (a, m) = msg(EditorMsg::ReloadRequested);
    let task = app.handle_editor_event(a, m);
    run_write(&mut app, task);
    let (a, m) = msg(EditorMsg::ReloadConfirmed);
    let task = app.handle_editor_event(a, m);
    run_write(&mut app, task);
    let preview = editor(&app, &address);
    assert_eq!(preview.row.internal_pn.as_str(), "R-115b");
    assert_eq!(preview.row.server_revision, Some(2));
    assert!(!preview.dirty && !preview.stale_base);

    app.library.editors.get_mut(&address).unwrap().dirty = true;
    app.document_state.error_notice = None;
    let task = app.handle_save_row(&address);
    run_write(&mut app, task);
    assert!(
        app.document_state.error_notice.is_none(),
        "saves after reload"
    );
}

#[test]
fn a_cascade_refreshes_a_clean_preview_and_spares_a_dirty_one() {
    let (mut app, address) = app_with_row(1, 1);
    let library_id = app
        .library
        .library_at(&address.library_path)
        .unwrap()
        .library_id;
    // The cascade rewrote the row on the server.
    let mut bumped = editor(&app, &address).row.clone();
    bumped.symbol_version = "9.9.9".into();
    app.library
        .set
        .get(library_id)
        .unwrap()
        .update_row(TABLE, bumped, "cascade")
        .unwrap();
    let inside = address.library_path.parent().unwrap().join("x.snxsym");

    // Dirty: untouched, and its own save is refused as stale.
    app.refresh_editors_after_cascade(&inside, &[address.row_id]);
    assert_eq!(editor(&app, &address).row.server_revision, Some(1));
    let task = app.handle_save_row(&address);
    run_write(&mut app, task);
    assert!(app.document_state.error_notice.is_some());

    // Clean: follows the cascade.
    app.library.editors.get_mut(&address).unwrap().dirty = false;
    app.refresh_editors_after_cascade(&inside, &[address.row_id]);
    let preview = editor(&app, &address);
    assert_eq!(preview.row.symbol_version, "9.9.9");
    assert_eq!(preview.row.server_revision, Some(2));
}

#[test]
fn the_edit_modal_reloads_the_stored_row_after_a_stale_refusal() {
    use crate::library::messages::BrowserEditMsg;
    let (mut app, address) = app_with_row(2, 1);
    let library_path: PathBuf = address.library_path.clone();
    let mut browser = crate::library::state::LibraryBrowserState::new(library_path.clone());
    let mut draft = editor(&app, &address).row.clone();
    draft.internal_pn = signex_library::InternalPn::new("mine");
    browser.edit_modal = Some(crate::library::state::EditRowModalState::new(
        address.clone(),
        draft,
    ));
    app.library
        .library_browsers
        .insert(library_path.clone(), browser);
    let modal = |app: &Signex| {
        app.library.library_browsers[&library_path]
            .edit_modal
            .clone()
            .expect("modal open")
    };

    let task = app.handle_browser_edit_msg(library_path.clone(), BrowserEditMsg::Save);

    run_write(&mut app, task);
    assert!(modal(&app).stale_base, "a stale save offers the reload");
    assert_eq!(modal(&app).draft.internal_pn.as_str(), "mine");

    let task = app.handle_browser_edit_msg(library_path.clone(), BrowserEditMsg::ReloadRequested);

    run_write(&mut app, task);
    assert!(modal(&app).reload_confirming);
    assert_eq!(modal(&app).draft.internal_pn.as_str(), "mine", "asks first");

    let task = app.handle_browser_edit_msg(library_path.clone(), BrowserEditMsg::ReloadConfirmed);

    run_write(&mut app, task);
    let reloaded = modal(&app);
    assert_eq!(reloaded.draft.internal_pn.as_str(), "R-115");
    assert_eq!(reloaded.draft.server_revision, Some(2));
    assert!(!reloaded.stale_base && !reloaded.reload_confirming);
}

#[test]
fn a_preview_with_a_half_typed_value_is_not_reloaded_over() {
    let (mut app, address) = app_with_row(1, 1);
    {
        let preview = app.library.editors.get_mut(&address).unwrap();
        preview.dirty = false;
        preview.params_edit_buf.insert("tcr".into(), "4.".into());
    }

    commit_cell(&mut app, &address, "R-115b");

    let preview = editor(&app, &address);
    assert_eq!(
        preview.row.internal_pn.as_str(),
        "R-115",
        "typed value kept"
    );
    assert_eq!(
        preview.params_edit_buf.get("tcr").map(String::as_str),
        Some("4.")
    );
    assert_eq!(preview.row.server_revision, Some(1));
}

#[test]
fn a_stale_primitive_bind_from_the_preview_offers_the_reload() {
    let (mut app, address) = app_with_row(2, 1);

    let task = app.apply_primitive_pick_to_preview(
        address.clone(),
        signex_library::PrimitiveKind::Symbol,
        PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
    );

    run_write(&mut app, task);

    assert!(editor(&app, &address).stale_base);
    assert_eq!(editor(&app, &address).row.server_revision, Some(1));
}

/// An off-thread symbol save that reports the rows its cascade moved.
fn primitive_saved_with_cascade(
    inside: PathBuf,
    rows: Vec<RowId>,
) -> crate::persist::PrimitiveSaveDone {
    use crate::persist::{PrimitiveSaveDone, PrimitiveWritten, WrittenContainer};
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
    PrimitiveSaveDone {
        path: inside,
        generation: 0,
        save_as: None,
        result: Ok(PrimitiveWritten {
            file: WrittenContainer::Symbol(file),
            versions: std::collections::HashMap::new(),
            commit_message: "Save symbol S".into(),
            cascaded_rows: rows,
            cascade_owed: Vec::new(),
        }),
    }
}

#[test]
fn an_off_thread_save_refreshes_the_previews_of_the_rows_it_cascaded() {
    let (mut app, address) = app_with_row(1, 1);
    let library_id = app
        .library
        .library_at(&address.library_path)
        .unwrap()
        .library_id;
    let mut bumped = editor(&app, &address).row.clone();
    bumped.symbol_version = "9.9.9".into();
    app.library
        .set
        .get(library_id)
        .unwrap()
        .update_row(TABLE, bumped, "cascade")
        .unwrap();
    let inside = address.library_path.parent().unwrap().join("x.snxsym");

    // A dirty preview keeps its draft, and its own save is refused as
    // stale: the refusal reaches the user.
    let _ = app.handle_primitive_saved(primitive_saved_with_cascade(
        inside.clone(),
        vec![address.row_id],
    ));
    assert_eq!(editor(&app, &address).row.server_revision, Some(1));
    let task = app.handle_save_row(&address);
    run_write(&mut app, task);
    assert!(app.document_state.error_notice.is_some(), "stale refused");

    // A clean one follows the cascade the worker ran.
    app.library.editors.get_mut(&address).unwrap().dirty = false;
    let _ = app.handle_primitive_saved(primitive_saved_with_cascade(inside, vec![address.row_id]));
    let preview = editor(&app, &address);
    assert_eq!(preview.row.symbol_version, "9.9.9");
    assert_eq!(preview.row.server_revision, Some(2));
}

#[test]
fn a_refused_browser_row_bind_refreshes_the_cache_so_a_retry_is_not_refused_again() {
    let (mut app, address) = app_with_row(2, 1);
    let cached_base = |app: &Signex| {
        app.library
            .library_at(&address.library_path)
            .and_then(|lib| lib.tables.get(TABLE))
            .and_then(|rows| rows.first())
            .and_then(|row| row.server_revision)
    };
    assert_eq!(cached_base(&app), Some(1), "the cache starts stale");

    let task = app.apply_primitive_pick_to_browser_row(
        address.clone(),
        signex_library::PrimitiveKind::Symbol,
        PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
    );

    run_write(&mut app, task);

    drop(app.land_pending_refresh(&address.library_path));

    assert_eq!(cached_base(&app), Some(2), "the refusal re-reads the rows");
}

/// The read lands a moment after the write, so the cell shows the saved
/// row at once instead of flashing the old value, and the next commit
/// starts from the saved revision.
#[test]
fn a_browser_cell_commit_shows_the_saved_row_before_the_re_read_lands() {
    let (mut app, address) = app_with_row(1, 1);

    commit_cell(&mut app, &address, "R-115b");

    let cached = app
        .library
        .library_at(&address.library_path)
        .and_then(|lib| lib.tables.get(TABLE))
        .and_then(|rows| rows.first())
        .expect("row cached");
    assert_eq!(cached.internal_pn.as_str(), "R-115b");
    assert_eq!(cached.server_revision, Some(2));
}

/// A re-read that a newer one superseded applies nothing, but the
/// follow-up it was started with is not lost: it runs when the latest
/// read lands. Here row A's clean preview must catch up with the saved
/// row even though row B's request came in before A's read finished.
#[test]
fn a_superseded_refreshs_follow_up_runs_when_the_latest_lands() {
    let (mut app, address) = app_with_row(2, 1);
    app.library.editors.get_mut(&address).unwrap().dirty = false;
    let path = address.library_path.clone();
    let _ = app.refresh_library_task(&path, RefreshFollowUp::CleanEditors(vec![address.row_id]));
    let superseded = app.library.refresh_generations[&path];
    let other_row = RowId::from_uuid(Uuid::new_v4());
    let _ = app.refresh_library_task(&path, RefreshFollowUp::CleanEditors(vec![other_row]));

    // The older read reports first: nothing applies, nothing follows yet.
    drop(app.land_refresh_with_generation(&path, superseded));
    assert_eq!(editor(&app, &address).row.server_revision, Some(1));

    drop(app.land_pending_refresh(&path));
    assert_eq!(
        editor(&app, &address).row.server_revision,
        Some(2),
        "A's clean preview follows the saved row"
    );
}

/// The same, with the committed cells of two rows: the first commit's
/// follow-up must survive the second commit's request.
#[test]
fn two_quick_commits_still_refresh_the_first_rows_clean_preview() {
    let (mut app, address) = app_with_row(1, 1);
    app.library.editors.get_mut(&address).unwrap().dirty = false;
    commit_cell(&mut app, &address, "R-115b");
    // A second commit's request (another row) is made before the first
    // read lands.
    let other_row = RowId::from_uuid(Uuid::new_v4());
    let _ = app.refresh_library_task(
        &address.library_path,
        RefreshFollowUp::CleanEditors(vec![other_row]),
    );

    drop(app.land_pending_refresh(&address.library_path));

    let preview = editor(&app, &address);
    assert_eq!(preview.row.internal_pn.as_str(), "R-115b");
    assert_eq!(preview.row.server_revision, Some(2));
}

/// A failed async refresh with no follow-up (Add Existing on a mounted
/// library, a table add, a retry...) must still reach the Messages panel,
/// once, naming the library.
#[test]
fn a_failed_refresh_without_a_follow_up_warns_once_naming_the_library() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, address) = app_with_row(1, 1);
    let path = address.library_path.clone();
    let name = app
        .library
        .library_at(&path)
        .expect("open")
        .display_name
        .clone();
    let _ = app.refresh_library_task(&path, RefreshFollowUp::Nothing);
    let generation = app.library.refresh_generations[&path];
    let failed = Err(signex_library::LibraryError::Backend("marker-1374".into()));

    drop(app.handle_refresh_finished(
        path,
        generation,
        crate::library::refresh::RefreshSnapshotCell::new(failed),
    ));

    let hits: Vec<_> = crate::diagnostics::recent_entries()
        .into_iter()
        .filter(|entry| entry.message.contains("marker-1374"))
        .collect();
    assert_eq!(hits.len(), 1, "exactly one warning for the failed read");
    assert!(hits[0].message.contains(&name), "it names the library");
}
