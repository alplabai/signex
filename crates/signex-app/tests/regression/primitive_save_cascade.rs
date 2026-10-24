//! #832 — saving a symbol in the editor bumps the component rows that use
//! it. Only `LocalGitAdapter::save_*` ran the cascade, and the editor does
//! not save through that, so a row kept the old `symbol_version` while the
//! symbol's version moved on and placed instances missed the change.

use signex_app::app::{
    FileMsg, FootprintEditorState, Message, Signex, SymbolEditorState, TabInfo, TabKind,
};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct Fixture {
    app: Signex,
    _tmp: tempfile::TempDir,
    library_file: PathBuf,
    symbol_path: PathBuf,
    users: Uuid,
    bystander: Uuid,
    footprint_path: PathBuf,
    footprint_row: Uuid,
}

fn row(pn: &str, library_id: Uuid, symbol: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, symbol),
        footprint_ref: None,
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
    }
}

/// A mounted library with a symbol `A` (used by row `R-USES`) and a
/// symbol `B` (used by row `R-OTHER`), and an editor open on the file.
fn fixture() -> Fixture {
    signex_library::git_identity::test_identity::install();
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let library_file = tmp.path().join("Lib").join("Lib.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(&library_file, manifest, LibraryInitOptions::default()).unwrap();

    let symbol_path = library_file
        .parent()
        .unwrap()
        .join("symbols")
        .join("pair.snxsym");
    std::fs::create_dir_all(symbol_path.parent().unwrap()).unwrap();
    let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
    file.symbols.push(Symbol::empty("B"));
    std::fs::write(&symbol_path, file.to_toml_string().unwrap()).unwrap();
    let (a, b) = (file.symbols[0].uuid, file.symbols[1].uuid);

    let (mut app, _t) = Signex::new();
    app.library.open_library(library_file.clone()).unwrap();
    let adapter = app.library.set.get(library_id).unwrap();
    let uses = row("R-USES", library_id, a);
    let other = row("R-OTHER", library_id, b);
    let (users, bystander) = (uses.row_id, other.row_id);
    adapter.insert_row("R", uses, "seed").unwrap();
    adapter.insert_row("R", other, "seed").unwrap();

    let footprint_path = library_file
        .parent()
        .unwrap()
        .join("footprints")
        .join("pair.snxfpt");
    std::fs::create_dir_all(footprint_path.parent().unwrap()).unwrap();
    let fp_file = FootprintFile::from_footprint(Footprint::empty("F"));
    std::fs::write(&footprint_path, fp_file.to_toml_string().unwrap()).unwrap();
    let mut fp_row = row("R-FP", library_id, Uuid::now_v7());
    fp_row.footprint_ref = Some(PrimitiveRef::new(library_id, fp_file.footprints[0].uuid));
    fp_row.footprint_version = "0.0.1".into();
    let footprint_row = fp_row.row_id;
    adapter.insert_row("R", fp_row, "seed").unwrap();
    app.document_state.footprint_editors.insert(
        footprint_path.clone(),
        FootprintEditorState::new(footprint_path.clone(), fp_file),
    );

    app.document_state.symbol_editors.insert(
        symbol_path.clone(),
        SymbolEditorState::new(symbol_path.clone(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "pair".into(),
        path: symbol_path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::SymbolEditor(symbol_path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    Fixture {
        app,
        _tmp: tmp,
        library_file,
        symbol_path,
        users,
        bystander,
        footprint_path,
        footprint_row,
    }
}

/// Edit symbol `A` and move its version on to 0.0.4.
fn edit_a(f: &mut Fixture) {
    let editor = f
        .app
        .document_state
        .symbol_editors
        .get_mut(&f.symbol_path)
        .unwrap();
    editor.file.symbols[0].description = "edited".into();
    editor.file.symbols[0].version = "0.0.4".into();
    editor.dirty = true;
}

fn save(app: &mut Signex) {
    let _ = app.update(Message::File(FileMsg::Save));
}

/// The row as a fresh process reads it from disk.
fn row_on_disk(library_file: &Path, id: Uuid) -> ComponentRow {
    LocalGitAdapter::open(library_file)
        .unwrap()
        .read_table("R")
        .unwrap()
        .into_iter()
        .find(|r| r.row_id == id)
        .unwrap()
}

#[test]
fn saving_a_symbol_bumps_the_component_row_that_uses_it() {
    let mut f = fixture();
    edit_a(&mut f);
    save(&mut f.app);

    let bumped = row_on_disk(&f.library_file, f.users);
    assert_eq!(bumped.symbol_version, "0.0.4");
    assert_eq!(bumped.version, "0.0.2");
}

#[test]
fn a_row_that_does_not_use_the_saved_symbol_is_untouched() {
    let mut f = fixture();
    edit_a(&mut f);
    save(&mut f.app);

    let other = row_on_disk(&f.library_file, f.bystander);
    assert_eq!(other.symbol_version, "0.0.1");
    assert_eq!(other.version, "0.0.1");
}

#[test]
fn a_refused_save_does_not_bump_any_row() {
    let mut f = fixture();
    edit_a(&mut f);
    // Another program broke the file since it was opened: the save is
    // refused rather than overwriting it.
    std::fs::write(&f.symbol_path, "not a symbol library").unwrap();
    save(&mut f.app);

    let untouched = row_on_disk(&f.library_file, f.users);
    assert_eq!(untouched.symbol_version, "0.0.1");
    assert_eq!(untouched.version, "0.0.1");
}

#[test]
fn the_messages_panel_reports_the_bumped_rows() {
    let mut f = fixture();
    edit_a(&mut f);
    save(&mut f.app);

    let line = "Bumped 1 component rows that use A to v0.0.4";
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(line)),
        "the Messages panel must say how many rows the save bumped"
    );
}

#[test]
fn saving_a_footprint_bumps_the_component_row_that_uses_it() {
    let mut f = fixture();
    let path = f.footprint_path.clone();
    let editor = f
        .app
        .document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap();
    editor.file.footprints[0].description = "edited".into();
    editor.file.footprints[0].version = "0.0.4".into();
    editor.dirty = true;
    f.app.document_state.tabs.push(TabInfo {
        title: "fp".into(),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    f.app.document_state.active_tab = f.app.document_state.tabs.len() - 1;
    save(&mut f.app);

    let bumped = row_on_disk(&f.library_file, f.footprint_row);
    assert_eq!(bumped.footprint_version, "0.0.4");
    assert_eq!(bumped.version, "0.0.2");
}
