//! History restore of a `.snxlib` tab (#859): the tables are re-read on a
//! worker, through a freshly opened adapter, and applied when it lands.

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{InternalPn, LibraryAdapter, PrimitiveRef};
use uuid::Uuid;

use crate::app::restore_job::RestoreJobDocument;
use crate::app::{Signex, TabInfo, TabKind};
use crate::library::component_preview::test_row;
use crate::library::state::{LibraryDisplaySettings, OpenLibrary};

const TABLE: &str = "R";

/// A real library on disk, mounted and open with EMPTY caches, plus a
/// Library Browser tab on it.
fn app_with_library_tab(dir: &std::path::Path) -> (Signex, std::path::PathBuf, Uuid) {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Lib").join("Lib.snxlib");
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
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init");
    adapter.create_empty_table(TABLE, "seed").expect("table");
    let (mut app, _task) = Signex::new();
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
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: file.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::LibraryBrowser(file.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, file, library_id)
}

/// What a History restore does to the files: a row appears on disk that
/// the open library's caches have not seen.
fn restore_writes_a_row(file: &std::path::Path, library_id: Uuid) {
    let adapter = LocalGitAdapter::open(file).expect("open");
    let mut row = test_row();
    row.internal_pn = InternalPn::new("RESTORED");
    row.symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
    adapter.insert_row(TABLE, row, "restored").expect("insert");
}

fn tab_index(app: &Signex) -> usize {
    app.document_state.active_tab
}

fn cached_pns(app: &Signex, file: &std::path::Path) -> Vec<String> {
    app.library
        .library_at(file)
        .and_then(|lib| lib.tables.get(TABLE))
        .map(|rows| {
            rows.iter()
                .map(|r| r.internal_pn.as_str().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_restored_library_tab_reloads_its_tables_once_the_task_has_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, file, library_id) = app_with_library_tab(dir.path());
    restore_writes_a_row(&file, library_id);

    let idx = tab_index(&app);
    let task = app.apply_reloaded(idx, Ok(RestoreJobDocument::Library), None);

    assert!(
        cached_pns(&app, &file).is_empty(),
        "update must not read the library: nothing is applied before the worker lands"
    );
    crate::app::path_ops::settle_task(&mut app, task);
    assert_eq!(cached_pns(&app, &file), ["RESTORED"]);
}

#[test]
fn a_restored_library_tab_that_cannot_be_reopened_keeps_its_caches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, file, _id) = app_with_library_tab(dir.path());
    {
        use crate::ignore::IgnoreResult;
        crate::diagnostics::init_logging()
            .ignore("another test in this binary already installed the same logger");
    }
    app.library
        .library_at_mut(&file)
        .expect("open")
        .tables
        .insert(TABLE.to_string(), vec![test_row()]);
    // The temp dir is unique per test, so its path picks out this test's
    // entries in the shared diagnostics ring.
    let marker = file.display().to_string();
    // The files are gone, as if the restore had broken the library.
    std::fs::remove_dir_all(file.parent().expect("dir")).expect("remove");

    let idx = tab_index(&app);
    let task = app.apply_reloaded(idx, Ok(RestoreJobDocument::Library), None);
    crate::app::path_ops::settle_task(&mut app, task);

    assert!(app.library.library_at(&file).is_some(), "still mounted");
    assert_eq!(cached_pns(&app, &file), ["U1"], "the previous caches stand");
    let reported = crate::diagnostics::recent_entries()
        .into_iter()
        .any(|entry| {
            entry.message.contains(&marker)
                && entry.message.contains("LocalGitAdapter::open")
                && entry.level == crate::diagnostics::DiagnosticLevel::Warning
        });
    assert!(reported, "the failed reload reaches the Messages panel");
}

#[test]
fn a_restored_library_tab_whose_library_was_closed_meanwhile_is_dropped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, file, library_id) = app_with_library_tab(dir.path());
    restore_writes_a_row(&file, library_id);

    let idx = tab_index(&app);
    let task = app.apply_reloaded(idx, Ok(RestoreJobDocument::Library), None);
    // Closed and opened again (same id, same path) while the read ran.
    // The running read holds a place in the library's write order, which
    // makes `close_library` wait for it; forget that place so the close
    // happens while the read is still running.
    app.library.writes = Default::default();
    app.library.close_library(&file);
    app.library
        .set
        .mount(Box::new(LocalGitAdapter::open(&file).expect("open")))
        .expect("remount");
    app.library.open_libraries.push(OpenLibrary {
        root: file.clone(),
        display_name: "Lib".into(),
        library_id,
        tables: std::collections::HashMap::from([(TABLE.to_string(), vec![test_row()])]),
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
    crate::app::path_ops::settle_task(&mut app, task);

    assert_eq!(
        cached_pns(&app, &file),
        ["U1"],
        "the old mount's read must not land"
    );
}
