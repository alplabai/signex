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
fn a_pdf_picked_while_the_store_runs_is_not_saved_and_the_editor_stays_dirty() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let first = pick(&mut app, &address, PDF);
    let _task = app.handle_save_row(&address);
    let staged = editor(&app, &address).staged_assets_in_use();
    let written = store_assets(&file, &staged).unwrap();

    // A second pick lands before the completion message.
    let late = pick(&mut app, &address, b"%PDF-1.7 a later pick");
    app.finish_staged_asset_store(&address, Ok(written));

    assert!(app.document_state.error_notice.is_none());
    assert!(editor(&app, &address).dirty, "the later pick is unsaved");
    assert!(!datasheet_asset_path(&file, &late).unwrap().exists());
    let stored = app
        .library
        .set
        .get(app.library.library_at(&file).unwrap().library_id)
        .unwrap()
        .read_row(TABLE, address.row_id)
        .unwrap();
    assert_eq!(
        stored.datasheet,
        DatasheetRef::hash_pinned(first.clone(), "ds.pdf"),
        "the row as it was when Save was pressed"
    );
    assert!(editor(&app, &address).staged_datasheets.contains_key(&late));
    assert!(editor(&app, &address).saving_row.is_none());

    // The snapshot's PDF is in HEAD, and the stored row pins nothing HEAD
    // lacks (#1332).
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let rel = |h: &str| signex_library::datasheet_asset_rel_path(h).unwrap();
    assert!(tree.get_path(Path::new(&rel(&first))).is_ok());
    for doc in stored.documents() {
        if let DatasheetRef::HashPinned { hash, .. } = doc.source {
            assert!(
                tree.get_path(Path::new(&rel(&hash))).is_ok(),
                "{hash} pinned without a committed file"
            );
        }
    }
}

#[test]
fn a_primitive_bind_while_a_save_runs_is_refused_with_a_message() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);
    let _task = app.handle_save_row(&address);
    assert!(editor(&app, &address).saving_row.is_some());
    let library_id = app.library.library_at(&file).unwrap().library_id;

    app.apply_primitive_pick_to_preview(
        address.clone(),
        signex_library::PrimitiveKind::Footprint,
        signex_library::PrimitiveRef::new(library_id, Uuid::now_v7()),
    );

    let state = editor(&app, &address);
    assert!(state.row.footprint_ref.is_none(), "the bind was refused");
    assert!(state.saving_row.is_some(), "the save is untouched");
}

#[test]
fn a_second_save_and_a_reload_wait_for_the_running_save() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);
    let _first = app.handle_save_row(&address);
    assert!(editor(&app, &address).saving_row.is_some());
    let _second = app.handle_save_row(&address);
    assert!(
        app.document_state.document_saves.is_saving(&file),
        "one write in flight"
    );
    let _ = app.handle_editor_event(address.clone(), EditorMsg::ReloadConfirmed);
    assert!(
        editor(&app, &address).dirty,
        "reload did not replace the copy being saved"
    );
    // Save All / quit does not race the worker either.
    assert!(app.try_save_row(&address).is_err());
}

#[test]
fn a_written_pdf_the_saved_row_does_not_pin_is_not_committed() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let _task = app.handle_save_row(&address);
    let staged = editor(&app, &address).staged_assets_in_use();
    let mut written = store_assets(&file, &staged).unwrap();
    let orphan = signex_library::store_datasheet(&file, b"%PDF-1.7 orphan").unwrap();
    written.push(signex_library::datasheet_asset_rel_path(&orphan).unwrap());
    app.finish_staged_asset_store(&address, Ok(written));

    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let rel = |h: &str| signex_library::datasheet_asset_rel_path(h).unwrap();
    assert!(tree.get_path(Path::new(&rel(&hash))).is_ok());
    assert!(tree.get_path(Path::new(&rel(&orphan))).is_err());
}

#[test]
fn a_failed_commit_keeps_the_editor_owed_and_a_retry_commits_the_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let head = file.parent().unwrap().join(".git").join("HEAD");
    let good_head = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"not a ref").unwrap();

    let err = app.try_save_row(&address).unwrap_err();
    assert!(format!("{err:#}").contains("not saved"), "{err:#}");
    let state = editor(&app, &address);
    assert!(state.dirty && state.staged_datasheets.contains_key(&hash));

    std::fs::write(&head, good_head).unwrap();
    app.try_save_row(&address).expect("retry");
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
fn binding_a_primitive_never_saves_a_staged_hash_without_its_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let library_id = app.library.library_at(&file).unwrap().library_id;

    app.apply_primitive_pick_to_preview(
        address.clone(),
        signex_library::PrimitiveKind::Footprint,
        signex_library::PrimitiveRef::new(library_id, Uuid::now_v7()),
    );

    let stored = app
        .library
        .set
        .get(library_id)
        .unwrap()
        .read_row(TABLE, address.row_id)
        .unwrap();
    assert!(stored.footprint_ref.is_some(), "the binding is saved");
    assert_ne!(
        stored.datasheet,
        DatasheetRef::hash_pinned(hash, "ds.pdf"),
        "the unsaved pin is not persisted"
    );
    for doc in stored.documents() {
        if let DatasheetRef::HashPinned { hash: pinned, .. } = doc.source {
            assert!(
                datasheet_asset_path(&file, &pinned).unwrap().is_file(),
                "{pinned} pinned without its file"
            );
        }
    }
    // The preview keeps its unsaved pin, and can still save it.
    assert!(editor(&app, &address).dirty);
    app.try_save_row(&address).expect("save");
    assert!(!editor(&app, &address).dirty);
}

fn event(address: &EditorAddress, msg: EditorMsg) -> Message {
    Message::Library(LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    })
}

#[test]
fn the_messages_route_through_update() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let state = app.library.editors.get_mut(&address).unwrap();
    apply_inline_edit(
        state,
        EditorMsg::Datasheet(DatasheetMsg::SetMode(0, DatasheetMode::PinnedPdf)),
    );
    let picked_for = state.row.documents()[0].clone();
    let upload = |file| {
        event(
            &address,
            EditorMsg::Datasheet(DatasheetMsg::UploadResult(Box::new(PickedPdf {
                index: 0,
                picked_for: picked_for.clone(),
                file: Some(file),
            }))),
        )
    };

    // A refused pick records its reason and pins nothing.
    let _ = app.update(upload(picked_file(b"MZ".to_vec(), "x.exe".into())));
    assert!(editor(&app, &address).datasheet_errors.contains_key(&0));
    assert!(editor(&app, &address).staged_datasheets.is_empty());

    // A PDF is staged, and still written nowhere.
    let _ = app.update(upload(picked_file(PDF.to_vec(), "ds.pdf".into())));
    let hash = signex_library::datasheet_hash(PDF);
    assert!(editor(&app, &address).staged_datasheets.contains_key(&hash));
    assert!(!datasheet_asset_path(&file, &hash).unwrap().exists());

    // Save, then the worker's completion arrives as a message.
    let _ = app.update(event(&address, EditorMsg::SaveDraft));
    assert!(editor(&app, &address).saving_row.is_some());
    let staged = editor(&app, &address).staged_assets_in_use();
    let written = store_assets(&file, &staged).unwrap();
    let _ = app.update(event(
        &address,
        EditorMsg::Datasheet(DatasheetMsg::AssetsStored(Ok(written))),
    ));

    assert!(!editor(&app, &address).dirty);
    assert!(!app.document_state.document_saves.is_saving(&file));
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
fn save_all_and_close_library_write_a_staged_pdf_with_its_row() {
    use crate::app::contracts::{SaveAllNext, SaveAllPlan};
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let _ = app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: Vec::new(),
        rows: vec![address.clone()],
        next: SaveAllNext::Quit,
        probes: Default::default(),
        undecided: Vec::new(),
    });
    assert!(!editor(&app, &address).dirty);
    assert!(datasheet_asset_path(&file, &hash).unwrap().is_file());

    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, b"%PDF-1.7 closing");
    let _ = app.update(Message::Library(LibraryMessage::ConfirmCloseLibrary {
        library_path: file.clone(),
        dirty_editors: vec![address.clone()],
    }));
    let _ = app.update(Message::Library(LibraryMessage::CloseLibraryConfirm(
        crate::library::messages::CloseLibraryChoice::SaveAll,
    )));
    assert!(datasheet_asset_path(&file, &hash).unwrap().is_file());
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
fn a_file_over_the_size_cap_is_refused_before_it_is_hashed() {
    let mut big = b"%PDF-1.7".to_vec();
    big.resize(signex_library::MAX_DATASHEET_BYTES as usize + 1, 0);
    let err = picked_file(big, "big.pdf".into()).unwrap_err();
    assert!(err.contains("big.pdf") && err.contains("larger"), "{err}");
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

    for result in [
        app.try_save_row(&address),
        app.begin_save_row(&address).map(|_| ()),
    ] {
        let err = result.unwrap_err();
        assert!(
            format!("{err:#}").contains("cannot store pinned datasheet"),
            "{err:#}"
        );
    }
    assert!(editor(&app, &address).dirty);
    assert!(editor(&app, &address).saving_row.is_none());
    assert!(
        !tmp.path().join("assets").exists(),
        "refused before any file was written"
    );
}
