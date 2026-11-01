//! #1840 — Save All with a dirty symbol and a dirty Component Preview of a
//! row that symbol binds: the symbol's cascade moves the row first, and the
//! row's own save must still land with the edit and the bumped pin.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{ComponentRow, LibraryAdapter, PrimitiveRef, RowId, Symbol, SymbolFile};
use uuid::Uuid;

use crate::app::contracts::{SaveAllNext, SaveAllPlan};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::library::component_preview::test_row;
use crate::library::state::{ComponentPreviewState, EditorAddress};

const TABLE: &str = "generics";

struct Fixture {
    app: Signex,
    snxlib: PathBuf,
    symbol_path: PathBuf,
    address: EditorAddress,
    library_id: Uuid,
}

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    for message in crate::persist::run_task::outputs(task) {
        let next = app.update(message);
        settle(app, next);
    }
}

fn fixture(dir: &Path) -> Fixture {
    signex_library::git_identity::test_identity::install();
    let snxlib = dir.join("cells").join("cells.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
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
    .expect("init");
    let symbol = Symbol::empty("GATEDRV8");
    let symbol_path = snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("GATEDRV8.snxsym");
    std::fs::create_dir_all(symbol_path.parent().unwrap()).unwrap();
    std::fs::write(
        &symbol_path,
        SymbolFile::from_symbol(symbol.clone())
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    let mut seed = test_row();
    seed.symbol_ref = PrimitiveRef::new(library_id, symbol.uuid);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));

    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.library.open_library(snxlib.clone()).expect("mount");
    let row = app
        .library
        .set
        .get(library_id)
        .unwrap()
        .read_row(TABLE, RowId::from_uuid(seed.row_id))
        .unwrap();
    let address = EditorAddress::new(snxlib.clone(), TABLE.into(), RowId::from_uuid(row.row_id));
    let mut preview = ComponentPreviewState::from_row(snxlib.clone(), TABLE.into(), row);
    preview.row.description = "Mine".into();
    preview.dirty = true;
    app.library.editors.insert(address.clone(), preview);

    let file = SymbolFile::from_bytes(&std::fs::read(&symbol_path).unwrap()).unwrap();
    let mut editor = SymbolDocument::new(symbol_path.clone(), file);
    editor.primitive_mut().name = "GATEDRV8-B".into();
    editor.mark_dirty();
    app.document_state
        .symbol_editors
        .insert(symbol_path.clone(), editor);
    app.document_state.mark_document_dirty(&symbol_path);
    app.document_state.tabs.push(TabInfo {
        title: "GATEDRV8".into(),
        path: symbol_path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(symbol_path.clone()),
    });
    Fixture {
        app,
        snxlib,
        symbol_path,
        address,
        library_id,
    }
}

fn stored(f: &Fixture) -> ComponentRow {
    f.app
        .library
        .set
        .get(f.library_id)
        .unwrap()
        .read_row(TABLE, f.address.row_id)
        .unwrap()
}

#[test]
fn save_all_saves_the_symbol_and_the_row_with_its_edit_on_the_bumped_pin() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let plan = SaveAllPlan {
        context: "Save All".into(),
        paths: vec![f.symbol_path.clone()],
        rows: vec![f.address.clone()],
        next: SaveAllNext::Nothing,
        probes: Default::default(),
        undecided: Vec::new(),
    };

    let task = f.app.begin_save_all_planned(plan);
    settle(&mut f.app, task);

    assert!(
        f.app.document_state.error_notice.is_none(),
        "nothing refused"
    );
    assert!(!f.app.document_state.is_document_dirty(&f.symbol_path));
    let row = stored(&f);
    assert_eq!(row.description, "Mine", "the edit is saved");
    let on_disk = SymbolFile::from_bytes(&std::fs::read(&f.symbol_path).unwrap()).unwrap();
    assert_eq!(on_disk.symbols[0].name, "GATEDRV8-B");
    assert_eq!(
        row.symbol_version, on_disk.symbols[0].version,
        "the bumped pin is kept"
    );
    assert!(!f.app.library.editors[&f.address].dirty);
    let _ = &f.snxlib;
}

#[test]
fn a_row_someone_else_changed_after_the_cascade_is_still_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let task = f
        .app
        .save_primitive_tab_at(&f.symbol_path.clone())
        .expect("starts");
    settle(&mut f.app, task);
    // Another writer (not this app's ledger) changes the row afterwards.
    let adapter = f.app.library.set.get(f.library_id).unwrap();
    let mut other = adapter.read_row(TABLE, f.address.row_id).unwrap();
    other.description = "Theirs".into();
    adapter.update_row(TABLE, other, "other writer").unwrap();

    let task = f.app.handle_save_row(&f.address);
    settle(&mut f.app, task);

    assert_eq!(stored(&f).description, "Theirs", "their change stands");
    assert!(f.app.library.editors[&f.address].dirty, "the edit is kept");
    assert!(f.app.document_state.error_notice.is_some());
}
