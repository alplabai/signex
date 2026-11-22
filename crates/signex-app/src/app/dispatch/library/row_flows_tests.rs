//! #2773, #2778, #2780: Properties follows the selected row, New Library
//! refuses a library already at the path, and the Broken Binding choices act
//! on their row.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{InternalPn, LibraryAdapter, PrimitiveKind, PrimitiveRef, RowId};
use uuid::Uuid;

use crate::app::path_ops::settle_task;
use crate::app::{Message, Signex};
use crate::library::component_preview::test_row;
use crate::library::messages::LibraryMessage;
use crate::library::recovery::{BrokenBindingChoice, RecoveryDialog};
use crate::library::state::LibraryCreateOptionsState;

const TABLE: &str = "R";

/// A library on disk at `path` with `count` rows.
fn library_on_disk(path: &Path, count: usize) -> Vec<RowId> {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "parts".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let options = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(path, manifest, options).expect("init library");
    (0..count)
        .map(|n| {
            let mut row = test_row();
            row.internal_pn = InternalPn::new(format!("R-{n}"));
            row.symbol_ref = PrimitiveRef::new(adapter.library_id(), Uuid::now_v7());
            row.symbol_version = "0.0.1".into();
            let id = RowId::from_uuid(row.row_id);
            adapter.insert_row(TABLE, row, "seed").expect("insert row");
            id
        })
        .collect()
}

fn fresh_app() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.library.pending_mounts.clear();
    app
}

fn run(app: &mut Signex, msg: LibraryMessage) {
    let task = app.update(Message::Library(msg));
    settle_task(app, task);
}

/// An app with the library mounted and its Browser tab in front.
fn browser(count: usize, read_only: bool) -> (tempfile::TempDir, PathBuf, Vec<RowId>, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("parts.snxlib");
    let rows = library_on_disk(&root, count);
    let mut app = fresh_app();
    let mounted = if read_only {
        app.library.open_library_read_only(root.clone())
    } else {
        app.library.open_library(root.clone())
    };
    mounted.expect("mount");
    run(&mut app, LibraryMessage::OpenLibraryBrowser(root.clone()));
    (dir, root, rows, app)
}

fn select(app: &mut Signex, root: &Path, row_id: RowId) {
    run(
        app,
        LibraryMessage::BrowserSelectRow {
            library_path: root.to_path_buf(),
            table: TABLE.to_string(),
            row_id,
        },
    );
}

fn sheet_row(app: &Signex) -> Option<Uuid> {
    let detail = app.document_state.panel_ctx.library_row_detail.as_ref();
    detail.map(|detail| detail.row_id)
}

#[test]
fn properties_follows_the_selection_and_empties_when_it_clears() {
    let (_dir, root, rows, mut app) = browser(2, false);

    select(&mut app, &root, rows[0]);
    assert_eq!(sheet_row(&app), Some(rows[0].as_uuid()));

    select(&mut app, &root, rows[1]);
    assert_eq!(sheet_row(&app), Some(rows[1].as_uuid()));

    run(
        &mut app,
        LibraryMessage::BrowserSelectTable {
            library_path: root,
            table: TABLE.to_string(),
        },
    );
    assert_eq!(sheet_row(&app), None, "no row is left to pick for");
}

/// A New Library dialog on a library that is already on disk, not mounted.
fn dialog_on_existing() -> (tempfile::TempDir, PathBuf, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let lib = dir.path().join("taken.snxlib");
    library_on_disk(&lib, 0);
    let mut app = fresh_app();
    app.library.create_options = Some(LibraryCreateOptionsState {
        project_path: dir.path().join("p.snxprj"),
        lib_path: lib.clone(),
        enable_git: false,
        use_lfs: false,
        refusal: None,
        existing_library: false,
    });
    (dir, lib, app)
}

#[test]
fn an_existing_library_stays_refused_when_version_control_is_toggled() {
    let (_dir, _lib, mut app) = dialog_on_existing();
    run(&mut app, LibraryMessage::LibraryCreateOptionsConfirm);

    let reason = |app: &Signex| {
        let state = app.library.create_options.as_ref().expect("still open");
        state.refusal.clone().expect("still refused")
    };
    assert!(reason(&app).contains("taken.snxlib"), "{}", reason(&app));

    run(&mut app, LibraryMessage::LibraryCreateOptionsToggleGit);
    assert!(reason(&app).contains("taken.snxlib"));
}

#[test]
fn open_existing_library_closes_the_dialog_and_mounts_it() {
    let (_dir, lib, mut app) = dialog_on_existing();
    run(&mut app, LibraryMessage::LibraryCreateOptionsConfirm);

    run(
        &mut app,
        LibraryMessage::LibraryCreateOptionsOpenExistingRequested,
    );

    assert!(app.library.create_options.is_none());
    assert!(app.library.library_at(&lib).is_some());
}

fn broken_binding(app: &mut Signex, root: &Path, row_id: RowId) {
    app.library.recovery = Some(RecoveryDialog::BrokenPrimitiveBinding {
        library_path: root.to_path_buf(),
        table: TABLE.to_string(),
        row_id: row_id.as_uuid(),
        internal_pn: None,
        kind: PrimitiveKind::Symbol,
        missing_uuid: Uuid::now_v7(),
    });
}

fn row_count(app: &Signex, root: &Path) -> usize {
    let library = app.library.library_at(root).expect("mounted");
    library.tables.get(TABLE).map_or(0, Vec::len)
}

#[test]
fn remove_row_on_a_read_only_library_is_refused_and_keeps_the_row() {
    let (_dir, root, rows, mut app) = browser(1, true);
    broken_binding(&mut app, &root, rows[0]);

    run(
        &mut app,
        LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::RemoveRow),
    );

    assert!(app.library.recovery.is_none());
    assert_eq!(row_count(&app, &root), 1);
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn rebind_on_a_read_only_library_opens_no_picker() {
    let (_dir, root, rows, mut app) = browser(1, true);
    broken_binding(&mut app, &root, rows[0]);

    run(
        &mut app,
        LibraryMessage::RecoveryBrokenBinding(BrokenBindingChoice::Rebind),
    );

    assert!(app.library.primitive_picker.is_none());
    assert!(app.document_state.error_notice.is_some());
}
