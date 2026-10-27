//! #1332 -- a pinned PDF is staged at pick time, written only on Save, and
//! committed with its row; a clone of the library resolves the hash.

use std::path::{Path, PathBuf};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{DatasheetRef, RowId, datasheet_asset_path};
use uuid::Uuid;

use super::*;
use crate::library::component_preview::{apply_inline_edit, test_row};
use crate::library::editor::datasheet_picker::DatasheetMode;
use crate::library::state::{ComponentPreviewState, LibraryDisplaySettings, OpenLibrary};
use crate::library::test_adapters::RevisionedRowAdapter;

const TABLE: &str = "R";
const PDF: &[u8] = b"%PDF-1.7 the datasheet";

/// An app with a real git-backed library holding one row, and a Component
/// Preview of that row.
fn app_with_git_library(dir: &Path) -> (Signex, EditorAddress, PathBuf) {
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
    let mut seed = test_row();
    seed.symbol_ref = signex_library::PrimitiveRef::new(library_id, Uuid::now_v7());
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    let row = adapter
        .read_row(TABLE, RowId::from_uuid(seed.row_id))
        .unwrap();

    let (mut app, _task) = Signex::new();
    app.library.open_libraries.clear();
    app.library.set.mount(Box::new(adapter)).expect("mount");
    open_library(&mut app, &file, library_id, &row);
    let address = EditorAddress::new(file.clone(), TABLE.into(), RowId::from_uuid(row.row_id));
    let preview = ComponentPreviewState::from_row(file.clone(), TABLE.into(), row);
    app.library.editors.insert(address.clone(), preview);
    (app, address, file)
}

fn open_library(
    app: &mut Signex,
    file: &Path,
    library_id: Uuid,
    row: &signex_library::ComponentRow,
) {
    app.library.open_libraries.push(OpenLibrary {
        root: file.to_path_buf(),
        display_name: "Lib".into(),
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
}

/// Pin `PDF` on the row's primary document the way a finished dialog does.
fn pick(app: &mut Signex, address: &EditorAddress, bytes: &[u8]) -> String {
    let state = app.library.editors.get_mut(address).unwrap();
    apply_inline_edit(
        state,
        EditorMsg::Datasheet(DatasheetMsg::SetMode(0, DatasheetMode::PinnedPdf)),
    );
    let picked_for = state.row.documents()[0].clone();
    let file = picked_file(bytes.to_vec(), "ds.pdf".into());
    apply_inline_edit(
        state,
        EditorMsg::Datasheet(DatasheetMsg::UploadResult(Box::new(PickedPdf {
            index: 0,
            picked_for,
            file: Some(file),
        }))),
    );
    signex_library::datasheet_hash(bytes)
}

fn editor<'a>(app: &'a Signex, address: &EditorAddress) -> &'a ComponentPreviewState {
    app.library.editors.get(address).expect("editor open")
}

#[test]
fn a_non_pdf_is_refused_with_its_file_name() {
    let err = picked_file(b"MZ not a pdf".to_vec(), "setup.exe".into()).unwrap_err();
    assert!(
        err.contains("setup.exe") && err.contains("not a PDF"),
        "{err}"
    );
    let ok = picked_file(PDF.to_vec(), "ds.pdf".into()).unwrap();
    assert_eq!(ok.asset.hash, signex_library::datasheet_hash(PDF));
}

#[test]
fn picking_writes_nothing_until_the_row_is_saved() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);

    assert!(editor(&app, &address).dirty);
    assert!(
        !datasheet_asset_path(&file, &hash).unwrap().exists(),
        "no disk write at pick time"
    );
    assert!(!file.parent().unwrap().join("assets").exists());
}

#[test]
fn a_save_commits_the_pdf_with_the_row_and_a_clone_resolves_the_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);

    app.try_save_row(&address).expect("save");

    assert!(!editor(&app, &address).dirty);
    assert!(editor(&app, &address).staged_datasheets.is_empty());
    assert_eq!(
        std::fs::read(datasheet_asset_path(&file, &hash).unwrap()).unwrap(),
        PDF
    );

    let root = file.parent().unwrap();
    let repo = git2::Repository::open(root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let rel = signex_library::datasheet_asset_rel_path(&hash).unwrap();
    assert!(tree.get_path(Path::new(&rel)).is_ok(), "asset in HEAD");

    let clone_dir = tmp.path().join("clone");
    git2::Repository::clone(root.to_str().unwrap(), &clone_dir).unwrap();
    let cloned_file = clone_dir.join("Lib.snxlib");
    let cloned = LocalGitAdapter::open(&cloned_file).unwrap();
    let back = cloned.read_row(TABLE, address.row_id).unwrap();
    let DatasheetRef::HashPinned { hash: pinned, .. } = back.datasheet else {
        panic!("not pinned in the clone");
    };
    assert_eq!(
        std::fs::read(datasheet_asset_path(&cloned_file, &pinned).unwrap()).unwrap(),
        PDF
    );
}

#[test]
fn the_save_button_stores_on_a_worker_then_saves_the_row() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);

    let _task = app.handle_save_row(&address);
    assert!(app.document_state.error_notice.is_none());
    assert!(
        app.document_state.document_saves.is_saving(&file),
        "the PDF write counts as in flight"
    );
    assert!(editor(&app, &address).dirty, "the row waits for the write");

    // What the worker does, then its completion message.
    let staged = editor(&app, &address).staged_assets_in_use();
    let written = store_assets(&file, &staged).unwrap();
    app.finish_staged_asset_store(&address, Ok(written));

    assert!(!app.document_state.document_saves.is_saving(&file));
    assert!(app.document_state.error_notice.is_none());
    assert!(!editor(&app, &address).dirty);
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let rel = signex_library::datasheet_asset_rel_path(&hash).unwrap();
    assert!(tree.get_path(Path::new(&rel)).is_ok());
}

#[test]
fn a_failed_store_leaves_the_row_unsaved_and_reports_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);
    let before = std::fs::read(&file).unwrap();

    let _task = app.handle_save_row(&address);
    app.finish_staged_asset_store(&address, Err("disk full".into()));

    assert!(!app.document_state.document_saves.is_saving(&file));
    assert!(app.document_state.error_notice.is_some());
    assert!(editor(&app, &address).dirty);
    assert_eq!(std::fs::read(&file).unwrap(), before, "row not written");
}

#[test]
fn a_pdf_picked_while_the_store_runs_is_not_pinned_to_a_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);
    let _task = app.handle_save_row(&address);
    let staged = editor(&app, &address).staged_assets_in_use();
    let written = store_assets(&file, &staged).unwrap();

    // A second pick lands before the completion message.
    let late = pick(&mut app, &address, b"%PDF-1.7 a later pick");
    app.finish_staged_asset_store(&address, Ok(written));

    assert!(app.document_state.error_notice.is_some());
    assert!(editor(&app, &address).dirty);
    assert!(!datasheet_asset_path(&file, &late).unwrap().exists());
}

#[test]
fn a_library_that_keeps_no_files_refuses_a_pinned_pdf() {
    let (mut app, _task) = Signex::new();
    app.library.open_libraries.clear();
    let library_id = Uuid::new_v4();
    let adapter = RevisionedRowAdapter::new(library_id, 1);
    let mut row = test_row();
    row.server_revision = Some(1);
    adapter.store(row.clone());
    app.library.set.mount(Box::new(adapter)).expect("mount");
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("srv.snxlib");
    open_library(&mut app, &file, library_id, &row);
    let address = EditorAddress::new(file.clone(), TABLE.into(), RowId::from_uuid(row.row_id));
    app.library.editors.insert(
        address.clone(),
        ComponentPreviewState::from_row(file, TABLE.into(), row),
    );
    pick(&mut app, &address, PDF);

    let err = app.try_save_row(&address).unwrap_err();
    assert!(
        format!("{err:#}").contains("cannot store pinned datasheet"),
        "{err:#}"
    );
    assert!(editor(&app, &address).dirty);
}
