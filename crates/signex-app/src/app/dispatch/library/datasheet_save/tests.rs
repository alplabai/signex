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
use crate::library::writes::test_support::run_task;
use crate::library::writes::test_support::run_write;

const TABLE: &str = "R";
const PDF: &[u8] = b"%PDF-1.7 the datasheet";

/// An app with a real git-backed library holding one row, and a Component
/// Preview of that row.
fn app_with_git_library(dir: &Path) -> (Signex, EditorAddress, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.library.open_libraries.clear();
    let (address, file) = add_git_library(&mut app, dir);
    (app, address, file)
}

/// Mount another git-backed library under `dir`, with a Component Preview
/// of its one row.
fn add_git_library(app: &mut Signex, dir: &Path) -> (EditorAddress, PathBuf) {
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

    app.library.set.mount(Box::new(adapter)).expect("mount");
    open_library(app, &file, library_id, &row);
    let address = EditorAddress::new(file.clone(), TABLE.into(), RowId::from_uuid(row.row_id));
    let preview = ComponentPreviewState::from_row(file.clone(), TABLE.into(), row);
    app.library.editors.insert(address.clone(), preview);
    (address, file)
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

/// Run `task` the way the iced runtime does, feeding each message back.
fn settle(app: &mut Signex, task: iced::Task<Message>) {
    for message in crate::persist::run_task::outputs(task) {
        let next = app.update(message);
        settle(app, next);
    }
}

/// The Save button, with the worker's store and the commit that follows it.
fn save_row_to_completion(app: &mut Signex, address: &EditorAddress) {
    let task = app.handle_save_row(address);
    settle(app, task);
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

    save_row_to_completion(&mut app, &address);

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
    let task = app.finish_staged_asset_store(
        &address,
        editor(&app, &address).saving_generation,
        Ok(written),
    );
    run_task(&mut app, task);

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
    let task = app.finish_staged_asset_store(
        &address,
        editor(&app, &address).saving_generation,
        Err("disk full".into()),
    );
    run_task(&mut app, task);

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
    let task = app.finish_staged_asset_store(
        &address,
        editor(&app, &address).saving_generation,
        Ok(written),
    );
    run_task(&mut app, task);

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

    let task = app.apply_primitive_pick_to_preview(
        address.clone(),
        signex_library::PrimitiveKind::Footprint,
        signex_library::PrimitiveRef::new(library_id, Uuid::now_v7()),
    );

    run_write(&mut app, task);

    let state = editor(&app, &address);
    assert!(state.row.footprint_ref.is_none(), "the bind was refused");
    assert!(state.saving_row.is_some(), "the save is untouched");
}

fn stored_row(app: &Signex, file: &Path, address: &EditorAddress) -> signex_library::ComponentRow {
    app.library
        .set
        .get(app.library.library_at(file).unwrap().library_id)
        .unwrap()
        .read_row(TABLE, address.row_id)
        .unwrap()
}

fn head_has(file: &Path, hash: &str) -> bool {
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let rel = signex_library::datasheet_asset_rel_path(hash).unwrap();
    tree.get_path(Path::new(&rel)).is_ok()
}

/// #1471 -- the editor is closed while the PDFs are written and a new one is
/// opened on the same row. The late completion must not commit the new
/// editor's live row (it was never saved) or mark it clean.
#[test]
fn a_store_finishing_after_the_editor_was_reopened_saves_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);
    let _task = app.update(event(&address, EditorMsg::SaveDraft));
    assert!(editor(&app, &address).saving_row.is_some());
    let staged = editor(&app, &address).staged_assets_in_use();
    let written = store_assets(&file, &staged).unwrap();
    let generation = editor(&app, &address).saving_generation;
    let before = stored_row(&app, &file, &address);

    // Close, then reopen the row with a fresh unsaved edit.
    app.library.editors.remove(&address);
    let mut reopened = ComponentPreviewState::from_row(file.clone(), TABLE.into(), before.clone());
    reopened.row.datasheet_label = "UNSAVED EDIT".into();
    reopened.dirty = true;
    app.library.editors.insert(address.clone(), reopened);

    let _ = app.update(event(
        &address,
        EditorMsg::Datasheet(DatasheetMsg::AssetsStored {
            generation,
            result: Ok(written),
        }),
    ));

    assert!(
        !app.document_state.document_saves.is_saving(&file),
        "the in-flight mark is released"
    );
    assert!(app.document_state.error_notice.is_none());
    let state = editor(&app, &address);
    assert!(state.dirty, "the reopened editor is still unsaved");
    assert_eq!(state.row.datasheet_label, "UNSAVED EDIT");
    assert_eq!(
        stored_row(&app, &file, &address).datasheet_label,
        before.datasheet_label,
        "nothing was committed"
    );
}

/// #1471 -- a save whose commit and restore write both failed leaves the
/// pinned PDF on disk and untracked while the `.snxlib` pins it. After a
/// Reload nothing is staged, yet saving that row must still commit the file.
#[test]
fn a_save_after_a_double_failure_commits_the_untracked_pinned_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = signex_library::datasheet_hash(PDF);

    // The state the double failure leaves: the row pins the hash (written
    // to the library), the PDF sits on disk, and HEAD holds neither the
    // pin's file nor an editor that stages it.
    let library_id = app.library.library_at(&file).unwrap().library_id;
    let mut pinned = stored_row(&app, &file, &address);
    pinned.datasheet = DatasheetRef::hash_pinned(hash.clone(), "ds.pdf");
    app.library
        .set
        .get(library_id)
        .unwrap()
        .update_row(TABLE, pinned, "pin without its file")
        .unwrap();
    store_assets(
        &file,
        &[signex_library::PdfAsset::from_bytes(PDF.to_vec()).unwrap()],
    )
    .unwrap();
    assert!(!head_has(&file, &hash));

    // Reload builds a fresh editor with nothing staged; then an edit.
    let task = app.reload_editor_from_server(&address, false);
    crate::app::path_ops::settle_task(&mut app, task);
    assert!(editor(&app, &address).staged_datasheets.is_empty());
    let state = app.library.editors.get_mut(&address).unwrap();
    state.row.datasheet_label = "edited after reload".into();
    state.dirty = true;
    let task = app.update(event(&address, EditorMsg::SaveDraft));
    run_task(&mut app, task);

    assert!(app.document_state.error_notice.is_none());
    assert!(!editor(&app, &address).dirty);
    assert!(head_has(&file, &hash), "the pinned PDF is committed");
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
    let task = app.handle_editor_event(address.clone(), EditorMsg::ReloadConfirmed);
    run_write(&mut app, task);
    assert!(
        editor(&app, &address).dirty,
        "reload did not replace the copy being saved"
    );
    // Save All / quit does not race the worker either.
    assert!(app.begin_save_row_in_batch(&address, 1).is_err());
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
    let task = app.finish_staged_asset_store(
        &address,
        editor(&app, &address).saving_generation,
        Ok(written),
    );
    run_task(&mut app, task);

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

/// #858: the row and its PDF are written, so a failed commit is a saved row
/// that is not committed, not a refused save; "Commit now" records the PDF.
#[test]
fn a_failed_commit_still_saves_the_row_and_commit_now_commits_the_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let head = file.parent().unwrap().join(".git").join("HEAD");
    let good_head = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"not a ref").unwrap();

    save_row_to_completion(&mut app, &address);
    assert!(app.document_state.error_notice.is_none());
    assert!(!editor(&app, &address).dirty, "the row is saved");
    app.library.sync_uncommitted();
    let rel = signex_library::datasheet_asset_rel_path(&hash).unwrap();
    let pending = app
        .library
        .uncommitted_in(&file)
        .expect("uncommitted state");
    assert!(pending.paths.contains(&rel), "{pending:?}");
    assert!(datasheet_asset_path(&file, &hash).unwrap().is_file());

    std::fs::write(&head, good_head).unwrap();
    let library_id = app.library.library_at(&file).unwrap().library_id;
    let outcome = app
        .library
        .set
        .get(library_id)
        .unwrap()
        .commit_pending("")
        .unwrap();
    assert_eq!(outcome, signex_library::CommitOutcome::Committed);
    app.library.sync_uncommitted();
    assert!(app.library.uncommitted_in(&file).is_none());
    assert!(head_has(&file, &hash));
}

#[test]
fn binding_a_primitive_never_saves_a_staged_hash_without_its_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let library_id = app.library.library_at(&file).unwrap().library_id;

    let task = app.apply_primitive_pick_to_preview(
        address.clone(),
        signex_library::PrimitiveKind::Footprint,
        signex_library::PrimitiveRef::new(library_id, Uuid::now_v7()),
    );

    run_write(&mut app, task);

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
    save_row_to_completion(&mut app, &address);
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
    let generation = editor(&app, &address).saving_generation;
    let task = app.update(event(
        &address,
        EditorMsg::Datasheet(DatasheetMsg::AssetsStored {
            generation,
            result: Ok(written),
        }),
    ));
    run_task(&mut app, task);

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

/// A Save All over the row, as quit does it (nothing but the row to save).
fn start_save_all(app: &mut Signex, address: &EditorAddress) -> iced::Task<Message> {
    use crate::app::contracts::{SaveAllNext, SaveAllPlan};
    app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: Vec::new(),
        rows: vec![address.clone()],
        next: SaveAllNext::Quit,
        probes: Default::default(),
        undecided: Vec::new(),
    })
}

/// Run a Save All's task: the store completions, then the settle message,
/// which quits only when every row saved and otherwise raises the
/// "stayed open" card.
fn run_save_all(app: &mut Signex, task: iced::Task<Message>) {
    settle(app, task);
}

#[test]
fn save_all_stores_the_staged_pdf_off_the_update_and_then_quits() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);

    let task = start_save_all(&mut app, &address);
    // `update` wrote nothing: the row waits on the worker.
    assert!(!datasheet_asset_path(&file, &hash).unwrap().exists());
    assert!(editor(&app, &address).saving_row.is_some());
    assert!(app.document_state.document_saves.is_saving(&file));
    // A second Save All meanwhile is refused for that row, not doubled.
    let second = start_save_all(&mut app, &address);
    run_save_all(&mut app, second);
    let notice = app.document_state.error_notice.take().expect("a card");
    assert!(
        format!("{notice:?}").contains("still being saved"),
        "{notice:?}"
    );
    assert!(
        app.document_state.document_saves.is_saving(&file),
        "the first store still runs"
    );
    assert!(editor(&app, &address).saving_row.is_some());

    run_save_all(&mut app, task);
    assert!(datasheet_asset_path(&file, &hash).unwrap().is_file());
    assert!(!editor(&app, &address).dirty);
    assert!(head_has(&file, &hash));
    assert!(!app.document_state.document_saves.is_saving(&file));
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn save_all_with_a_failing_store_reports_it_and_does_not_quit() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    // A file where the assets directory belongs makes the store fail.
    std::fs::write(file.parent().unwrap().join("assets"), b"x").unwrap();

    let task = start_save_all(&mut app, &address);
    run_save_all(&mut app, task);

    let state = editor(&app, &address);
    assert!(state.dirty && state.saving_row.is_none());
    assert!(
        state.staged_datasheets.contains_key(&hash),
        "the PDF is still staged"
    );
    let notice = app.document_state.error_notice.as_ref().expect("a card");
    assert!(format!("{notice:?}").contains("stayed open"), "{notice:?}");
    assert!(
        app.library.batch_row_failures.is_empty(),
        "taken by the settle"
    );
    assert!(!app.document_state.document_saves.is_saving(&file));
}

fn close_library_save_all(app: &mut Signex, address: &EditorAddress) -> iced::Task<Message> {
    let _ = app.update(Message::Library(LibraryMessage::ConfirmCloseLibrary {
        library_path: address.library_path.clone(),
        dirty_editors: vec![address.clone()],
    }));
    app.update(Message::Library(LibraryMessage::CloseLibraryConfirm(
        crate::library::messages::CloseLibraryChoice::SaveAll,
    )))
}

#[test]
fn close_library_stores_the_staged_pdf_off_the_update_then_closes() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, b"%PDF-1.7 closing");

    let task = close_library_save_all(&mut app, &address);
    assert!(!datasheet_asset_path(&file, &hash).unwrap().exists());
    assert!(app.library.library_at(&file).is_some(), "still open");

    settle(&mut app, task);
    assert!(datasheet_asset_path(&file, &hash).unwrap().is_file());
    assert!(head_has(&file, &hash));
    assert!(app.library.library_at(&file).is_none(), "closed");
}

#[test]
fn close_library_with_a_failing_store_keeps_the_library_and_the_draft() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    std::fs::write(file.parent().unwrap().join("assets"), b"x").unwrap();

    let task = close_library_save_all(&mut app, &address);
    settle(&mut app, task);

    assert!(
        app.library.library_at(&file).is_some(),
        "the library stays open"
    );
    let state = editor(&app, &address);
    assert!(state.dirty && state.staged_datasheets.contains_key(&hash));
    let notice = app.document_state.error_notice.as_ref().expect("a card");
    assert!(format!("{notice:?}").contains("not closed"), "{notice:?}");
}

#[test]
fn close_library_prompts_again_for_a_row_edited_while_its_pdf_was_stored() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);

    let task = close_library_save_all(&mut app, &address);
    let state = app.library.editors.get_mut(&address).unwrap();
    state.row.datasheet_label = "typed while the PDF was being stored".into();
    state.dirty = true;
    settle(&mut app, task);

    assert!(
        app.library.library_at(&file).is_some(),
        "not closed over an edit"
    );
    assert!(app.library.close_library_confirm.is_some(), "asked again");
}

/// Break the repository's HEAD so the row commit fails after the PDF store.
fn break_head(file: &Path) -> Vec<u8> {
    let head = file.parent().unwrap().join(".git").join("HEAD");
    let good = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"not a ref").unwrap();
    good
}

#[test]
fn save_all_where_the_store_works_but_the_commit_fails_still_saves() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let good_head = break_head(&file);

    let task = start_save_all(&mut app, &address);
    run_save_all(&mut app, task);

    assert!(datasheet_asset_path(&file, &hash).unwrap().is_file());
    assert!(!editor(&app, &address).dirty, "the row is saved");
    assert!(app.document_state.error_notice.is_none());
    assert!(app.library.batch_row_failures.is_empty());
    app.library.sync_uncommitted();
    assert!(app.library.uncommitted_in(&file).is_some());
    std::fs::write(file.parent().unwrap().join(".git").join("HEAD"), good_head).unwrap();
}

#[test]
fn a_batch_across_two_libraries_commits_one_and_lists_the_other() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, good, good_file) = app_with_git_library(&tmp.path().join("a"));
    let (bad, bad_file) = add_git_library(&mut app, &tmp.path().join("b"));
    let good_hash = pick(&mut app, &good, PDF);
    pick(&mut app, &bad, b"%PDF-1.7 the other library");
    std::fs::write(bad_file.parent().unwrap().join("assets"), b"x").unwrap();

    use crate::app::contracts::{SaveAllNext, SaveAllPlan};
    let task = app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: Vec::new(),
        rows: vec![good.clone(), bad.clone()],
        next: SaveAllNext::Quit,
        probes: Default::default(),
        undecided: Vec::new(),
    });
    run_save_all(&mut app, task);

    assert!(!editor(&app, &good).dirty, "the working library saved");
    assert!(head_has(&good_file, &good_hash));
    assert!(editor(&app, &bad).dirty);
    let notice = format!(
        "{:?}",
        app.document_state.error_notice.as_ref().expect("a card")
    );
    assert!(
        notice.contains("stayed open") && notice.contains("1 item"),
        "{notice}"
    );
}

#[test]
fn close_project_save_all_waits_for_a_staged_row() {
    use crate::app::contracts::{SaveAllNext, SaveAllPlan};
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    let task = app.begin_save_all_planned(SaveAllPlan {
        context: "Close Project".into(),
        paths: Vec::new(),
        rows: vec![address.clone()],
        next: SaveAllNext::CloseProject {
            project_id: crate::app::ProjectId::test_id(9999),
            sheet_refs: Default::default(),
            kept_paths: Vec::new(),
        },
        probes: Default::default(),
        undecided: Vec::new(),
    });
    assert!(!datasheet_asset_path(&file, &hash).unwrap().exists());
    run_save_all(&mut app, task);
    assert!(head_has(&file, &hash));
    assert!(app.library.batch_row_failures.is_empty());
}

#[test]
fn a_stale_completion_during_a_batch_is_not_a_failure_of_the_reopened_editor() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);
    let _task = start_save_all(&mut app, &address);
    let old_generation = editor(&app, &address).saving_generation;

    let before = stored_row(&app, &file, &address);
    app.library.editors.remove(&address);
    app.library.editors.insert(
        address.clone(),
        ComponentPreviewState::from_row(file.clone(), TABLE.into(), before),
    );
    let task = app.finish_staged_asset_store(&address, old_generation, Err("late".into()));
    run_task(&mut app, task);

    assert!(app.library.batch_row_failures.is_empty());
    assert!(editor(&app, &address).saving_batch.is_none());
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
    let library_id = Uuid::now_v7();
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
        app.begin_save_row_in_batch(&address, 1).map(|_| ()),
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

/// #1471 -- the editor is closed during a save, reopened, and saved again.
/// The first save's late completion must not act on the second save: the
/// second one still commits, with no false "not stored yet" error.
#[test]
fn an_old_store_completion_does_not_act_on_a_newer_save() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    pick(&mut app, &address, PDF);
    let _task = app.handle_save_row(&address);
    let old_generation = editor(&app, &address).saving_generation;
    let old_written = store_assets(&file, &editor(&app, &address).staged_assets_in_use()).unwrap();

    // Close and reopen, pick another PDF and save again.
    let before = stored_row(&app, &file, &address);
    app.library.editors.remove(&address);
    app.library.editors.insert(
        address.clone(),
        ComponentPreviewState::from_row(file.clone(), TABLE.into(), before),
    );
    let second = pick(&mut app, &address, b"%PDF-1.7 the second pick");
    let _task = app.handle_save_row(&address);
    let new_generation = editor(&app, &address).saving_generation;
    assert_ne!(old_generation, new_generation);

    // The old completion arrives first and is ignored.
    let task = app.finish_staged_asset_store(&address, old_generation, Ok(old_written));
    run_task(&mut app, task);
    assert!(app.document_state.error_notice.is_none(), "no false error");
    assert!(
        editor(&app, &address).saving_row.is_some(),
        "the newer save is untouched"
    );

    // The newer completion commits normally.
    let written = store_assets(&file, &editor(&app, &address).staged_assets_in_use()).unwrap();
    let task = app.finish_staged_asset_store(&address, new_generation, Ok(written));
    run_task(&mut app, task);
    assert!(app.document_state.error_notice.is_none());
    assert!(!editor(&app, &address).dirty);
    assert!(head_has(&file, &second));
}

/// #859 -- a library rename or delete owns the library's directory: a staged
/// PDF written beside it would recreate the old directory (and strand the
/// rollback) or race the delete. The save refuses first.
fn app_with_library_op(
    kind: crate::app::path_ops::PathOpKind,
) -> (Signex, EditorAddress, PathBuf, String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, address, file) = app_with_git_library(tmp.path());
    let hash = pick(&mut app, &address, PDF);
    app.document_state.pending_path_ops.begin(
        kind,
        vec![file.clone(), tmp.path().join("renamed.snxlib")],
        crate::app::state::scope::CaseFold::PRESERVE,
    );
    (app, address, file, hash, tmp)
}

#[test]
fn a_staged_pdf_is_not_written_while_the_library_is_renamed_or_deleted() {
    use crate::app::path_ops::PathOpKind;
    for kind in [PathOpKind::RenameLibrary, PathOpKind::Delete] {
        let (mut app, address, file, hash, _tmp) = app_with_library_op(kind);

        let begin = app.begin_save_row(&address).map(|_| ());
        let err = format!("{:#}", begin.unwrap_err());
        assert!(err.contains("is still running"), "{err}");
        assert!(
            !datasheet_asset_path(&file, &hash).is_some_and(|p| p.exists()),
            "nothing was written beside the library"
        );
        assert!(editor(&app, &address).saving_row.is_none());
    }
}

#[test]
fn save_all_reports_the_library_hold_as_a_refusal() {
    let (mut app, address, file, hash, _tmp) =
        app_with_library_op(crate::app::path_ops::PathOpKind::RenameLibrary);

    let result = app.begin_save_row_in_batch(&address, 1).map(|_| ());

    assert!(format!("{:#}", result.unwrap_err()).contains("is still running"));
    assert!(editor(&app, &address).saving_row.is_none());
    assert!(!datasheet_asset_path(&file, &hash).is_some_and(|p| p.exists()));
    assert!(editor(&app, &address).dirty, "the row stays owed");
}
