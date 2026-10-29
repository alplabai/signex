//! #968 — a row bound to a primitive that does not exist is reported to
//! the Messages panel once, and a saved primitive refreshes the previews
//! that already resolved it.

use signex_app::app::{FileMsg, Message, Signex, SymbolDocument, TabInfo, TabKind};
use signex_app::library::state::{ComponentPreviewState, EditorAddress};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use signex_library::{RefSlot, Symbol, SymbolFile};
use std::path::PathBuf;
use uuid::Uuid;

fn row(pn: &str, library_id: Uuid, symbol: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, symbol),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
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
    }
}

struct Fixture {
    app: Signex,
    _tmp: tempfile::TempDir,
    library_file: PathBuf,
    library_id: Uuid,
}

/// Initialise an empty library named `name` under `dir`; returns its
/// `.snxlib` path and library id.
fn init_library(dir: &std::path::Path, name: &str) -> (PathBuf, Uuid) {
    let library_file = dir.join(name).join(format!("{name}.snxlib"));
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: name.into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(&library_file, manifest, LibraryInitOptions::default()).unwrap();
    (library_file, library_id)
}

fn fixture() -> Fixture {
    signex_library::git_identity::test_identity::install();
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let (library_file, library_id) = init_library(tmp.path(), "Lib");
    let (mut app, _t) = Signex::new();
    // Bootstrap queued mounts for the stock libraries; a sweep waits for
    // those, and these tests drive the sweep themselves.
    app.library.pending_mounts.clear();
    Fixture {
        app,
        _tmp: tmp,
        library_file,
        library_id,
    }
}

fn lines_containing(needle: &str) -> usize {
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(needle))
        .count()
}

#[test]
fn a_dangling_symbol_ref_is_reported_once_with_the_row_and_the_missing_id() {
    let mut f = fixture();
    let missing = Uuid::now_v7();
    let pn = format!("DANGLING-{}", Uuid::now_v7().simple());
    let bad = row(&pn, f.library_id, missing);
    let bad_id = bad.row_id;
    LocalGitAdapter::open(&f.library_file)
        .unwrap()
        .insert_row("R", bad, "seed")
        .unwrap();

    f.app.library.open_library(f.library_file.clone()).unwrap();
    // A later refresh of the same table must not repeat the report.
    f.app.library.refresh_components(&f.library_file).unwrap();
    f.app.library.sweep_dangling_refs();

    assert_eq!(
        lines_containing(&pn),
        1,
        "reported once per appearance, naming the row"
    );
    assert_eq!(
        lines_containing(&missing.to_string()),
        1,
        "names the missing id"
    );
    let lib = f.app.library.library_at(&f.library_file).unwrap();
    assert!(
        lib.dangling.marks_row(bad_id, RefSlot::Symbol),
        "the browser marks the row from this set"
    );
}

#[test]
fn a_row_bound_to_an_existing_symbol_is_not_reported() {
    let mut f = fixture();
    let symbol = Symbol::empty("GOOD");
    let symbol_id = symbol.uuid;
    let dir = f.library_file.parent().unwrap().join("symbols");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("good.snxsym"),
        SymbolFile::from_symbol(symbol).to_toml_string().unwrap(),
    )
    .unwrap();
    let pn = format!("RESOLVED-{}", Uuid::now_v7().simple());
    LocalGitAdapter::open(&f.library_file)
        .unwrap()
        .insert_row("R", row(&pn, f.library_id, symbol_id), "seed")
        .unwrap();

    f.app.library.open_library(f.library_file.clone()).unwrap();

    assert_eq!(lines_containing(&pn), 0);
    let lib = f.app.library.library_at(&f.library_file).unwrap();
    assert!(lib.dangling.refs.is_empty());
}

#[test]
fn saving_a_symbol_refreshes_a_preview_that_already_resolved_it() {
    let mut f = fixture();
    let symbol_path = f
        .library_file
        .parent()
        .unwrap()
        .join("symbols")
        .join("part.snxsym");
    std::fs::create_dir_all(symbol_path.parent().unwrap()).unwrap();
    let file = SymbolFile::from_symbol(Symbol::empty("PREVIEWED"));
    std::fs::write(&symbol_path, file.to_toml_string().unwrap()).unwrap();
    let symbol_id = file.symbols[0].uuid;
    let bound = row("PREVIEW-ROW", f.library_id, symbol_id);
    let row_id = bound.row_id;
    LocalGitAdapter::open(&f.library_file)
        .unwrap()
        .insert_row("R", bound.clone(), "seed")
        .unwrap();
    f.app.library.open_library(f.library_file.clone()).unwrap();

    // A Component Preview tab that has resolved the symbol already.
    let mut preview = ComponentPreviewState::from_row(f.library_file.clone(), "R".into(), bound);
    preview.symbol = f
        .app
        .library
        .set
        .resolve_symbol(&PrimitiveRef::new(f.library_id, symbol_id))
        .unwrap();
    assert_eq!(preview.symbol.as_ref().unwrap().description, "");
    let address = EditorAddress::new(f.library_file.clone(), "R".into(), RowId::from_uuid(row_id));
    f.app.library.editors.insert(address.clone(), preview);

    // Edit and save the symbol in its own editor.
    let mut edited = file.clone();
    edited.symbols[0].description = "edited in the symbol editor".into();
    let mut editor = SymbolDocument::new(symbol_path.clone(), file);
    editor.file = edited;
    editor.mark_dirty();
    f.app
        .document_state
        .symbol_editors
        .insert(symbol_path.clone(), editor);
    f.app.document_state.tabs.push(TabInfo {
        title: "part".into(),
        path: symbol_path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(symbol_path),
    });
    f.app.document_state.active_tab = f.app.document_state.tabs.len() - 1;
    crate::settle_support::settle(&mut f.app, Message::File(FileMsg::Save));

    let shown = f.app.library.editors[&address].symbol.as_ref().unwrap();
    assert_eq!(
        shown.description, "edited in the symbol editor",
        "the preview must draw the saved symbol, not the copy resolved before the save"
    );
}

/// An unreadable `.snxsym` is skipped by the listing, so the symbols in
/// it are undetermined, not missing: the row must not be reported.
#[test]
fn a_binding_into_an_unreadable_symbol_file_is_not_reported_as_missing() {
    let mut f = fixture();
    let dir = f.library_file.parent().unwrap().join("symbols");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.snxsym"), b"not = [valid").unwrap();
    let pn = format!("UNDETERMINED-{}", Uuid::now_v7().simple());
    LocalGitAdapter::open(&f.library_file)
        .unwrap()
        .insert_row("R", row(&pn, f.library_id, Uuid::now_v7()), "seed")
        .unwrap();

    f.app.library.open_library(f.library_file.clone()).unwrap();

    assert_eq!(lines_containing(&pn), 0, "undetermined is not missing");
    let lib = f.app.library.library_at(&f.library_file).unwrap();
    assert!(lib.dangling.refs.is_empty());
    assert!(!lib.dangling.listings_complete);
}

/// A preview whose row binds a symbol in ANOTHER library refreshes when
/// that symbol is saved, even though the preview tab belongs to the
/// first library.
#[test]
fn saving_a_symbol_refreshes_a_preview_bound_across_libraries() {
    let mut f = fixture();
    let (other_file, other_id) = init_library(f._tmp.path(), "Other");
    let symbol_path = other_file
        .parent()
        .unwrap()
        .join("symbols")
        .join("far.snxsym");
    std::fs::create_dir_all(symbol_path.parent().unwrap()).unwrap();
    let file = SymbolFile::from_symbol(Symbol::empty("FAR"));
    std::fs::write(&symbol_path, file.to_toml_string().unwrap()).unwrap();
    let symbol_id = file.symbols[0].uuid;
    let bound = row("CROSS-ROW", other_id, symbol_id);
    let row_id = bound.row_id;
    LocalGitAdapter::open(&f.library_file)
        .unwrap()
        .insert_row("R", bound.clone(), "seed")
        .unwrap();
    f.app.library.open_library(f.library_file.clone()).unwrap();
    f.app.library.open_library(other_file).unwrap();

    let mut preview = ComponentPreviewState::from_row(f.library_file.clone(), "R".into(), bound);
    preview.symbol = f
        .app
        .library
        .set
        .resolve_symbol(&PrimitiveRef::new(other_id, symbol_id))
        .unwrap();
    assert_eq!(preview.symbol.as_ref().unwrap().description, "");
    let address = EditorAddress::new(f.library_file.clone(), "R".into(), RowId::from_uuid(row_id));
    f.app.library.editors.insert(address.clone(), preview);

    let mut edited = file.clone();
    edited.symbols[0].description = "edited in the other library".into();
    let mut editor = SymbolDocument::new(symbol_path.clone(), file);
    editor.file = edited;
    editor.mark_dirty();
    f.app
        .document_state
        .symbol_editors
        .insert(symbol_path.clone(), editor);
    f.app.document_state.tabs.push(TabInfo {
        title: "far".into(),
        path: symbol_path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(symbol_path),
    });
    f.app.document_state.active_tab = f.app.document_state.tabs.len() - 1;
    crate::settle_support::settle(&mut f.app, Message::File(FileMsg::Save));

    let shown = f.app.library.editors[&address].symbol.as_ref().unwrap();
    assert_eq!(shown.description, "edited in the other library");
}
