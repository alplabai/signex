//! #859 — after a primitive save, the library's caches and the open
//! Component Previews that show the primitive are refreshed by the Tasks
//! `update` returns, end to end through the message loop.

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{PrimitiveRef, RowId, Symbol};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, SymbolDocument, TabInfo, TabKind};
use crate::library::component_preview::test_row;
use crate::library::messages::LibraryMessage;
use crate::library::state::{
    ComponentPreviewState, EditorAddress, LibraryDisplaySettings, OpenLibrary,
};

/// A mounted library, a dirty symbol editor in it, and an open Component
/// Preview that already drew that symbol.
struct Fixture {
    app: crate::app::Signex,
    file: std::path::PathBuf,
    path: std::path::PathBuf,
    address: EditorAddress,
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("Lib").join("Lib.snxlib");
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
    let adapter = LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.library.open_libraries.clear();
    app.library.set.mount(Box::new(adapter)).expect("mount");
    app.library.open_libraries.push(OpenLibrary {
        root: file.clone(),
        display_name: "Lib".into(),
        library_id,
        tables: std::collections::HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });

    // A symbol editor on a symbol that is renamed and saved.
    let path = file
        .parent()
        .expect("dir")
        .join("symbols")
        .join("Sym.snxsym");
    let symbol = Symbol::empty("Before");
    let symbol_uuid = symbol.uuid;
    let mut editor = SymbolDocument::new(
        path.clone(),
        signex_library::SymbolFile::from_symbol(symbol),
    );
    editor.primitive_mut().name = "After".into();
    editor.mark_dirty();
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.mark_document_dirty(&path);
    app.document_state.tabs.push(TabInfo {
        title: "Sym".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });

    // A Component Preview whose row binds that symbol and already drew it.
    let mut row = test_row();
    row.symbol_ref = PrimitiveRef::new(library_id, symbol_uuid);
    let address = EditorAddress::new(file.clone(), "R".into(), RowId::from_uuid(row.row_id));
    let mut preview = ComponentPreviewState::from_row(file.clone(), "R".into(), row);
    preview.symbol = Some(Symbol::empty("Before"));
    app.library.editors.insert(address.clone(), preview);

    Fixture {
        app,
        file,
        path,
        address,
        _dir: dir,
    }
}

#[test]
fn saving_a_primitive_refreshes_the_picker_cache_and_the_open_preview() {
    let Fixture {
        mut app,
        path,
        address,
        _dir,
        ..
    } = fixture();
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    crate::app::path_ops::settle_task(&mut app, task);

    let cached: Vec<_> = app.library.open_libraries[0]
        .cached_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(cached, ["After"], "the picker lists the saved symbol");
    let shown = app.library.editors[&address]
        .symbol
        .as_ref()
        .map(|s| s.name.clone());
    assert_eq!(
        shown.as_deref(),
        Some("After"),
        "the preview draws the saved bytes"
    );
}

/// The preview read holds a place in the library's write order, so a close
/// asked meanwhile waits; whichever read lands last must finish it.
#[test]
fn a_library_closed_while_the_preview_read_runs_closes_when_it_lands() {
    let Fixture {
        mut app,
        file,
        path,
        _dir,
        ..
    } = fixture();
    let save = app.save_primitive_tab_at(&path).expect("save starts");
    let reads: Vec<_> = crate::persist::run_task::outputs(save)
        .into_iter()
        .map(|message| app.update(message))
        .flat_map(crate::persist::run_task::outputs)
        .collect();
    // Another write is queued behind the reads, so the refresh completion
    // finds the library busy; it is gone again when the preview read lands.
    let busy = app.library.writes.turn(&file);
    app.library.close_library(&file);
    assert!(
        app.library.library_at(&file).is_some(),
        "the close waits for the reads"
    );
    let (previews, others): (Vec<_>, Vec<_>) = reads.into_iter().partition(|message| {
        matches!(
            message,
            Message::Library(LibraryMessage::PreviewsReread { .. })
        )
    });
    assert_eq!(previews.len(), 1, "the preview read was started");
    for message in others {
        let task = app.update(message);
        crate::app::path_ops::settle_task(&mut app, task);
    }
    assert!(
        app.library.library_at(&file).is_some(),
        "still busy while the other write is queued"
    );
    drop(busy);
    for message in previews {
        let task = app.update(message);
        crate::app::path_ops::settle_task(&mut app, task);
    }

    assert!(
        app.library.library_at(&file).is_none(),
        "the close finished"
    );
}

/// A background read (the Properties panel's row summary, a preview
/// re-read) must not make the blocking save, used by the Exclude prompt and
/// by project close, fail: it only waits for the read, which cannot corrupt
/// a synchronous write.
#[test]
fn the_blocking_save_is_not_refused_by_a_pending_read() {
    let Fixture {
        mut app,
        file,
        path,
        _dir,
        ..
    } = fixture();
    let read = app.library.writes.read_turn(&file);
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(read);
    });

    let saved = app.save_primitive_tab_blocking(&path);
    release.join().expect("the read ends");

    assert!(saved.is_ok(), "{saved:?}");
    assert!(path.exists(), "the bytes landed");
}

#[test]
fn the_blocking_save_is_still_refused_while_a_write_is_pending() {
    let Fixture {
        mut app,
        file,
        path,
        _dir,
        ..
    } = fixture();
    let _write = app.library.writes.turn(&file);

    let refused = app.save_primitive_tab_blocking(&path).expect_err("refused");

    assert!(
        refused
            .to_string()
            .contains("a library write is still running"),
        "{refused}"
    );
}
