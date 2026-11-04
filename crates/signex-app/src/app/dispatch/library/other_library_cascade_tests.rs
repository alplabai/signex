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
fn two_libraries(dir: &Path, edited: bool) -> Two {
    two_libraries_with(dir, edited, false)
}

fn two_libraries_with(dir: &Path, edited: bool, far_read_only: bool) -> Two {
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
    if far_read_only {
        app.library
            .open_library_read_only(far.clone())
            .expect("mount far read-only");
    } else {
        app.library.open_library(far.clone()).expect("mount far");
    }
    let file = SymbolFile::from_bytes(&std::fs::read(&symbol_path).unwrap()).unwrap();
    let mut editor = SymbolDocument::new(symbol_path.clone(), file);
    if edited {
        editor.primitive_mut().name = "GATEDRV8-B".into();
        editor.mark_dirty();
    }
    app.document_state
        .symbol_editors
        .insert(symbol_path.clone(), editor);
    if edited {
        app.document_state.mark_document_dirty(&symbol_path);
    }
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
    let mut t = two_libraries(tmp.path(), true);
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

fn symbol_uuid(t: &Two) -> Uuid {
    t.app.document_state.symbol_editors[&t.symbol_path]
        .primitive()
        .uuid
}

/// What a failed cascade leaves on the editor: the symbol is saved, the far
/// row is still at its old pin.
fn owing_editor(t: &mut Two) {
    let uuid = symbol_uuid(t);
    t.app
        .document_state
        .symbol_editors
        .get_mut(&t.symbol_path)
        .unwrap()
        .cascade_owed = vec![uuid];
}

#[test]
fn closing_an_editor_that_owes_rows_keeps_the_debt_for_the_next_save() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), false);
    owing_editor(&mut t);
    let uuid = symbol_uuid(&t);
    let _ = t.app.close_tab_now(0);
    assert!(
        !t.app
            .document_state
            .symbol_editors
            .contains_key(&t.symbol_path)
    );

    let file = SymbolFile::from_bytes(&std::fs::read(&t.symbol_path).unwrap()).unwrap();
    let pin = file.symbols[0].version.clone();
    assert_ne!(far_row(&t).symbol_version, pin, "the far row is behind");
    t.app.open_symbol_editor_tab(t.symbol_path.clone(), file);

    let editor = &t.app.document_state.symbol_editors[&t.symbol_path];
    assert_eq!(
        editor.cascade_owed,
        vec![uuid],
        "the reopened editor owes it"
    );
    assert!(editor.is_dirty(), "and reads unsaved until it is paid");
    let task = t
        .app
        .save_primitive_tab_at(&t.symbol_path.clone())
        .expect("starts");
    settle(&mut t.app, task);
    assert_eq!(
        far_row(&t).symbol_version,
        pin,
        "the next save moved the row"
    );
    let editor = &t.app.document_state.symbol_editors[&t.symbol_path];
    assert!(editor.cascade_owed.is_empty(), "nothing is owed any more");
}

#[test]
fn a_save_carries_what_the_file_kept_even_when_its_editor_was_not_reopened_by_the_app() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), false);
    let uuid = symbol_uuid(&t);
    // The debt sits in the file's record; the open editor never saw it.
    t.app.library.owed_cascades.set(&t.symbol_path, &[uuid]);
    t.app
        .document_state
        .symbol_editors
        .get_mut(&t.symbol_path)
        .unwrap()
        .mark_dirty();
    let pin = SymbolFile::from_bytes(&std::fs::read(&t.symbol_path).unwrap())
        .unwrap()
        .symbols[0]
        .version
        .clone();
    let task = t
        .app
        .save_primitive_tab_at(&t.symbol_path.clone())
        .expect("starts");
    settle(&mut t.app, task);
    assert_eq!(far_row(&t).symbol_version, pin);
    assert!(t.app.library.owed_cascades.of(&t.symbol_path).is_empty());
}

/// The pin the open library's cached row reads, as the Components panel sees it.
fn cached_far_pin(t: &Two) -> String {
    let lib = t.app.library.library_at(&t.far).expect("far is mounted");
    lib.tables[TABLE]
        .iter()
        .find(|row| row.row_id == t.far_row.as_uuid())
        .expect("the row is cached")
        .symbol_version
        .clone()
}

fn assert_far_cache_current(t: &Two) {
    let pin = SymbolFile::from_bytes(&std::fs::read(&t.symbol_path).unwrap())
        .unwrap()
        .symbols[0]
        .version
        .clone();
    assert_eq!(
        cached_far_pin(t),
        pin,
        "the other library's cached row reads the new pin"
    );
    let lib = t.app.library.library_at(&t.far).unwrap();
    assert_eq!(
        lib.observed_stamp,
        signex_library::DiskStamp::of(&t.far).ok(),
        "its observed stamp is the one on disk, so the poll sees no outside edit"
    );
}

#[test]
fn the_other_librarys_cache_is_reloaded_after_the_cascade() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), true);
    t.app
        .library
        .refresh_components(&t.far)
        .expect("fill cache");
    t.app.library.drift.check_pending = false;
    let old_pin = cached_far_pin(&t);
    let task = t
        .app
        .save_primitive_tab_at(&t.symbol_path.clone())
        .expect("starts");
    settle(&mut t.app, task);
    assert_ne!(cached_far_pin(&t), old_pin, "the cache moved");
    assert_far_cache_current(&t);
}

#[test]
fn the_other_librarys_cache_is_reloaded_after_a_blocking_cascade() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), true);
    t.app
        .library
        .refresh_components(&t.far)
        .expect("fill cache");
    t.app.library.drift.check_pending = false;
    let old_pin = cached_far_pin(&t);
    t.app
        .save_primitive_tab_blocking(&t.symbol_path.clone())
        .expect("saves");
    assert_ne!(cached_far_pin(&t), old_pin, "the cache moved");
    assert_far_cache_current(&t);
    // No update ran after the save, so the pending drift check is still set.
    assert!(t.app.library.drift.check_pending, "and drift is re-checked");
}

/// A far library that is briefly unreadable (a share lock, a pull in
/// progress) is a failure a retry can clear: its rows stay owed.
#[cfg(unix)]
#[test]
fn a_far_library_that_cannot_be_read_for_now_keeps_the_cascade_owed() {
    use std::os::unix::fs::PermissionsExt;
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), true);
    let uuid = symbol_uuid(&t);
    let mode = std::fs::metadata(&t.far).unwrap().permissions().mode();
    std::fs::set_permissions(&t.far, std::fs::Permissions::from_mode(0o000)).unwrap();
    // A sandbox that ignores the mode (root) cannot make the error.
    if std::fs::read(&t.far).is_ok() {
        std::fs::set_permissions(&t.far, std::fs::Permissions::from_mode(mode)).unwrap();
        return;
    }
    let task = t
        .app
        .save_primitive_tab_at(&t.symbol_path.clone())
        .expect("starts");
    settle(&mut t.app, task);
    std::fs::set_permissions(&t.far, std::fs::Permissions::from_mode(mode)).unwrap();
    let editor = &t.app.document_state.symbol_editors[&t.symbol_path];
    assert_eq!(
        editor.cascade_owed,
        vec![uuid],
        "the far rows are still owed"
    );
    assert!(t.app.document_state.is_document_dirty(&t.symbol_path));
    let pin = SymbolFile::from_bytes(&std::fs::read(&t.symbol_path).unwrap())
        .unwrap()
        .symbols[0]
        .version
        .clone();
    assert_ne!(far_row(&t).symbol_version, pin, "and not moved");
}

#[test]
fn a_read_only_library_is_not_cascaded_into() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries_with(tmp.path(), true, true);
    let before = far_row(&t).symbol_version;
    let task = t
        .app
        .save_primitive_tab_at(&t.symbol_path.clone())
        .expect("starts");
    settle(&mut t.app, task);
    assert_eq!(
        far_row(&t).symbol_version,
        before,
        "its rows are left alone"
    );
}

/// Two busy libraries share the one wait: the save gives up after about
/// `limit`, not after `limit` for each.
#[test]
fn the_blocking_save_waits_once_for_all_busy_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), true);
    let (far2, _, adapter) = make_library(tmp.path(), "far2");
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&far2).expect("recover_init"));
    t.app
        .library
        .open_library(far2.clone())
        .expect("mount far2");
    // A write the worker never runs holds each library's place in line.
    let held = [
        t.app.library.writes.turn(&t.far),
        t.app.library.writes.turn(&far2),
    ];
    let limit = std::time::Duration::from_millis(500);
    let started = std::time::Instant::now();
    let result = t
        .app
        .save_primitive_tab_blocking_within(&t.symbol_path.clone(), limit);
    let took = started.elapsed();
    drop(held);
    assert!(result.is_ok(), "the primitive itself is saved: {result:?}");
    assert!(
        took < limit * 3 / 2,
        "waited {took:?} for two libraries; one deadline of {limit:?} covers both"
    );
}

/// A write for an editor that was replaced while it ran (closed and
/// reopened) still pays the debt the reopened editor was handed.
#[test]
fn a_replaced_editors_write_clears_the_debt_it_paid() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), false);
    owing_editor(&mut t);
    let uuid = symbol_uuid(&t);
    t.app
        .document_state
        .symbol_editors
        .get_mut(&t.symbol_path)
        .unwrap()
        .mark_dirty();
    let task = t
        .app
        .save_primitive_tab_at(&t.symbol_path.clone())
        .expect("starts");
    // Closed and reopened while the write runs: the debt follows the file.
    t.app.keep_owed_cascade(&t.symbol_path);
    t.app.document_state.symbol_editors.remove(&t.symbol_path);
    let file = SymbolFile::from_bytes(&std::fs::read(&t.symbol_path).unwrap()).unwrap();
    t.app.document_state.symbol_editors.insert(
        t.symbol_path.clone(),
        SymbolDocument::new(t.symbol_path.clone(), file),
    );
    t.app.seed_owed_cascade(&t.symbol_path);
    let reopened = &t.app.document_state.symbol_editors[&t.symbol_path];
    assert_eq!(
        reopened.cascade_owed,
        vec![uuid],
        "the reopened editor owes it"
    );

    settle(&mut t.app, task);
    let editor = &t.app.document_state.symbol_editors[&t.symbol_path];
    assert!(editor.cascade_owed.is_empty(), "the write paid it");
    assert!(!editor.is_dirty());
    assert!(!t.app.document_state.is_document_dirty(&t.symbol_path));
}

#[test]
fn quitting_names_the_files_that_still_owe_rows() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let mut t = two_libraries(tmp.path(), false);
    owing_editor(&mut t);
    let _ = t.app.flush_quit_writes_then(iced::Task::none());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Quit with rows still awaiting")
                && e.message.contains("GATEDRV8"))
    );
}
