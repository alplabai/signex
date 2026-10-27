//! #994 / #967 — New Component must never be a silent no-op, and a row
//! created without a symbol / footprint must be visibly unbound.
//!
//! #994: with no library open (or a library whose adapter is not
//! mounted, so no table can be resolved) the menu command used to set a
//! `new_component` modal state that nothing rendered. It now reports why
//! to the Messages panel.
//!
//! #967: creating a row never mints a `.snxsym` / `.snxfpt`, so the row
//! starts unbound. The creation is reported once to the Messages panel,
//! the row reports itself as unbound, and binding a symbol clears it.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_app::app::{Message, Signex};
use signex_app::diagnostics::{self, DiagnosticEntry};
use signex_app::library::messages::{LibraryMessage, PrimitivePickerMsg};
use signex_app::library::state::{
    EditorAddress, LibraryDisplaySettings, OpenLibrary, PrimitivePickerTarget,
};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{ComponentRow, PrimitiveKind, PrimitiveRef, RowId};
use uuid::Uuid;

/// Messages-panel records written after `after_id` that contain `needle`.
fn records_since(after_id: Option<u64>, needle: &str) -> Vec<DiagnosticEntry> {
    diagnostics::recent_entries()
        .into_iter()
        .filter(|e| after_id.is_none_or(|id| e.id > id))
        .filter(|e| e.message.contains(needle))
        .collect()
}

fn init_library(dir: &std::path::Path) -> PathBuf {
    let snxlib = dir.join("fresh.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "fresh".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(
        &snxlib,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init library");
    snxlib
}

fn only_row(app: &Signex, root: &std::path::Path) -> (String, ComponentRow) {
    let lib = app.library.library_at(root).expect("library is open");
    let mut found = lib
        .tables
        .iter()
        .flat_map(|(table, rows)| rows.iter().map(move |r| (table.clone(), r.clone())));
    let first = found.next().expect("the new row is in the library cache");
    assert!(found.next().is_none(), "exactly one row was created");
    first
}

/// Every `.snxsym` / `.snxfpt` under the library's `symbols/` and
/// `footprints/` directories (a missing directory holds none).
fn minted_primitive_files(root: &std::path::Path) -> Vec<PathBuf> {
    ["symbols", "footprints"]
        .iter()
        .filter_map(|dir| std::fs::read_dir(root.join(dir)).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "snxsym" || ext == "snxfpt")
        })
        .collect()
}

#[test]
fn new_component_with_no_library_open_says_why() {
    let _ = diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    app.library.open_libraries.clear();
    let before = diagnostics::newest_entry_id();

    let _ = app.update(Message::Library(LibraryMessage::NewComponent));

    let records = records_since(
        before,
        "no library is open. Open a library with a component table first",
    );
    assert_eq!(records.len(), 1, "silent no-op: {records:?}");
}

#[test]
fn new_component_on_a_library_without_a_table_says_why() {
    let _ = diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    app.library.open_libraries.clear();
    // Listed as open but no adapter mounted: no manifest, so no table.
    app.library.open_libraries.push(OpenLibrary {
        root: PathBuf::from("unmounted.snxlib"),
        display_name: "Unmounted".into(),
        library_id: Uuid::new_v4(),
        tables: HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        dangling: Default::default(),
        pricing: Default::default(),
    });
    let before = diagnostics::newest_entry_id();

    let _ = app.update(Message::Library(LibraryMessage::NewComponent));

    let records = records_since(before, "library \"Unmounted\" is not loaded");
    assert_eq!(records.len(), 1, "silent no-op: {records:?}");
}

#[test]
fn a_new_row_is_unbound_reported_once_and_binding_clears_it() {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::Builder::new()
        .prefix("signex-967-new-row-")
        .tempdir()
        .expect("tempdir");
    let snxlib = init_library(tmp.path());
    let (mut app, _boot) = Signex::new();
    app.library.open_libraries.clear();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    let before = diagnostics::newest_entry_id();

    let _ = app.update(Message::Library(LibraryMessage::NewComponent));

    let (table, row) = only_row(&app, &snxlib);
    assert!(row.is_symbol_unbound(), "no symbol was picked");
    assert!(row.is_footprint_unbound(), "no footprint was picked");
    let notes = records_since(before, "has no symbol and no footprint bound");
    assert_eq!(notes.len(), 1, "one Messages-panel note: {notes:?}");
    assert!(notes[0].message.contains("Pick Symbol / Pick Footprint"));
    assert_eq!(
        minted_primitive_files(tmp.path()),
        Vec::<PathBuf>::new(),
        "creating a row must not mint a symbol or footprint file"
    );

    // Bind a symbol through the real picker flow.
    let row_id = RowId::from_uuid(row.row_id);
    let address = EditorAddress::new(snxlib.clone(), table.clone(), row_id);
    let _ = app.update(Message::Library(LibraryMessage::OpenPrimitivePicker {
        kind: PrimitiveKind::Symbol,
        target: PrimitivePickerTarget::BrowserRow(address),
    }));
    let library_id = app.library.library_at(&snxlib).expect("open").library_id;
    let picked = PrimitiveRef::new(library_id, Uuid::now_v7());
    let _ = app.update(Message::Library(LibraryMessage::PrimitivePicker(
        PrimitivePickerMsg::Pick(picked),
    )));
    // The re-read the bind started, landing as the runtime would.
    app.land_pending_refresh(
        &snxlib,
        signex_app::library::refresh::RefreshFollowUp::Nothing,
    );

    let (_, bound) = only_row(&app, &snxlib);
    assert!(!bound.is_symbol_unbound(), "binding must clear the marker");
    assert!(bound.is_footprint_unbound(), "footprint is still unbound");
}
