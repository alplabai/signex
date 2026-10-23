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
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
        unknown: Default::default(),
    }
}

fn add_project(app: &mut Signex, dir: &Path, sheet: &str) -> ProjectId {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
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
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
        },
        pending_libraries: HashMap::new(),
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
