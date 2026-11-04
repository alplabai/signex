//! #2004 — saving a primitive through the app moves the rows of the other
//! mounted libraries that bind it, not only its own library's.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{InternalPn, LibraryAdapter, PrimitiveRef, RowId, Symbol, SymbolFile};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::library::component_preview::test_row;

const TABLE: &str = "generics";

struct Two {
    app: Signex,
    symbol_path: PathBuf,
    far: PathBuf,
    far_id: Uuid,
    far_row: RowId,
}

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    for message in crate::persist::run_task::outputs(task) {
        let next = app.update(message);
        settle(app, next);
    }
}

fn make_library(dir: &Path, name: &str) -> (PathBuf, Uuid, LocalGitAdapter) {
    let snxlib = dir.join(name).join(format!("{name}.snxlib"));
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
    let adapter = LocalGitAdapter::init(
        &snxlib,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init");
    (snxlib, library_id, adapter)
}

/// Library `home` holds a symbol; library `far` holds a row that binds it.
fn two_libraries(dir: &Path) -> Two {
    signex_library::git_identity::test_identity::install();
    let (home, home_id, home_adapter) = make_library(dir, "home");
    let (far, far_id, far_adapter) = make_library(dir, "far");
    let symbol = Symbol::empty("GATEDRV8");
    let symbol_path = home
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
    let mut row = test_row();
    row.internal_pn = InternalPn::new("FAR-1");
    row.symbol_ref = PrimitiveRef::new(home_id, symbol.uuid);
    far_adapter.insert_row(TABLE, row.clone(), "seed").unwrap();
    drop((home_adapter, far_adapter));
    drop(LocalGitAdapter::recover_init(&home).expect("recover_init"));
    drop(LocalGitAdapter::recover_init(&far).expect("recover_init"));

    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.library.open_library(home.clone()).expect("mount home");
    app.library.open_library(far.clone()).expect("mount far");
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
    Two {
        app,
        symbol_path,
        far,
        far_id,
        far_row: RowId::from_uuid(row.row_id),
    }
}

fn far_row(t: &Two) -> signex_library::ComponentRow {
    t.app
        .library
        .set
        .get(t.far_id)
        .unwrap()
        .read_row(TABLE, t.far_row)
        .unwrap()
}

#[test]
fn saving_a_symbol_moves_the_rows_of_another_mounted_library() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path());
    let before = far_row(&t);
    let task = t
        .app
        .save_primitive_tab_at(&t.symbol_path.clone())
        .expect("starts");
    settle(&mut t.app, task);

    let on_disk = SymbolFile::from_bytes(&std::fs::read(&t.symbol_path).unwrap()).unwrap();
    let after = far_row(&t);
    assert_ne!(
        on_disk.symbols[0].version, before.symbol_version,
        "the save bumped the symbol"
    );
    assert_eq!(
        after.symbol_version, on_disk.symbols[0].version,
        "the other library's row follows"
    );
    assert_ne!(after.version, before.version, "and its own version moved");
    let _ = &t.far;
}
