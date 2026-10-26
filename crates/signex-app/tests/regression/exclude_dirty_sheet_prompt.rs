//! #738 — Exclude from Project on a dirty sheet prompts Save / Discard /
//! Cancel; a clean sheet is excluded straight away.

use signex_app::app::{
    ExcludeDirtyChoice, LoadedProject, Message, ProjectCloseChoice, ProjectId, ProjectMsg,
    RemoveChoice, RemoveDialogState, RemoveMsg, Signex, TabInfo, TabKind, WindowMsg,
};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::SchematicSheet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const ORIGINAL: &[u8] = b"original-bytes";

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        ..Default::default()
    }
}

fn add_project(app: &mut Signex, dir: &Path, sheet: &str) -> ProjectId {
    add_named_project(app, dir, "P.snxprj", sheet)
}

fn add_named_project(app: &mut Signex, dir: &Path, file: &str, sheet: &str) -> ProjectId {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join(file),
        data: ProjectData {
            name: "P".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: sheet.to_string(),
                filename: sheet.to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    id
}

/// Park an engine + tab for `path`; `dirty` decides whether it is flagged.
fn open_sheet(app: &mut Signex, path: &Path, project: ProjectId, dirty: bool) {
    let mut engine = signex_engine::Engine::new(empty_sheet()).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty,
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
    if dirty {
        app.document_state.dirty_paths.insert(path.to_path_buf());
    }
}

/// A project with one sheet `X.snxsch` on disk holding `ORIGINAL`.
fn fixture(dirty: bool) -> (Signex, TempDir, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let sheet = tmp.path().join("X.snxsch");
    std::fs::write(&sheet, ORIGINAL).expect("write sheet");
    let (mut app, _task) = Signex::new();
    let id = add_project(&mut app, tmp.path(), "X.snxsch");
    open_sheet(&mut app, &sheet, id, dirty);
    (app, tmp, sheet)
}

fn exclude(app: &mut Signex, target: &Path) {
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: target.to_path_buf(),
        tree_path: vec![0, 0],
        project_id: app.document_state.projects.first().map(|p| p.id),
        display_name: target.file_name().unwrap().to_string_lossy().into_owned(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));
}

fn choose(app: &mut Signex, choice: ExcludeDirtyChoice) {
    let _ = app.update(Message::Remove(RemoveMsg::ExcludeDirty(choice)));
}

fn project_lists_sheet(app: &Signex) -> bool {
    app.document_state.projects[0]
        .data
        .sheets
        .iter()
        .any(|s| s.filename == "X.snxsch")
}

#[test]
fn excluding_a_dirty_sheet_prompts_and_changes_nothing_yet() {
    let (mut app, _tmp, sheet) = fixture(true);

    exclude(&mut app, &sheet);

    let dialog = app
        .ui_state
        .remove_dialog
        .as_ref()
        .expect("the modal must stay open for the unsaved-edits stage");
    assert!(dialog.unsaved_prompt);
    assert!(project_lists_sheet(&app));
    assert!(app.document_state.tabs.iter().any(|t| t.path == sheet));
    assert!(app.document_state.dirty_paths.contains(&sheet));
    assert_eq!(std::fs::read(&sheet).unwrap(), ORIGINAL);
}

#[test]
fn save_writes_the_sheet_then_excludes_it() {
    let (mut app, _tmp, sheet) = fixture(true);
    exclude(&mut app, &sheet);

    choose(&mut app, ExcludeDirtyChoice::Save);

    assert_ne!(std::fs::read(&sheet).unwrap(), ORIGINAL, "Save must write");
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(!project_lists_sheet(&app));
    assert!(!app.document_state.dirty_paths.contains(&sheet));
    assert!(app.document_state.tabs.iter().all(|t| t.path != sheet));
}

#[test]
fn discard_excludes_without_writing_and_leaves_no_dirty_engine() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, tmp, sheet) = fixture(true);
    exclude(&mut app, &sheet);

    choose(&mut app, ExcludeDirtyChoice::Discard);

    assert_eq!(std::fs::read(&sheet).unwrap(), ORIGINAL, "must not write");
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(!project_lists_sheet(&app));
    assert!(!app.document_state.dirty_paths.contains(&sheet));
    assert!(!app.document_state.engines.contains_key(&sheet));
    assert!(app.document_state.tabs.iter().all(|t| t.path != sheet));
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("X.snxsch") && e.message.contains("iscard")),
        "the discard must reach the Messages panel",
    );

    // Save All on quit (kept alive by a second dirty sheet) writes nothing for it.
    let other = tmp.path().join("Y.snxsch");
    let id = app.document_state.projects[0].id;
    open_sheet(&mut app, &other, id, true);
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveAll,
    )));
    assert_eq!(std::fs::read(&sheet).unwrap(), ORIGINAL);
}

#[test]
fn cancel_leaves_everything_as_it_was() {
    let (mut app, _tmp, sheet) = fixture(true);
    exclude(&mut app, &sheet);

    let _ = app.update(Message::Remove(RemoveMsg::Close));

    assert!(app.ui_state.remove_dialog.is_none());
    assert!(project_lists_sheet(&app));
    assert!(app.document_state.tabs.iter().any(|t| t.path == sheet));
    assert!(app.document_state.dirty_paths.contains(&sheet));
    assert!(app.document_state.engines.contains_key(&sheet));
    assert_eq!(std::fs::read(&sheet).unwrap(), ORIGINAL);
}

#[test]
fn a_clean_sheet_is_excluded_without_a_prompt() {
    let (mut app, _tmp, sheet) = fixture(false);

    exclude(&mut app, &sheet);

    assert!(app.ui_state.remove_dialog.is_none(), "no prompt for clean");
    assert!(!project_lists_sheet(&app));
    assert!(app.document_state.tabs.iter().all(|t| t.path != sheet));
    assert_eq!(std::fs::read(&sheet).unwrap(), ORIGINAL);
}

#[test]
fn a_failed_save_keeps_the_prompt_and_does_not_exclude() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    // The sheet's "directory" is a regular file, so the atomic write must fail.
    let blocker = tmp.path().join("blocker");
    std::fs::write(&blocker, b"not a directory").expect("write blocker");
    let sheet = blocker.join("X.snxsch");
    let (mut app, _task) = Signex::new();
    let id = add_project(&mut app, tmp.path(), "X.snxsch");
    open_sheet(&mut app, &sheet, id, true);
    exclude(&mut app, &sheet);

    choose(&mut app, ExcludeDirtyChoice::Save);

    assert!(project_lists_sheet(&app), "a failed save must not exclude");
    assert!(app.document_state.dirty_paths.contains(&sheet));
    assert!(app.document_state.engines.contains_key(&sheet));
    assert!(
        app.ui_state.remove_dialog.is_some() || app.document_state.error_notice.is_some(),
        "the failure must be visible to the user",
    );
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("X.snxsch")),
        "the failure must reach the Messages panel",
    );
}

/// Two open projects (A at index 0, B at index 1) both list the same
/// `X.snxsch`, and X is dirty.
fn shared_fixture() -> (Signex, TempDir, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let sheet = tmp.path().join("X.snxsch");
    std::fs::write(&sheet, ORIGINAL).expect("write sheet");
    let (mut app, _task) = Signex::new();
    let a = add_named_project(&mut app, tmp.path(), "A.snxprj", "X.snxsch");
    add_named_project(&mut app, tmp.path(), "B.snxprj", "X.snxsch");
    open_sheet(&mut app, &sheet, a, true);
    (app, tmp, sheet)
}

fn diagnostics_mention(app: &Signex, needle: &str) -> bool {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .any(|e| e.message.contains(needle))
}

#[test]
fn discard_is_refused_while_another_open_project_shares_the_sheet() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _tmp, sheet) = shared_fixture();
    exclude(&mut app, &sheet);
    assert!(app.ui_state.remove_dialog.is_some(), "prompt expected");

    choose(&mut app, ExcludeDirtyChoice::Discard);

    assert!(
        app.document_state.engines.contains_key(&sheet),
        "B's edits must survive"
    );
    assert!(app.document_state.dirty_paths.contains(&sheet));
    assert!(project_lists_sheet(&app), "A must not be excluded either");
    assert!(
        app.ui_state.remove_dialog.is_some(),
        "the prompt stays open so the user can Save or Cancel",
    );
    assert!(
        diagnostics_mention(&app, "another open project"),
        "the refusal must reach the Messages panel",
    );
    assert_eq!(std::fs::read(&sheet).unwrap(), ORIGINAL);
}

#[test]
fn save_still_works_for_a_sheet_shared_with_another_project() {
    let (mut app, _tmp, sheet) = shared_fixture();
    exclude(&mut app, &sheet);

    choose(&mut app, ExcludeDirtyChoice::Save);

    assert_ne!(std::fs::read(&sheet).unwrap(), ORIGINAL, "Save must write");
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(!project_lists_sheet(&app));
}

/// A project with a `Lib.snxlib` holding a dirty footprint draft whose
/// `dirty_paths` marker was never synced (only the editor flag is set).
fn library_fixture() -> (Signex, TempDir, PathBuf, PathBuf, Vec<u8>) {
    let tmp = TempDir::new().expect("tempdir");
    let lib = tmp.path().join("Lib.snxlib");
    let fp = lib.join("footprints").join("g.snxfpt");
    std::fs::create_dir_all(fp.parent().unwrap()).expect("mkdir");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, tmp.path(), "X.snxsch");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    // A container save merges into the file on disk, so it must be a real one.
    let original = file.to_toml_string().expect("serialise").into_bytes();
    std::fs::write(&fp, &original).expect("write footprint");
    let mut editor = signex_app::app::FootprintEditorState::new(fp.clone(), file);
    // A real edit: a container save merges only footprints changed since the
    // editor's baseline (#902), so an untouched draft would write nothing.
    editor.file.footprints[0].name = "fp-edited".to_string();
    editor.dirty = true;
    app.document_state
        .footprint_editors
        .insert(fp.clone(), editor);
    (app, tmp, lib, fp, original)
}

#[test]
fn excluding_a_library_with_a_dirty_draft_prompts_and_save_writes_it() {
    let (mut app, _tmp, lib, fp, original) = library_fixture();

    exclude(&mut app, &lib);

    let dialog = app
        .ui_state
        .remove_dialog
        .as_ref()
        .expect("prompt expected");
    assert!(dialog.unsaved_prompt);

    choose(&mut app, ExcludeDirtyChoice::Save);

    assert_ne!(std::fs::read(&fp).unwrap(), original, "Save must write it");
    let written = String::from_utf8(std::fs::read(&fp).unwrap()).expect("utf8 container");
    assert!(written.contains("fp-edited"), "the edit must be on disk");
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(!app.document_state.dirty_paths.contains(&fp));
}

#[test]
fn discarding_a_library_drops_every_draft_under_it() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _tmp, lib, fp, original) = library_fixture();
    exclude(&mut app, &lib);

    choose(&mut app, ExcludeDirtyChoice::Discard);

    assert_eq!(std::fs::read(&fp).unwrap(), original, "must not write");
    assert!(app.document_state.footprint_editors.is_empty());
    assert!(app.document_state.dirty_paths.is_empty());
    assert!(app.ui_state.remove_dialog.is_none());
}

#[test]
fn discard_is_refused_for_a_library_two_open_projects_list() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, tmp, lib, fp, original) = library_fixture();
    // `library_fixture` opened project A; add B, and let both list the one
    // `.snxlib` (A relative, B by an un-normalised absolute spelling).
    add_named_project(&mut app, tmp.path(), "B.snxprj", "Y.snxsch");
    let entry = |path: PathBuf, kind| signex_types::project::LibraryEntry {
        path,
        kind,
        library_id: None,
        unknown: Default::default(),
    };
    app.document_state.projects[0].data.libraries.push(entry(
        PathBuf::from("Lib.snxlib"),
        signex_types::project::LibraryEntryKind::ProjectLocal,
    ));
    app.document_state.projects[1].data.libraries.push(entry(
        lib.join("."),
        signex_types::project::LibraryEntryKind::Shared,
    ));
    exclude(&mut app, &lib);
    assert!(app.ui_state.remove_dialog.is_some(), "prompt expected");

    choose(&mut app, ExcludeDirtyChoice::Discard);

    assert!(
        app.document_state.footprint_editors.contains_key(&fp),
        "B's draft must survive"
    );
    assert!(app.document_state.dirty_paths.contains(&fp));
    assert!(app.ui_state.remove_dialog.is_some());
    assert!(diagnostics_mention(&app, "another open project"));
    assert_eq!(std::fs::read(&fp).unwrap(), original);
}

#[test]
fn discard_is_refused_for_a_library_another_project_lists_through_dotdot() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, tmp, lib, fp, original) = library_fixture();
    // B lives in `b/` and reaches the same `.snxlib` as `../Lib.snxlib`, a
    // spelling that only matches A's entry once `..` is folded away.
    let dir_b = tmp.path().join("b");
    std::fs::create_dir(&dir_b).expect("mkdir b");
    let b = add_named_project(&mut app, &dir_b, "B.snxprj", "Y.snxsch");
    // B's sheet is loaded, so its walk is complete and the only thing that
    // can withhold Discard is the shared library, not "not loaded".
    open_sheet(&mut app, &dir_b.join("Y.snxsch"), b, false);
    let entry = |path: PathBuf, kind| signex_types::project::LibraryEntry {
        path,
        kind,
        library_id: None,
        unknown: Default::default(),
    };
    app.document_state.projects[0].data.libraries.push(entry(
        PathBuf::from("Lib.snxlib"),
        signex_types::project::LibraryEntryKind::ProjectLocal,
    ));
    app.document_state.projects[1].data.libraries.push(entry(
        PathBuf::from("../Lib.snxlib"),
        signex_types::project::LibraryEntryKind::ProjectLocal,
    ));
    exclude(&mut app, &lib);
    assert!(app.ui_state.remove_dialog.is_some(), "prompt expected");

    choose(&mut app, ExcludeDirtyChoice::Discard);

    assert!(
        app.document_state.footprint_editors.contains_key(&fp),
        "B's draft must survive"
    );
    assert!(app.document_state.dirty_paths.contains(&fp));
    assert!(app.ui_state.remove_dialog.is_some());
    assert!(diagnostics_mention(&app, "also uses it"));
    assert_eq!(std::fs::read(&fp).unwrap(), original);
}

/// Project A (dirty `X.snxsch`) plus project B in its own directory, listing
/// `Y.snxsch`. When `load_other` is set B's sheet has an engine, so its walk
/// is fully known; otherwise B's references cannot be read without disk IO.
fn unrelated_project_fixture(load_other: bool) -> (Signex, TempDir, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let dir_b = tmp.path().join("b");
    std::fs::create_dir(&dir_b).expect("mkdir b");
    let sheet = tmp.path().join("X.snxsch");
    std::fs::write(&sheet, ORIGINAL).expect("write sheet");
    let (mut app, _task) = Signex::new();
    let a = add_named_project(&mut app, tmp.path(), "A.snxprj", "X.snxsch");
    let b = add_named_project(&mut app, &dir_b, "B.snxprj", "Y.snxsch");
    open_sheet(&mut app, &sheet, a, true);
    if load_other {
        open_sheet(&mut app, &dir_b.join("Y.snxsch"), b, false);
    }
    (app, tmp, sheet)
}

#[test]
fn discard_is_offered_when_every_other_project_walk_is_known_and_unrelated() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _tmp, sheet) = unrelated_project_fixture(true);
    exclude(&mut app, &sheet);
    assert!(app.ui_state.remove_dialog.is_some(), "prompt expected");

    choose(&mut app, ExcludeDirtyChoice::Discard);

    assert!(app.ui_state.remove_dialog.is_none(), "discard must proceed");
    assert!(!app.document_state.engines.contains_key(&sheet));
}

#[test]
fn discard_is_refused_when_another_projects_sheets_are_not_loaded() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _tmp, sheet) = unrelated_project_fixture(false);
    exclude(&mut app, &sheet);
    assert!(app.ui_state.remove_dialog.is_some(), "prompt expected");

    choose(&mut app, ExcludeDirtyChoice::Discard);

    assert!(
        app.document_state.engines.contains_key(&sheet),
        "nothing may be discarded while ownership is unknown"
    );
    assert!(app.ui_state.remove_dialog.is_some());
    assert!(
        diagnostics_mention(&app, "not loaded"),
        "the refusal must say why"
    );
    assert_eq!(std::fs::read(&sheet).unwrap(), ORIGINAL);
}
