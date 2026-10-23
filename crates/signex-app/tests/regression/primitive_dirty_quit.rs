//! #666 — footprint-editor and Component Preview edits must reach the
//! unsaved-changes gates.
//!
//! `handle_app_quit_requested` and `close_project_at_tree_path` decide
//! whether to prompt from `dirty_paths` alone. The footprint handlers
//! only flipped `editor.dirty` and the Component Preview reducers only
//! flipped `state.dirty`, so File ▸ Exit and Close Project closed
//! straight away and the edits went out with the process.

use signex_app::app::{
    FootprintEditorState, LoadedProject, Message, ProjectCloseChoice, ProjectMsg, Signex, TabInfo,
    TabKind, WindowMsg,
};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{EditorMsg, LibraryMessage};
use signex_app::library::state::{ComponentPreviewState, EditorAddress};
use signex_app::panels::PanelMsg;
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, Footprint, FootprintFile, InternalPn,
    LifecycleState, ManufacturerPart, ParamMap, PlmReserved, PrimitiveRef, RowId,
};
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Park a clean footprint editor at `path` behind an active tab, the
/// state right after opening a `.snxfpt`.
pub(crate) fn open_footprint_editor(app: &mut Signex, path: &Path) {
    let file = FootprintFile::from_footprint(Footprint::empty("fp"));
    app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        FootprintEditorState::new(path.to_path_buf(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "fp".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.to_path_buf()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// Edit the footprint description through the Properties panel.
pub(crate) fn edit_description_via_panel(app: &mut Signex) {
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetFootprintDescription("edited".into()),
    )));
}

fn request_quit(app: &mut Signex) {
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
}

#[test]
fn footprint_panel_edit_makes_quit_prompt_instead_of_closing() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("a.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);

    edit_description_via_panel(&mut app);
    request_quit(&mut app);

    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("an edited footprint must open the app-quit confirm modal");
    assert!(modal.dirty_paths.contains(&path));
    assert!(app.document_state.tabs[0].dirty, "tab shows the dirty dot");
}

#[test]
fn footprint_canvas_edit_marks_dirty_paths() {
    use signex_app::library::messages::{FootprintEditorMsg, PrimitiveEdit};
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("b.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);

    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetName("renamed".into())),
    }));

    assert!(app.document_state.dirty_paths.contains(&path));
}

#[test]
fn quit_save_all_writes_the_edited_footprint() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("c.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);
    request_quit(&mut app);

    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveAll,
    )));

    let written = std::fs::read_to_string(&path).expect("Save All must write the .snxfpt");
    assert!(written.contains("edited"), "the edit must be on disk");
    assert!(app.document_state.dirty_paths.is_empty());
    assert!(!app.document_state.footprint_editors[&path].dirty);
}

/// Load a project rooted at `dir` mounting the project-local `lib.snxlib`.
fn push_project(app: &mut Signex, dir: &Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let prj = dir.join(format!("{name}.snxprj"));
    std::fs::write(&prj, b"{}").unwrap();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj,
        data: ProjectData {
            name: name.into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: vec![LibraryEntry {
                path: PathBuf::from("lib.snxlib"),
                kind: LibraryEntryKind::ProjectLocal,
                library_id: None,
                unknown: Default::default(),
            }],
            enable_git: false,
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
    });
}

/// An app with one loaded project `p` rooted at `dir` (project id 0).
fn app_with_open_project(dir: &Path) -> Signex {
    let (mut app, _t) = Signex::new();
    push_project(&mut app, dir, "p");
    app
}

#[test]
fn project_close_keeps_an_edited_footprint_draft_it_cannot_prove_is_its_own() {
    // A footprint draft under the project's folder is reached by no sheet walk,
    // so it is never the project's to discard: it is kept, its editor and tab
    // stay open, and the close does not prompt for it.
    let tmp = TempDir::new().unwrap();
    let path = tmp
        .path()
        .join("lib.snxlib")
        .join("footprints")
        .join("d.snxfpt");
    let mut app = app_with_open_project(tmp.path());
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);

    crate::close_support::close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "nothing is provably the project's, so there is nothing to save or discard",
    );
    assert!(app.document_state.projects.is_empty(), "the project closes");
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.footprint_editors.contains_key(&path));
    assert!(app.document_state.tabs.iter().any(|t| t.path == path));
}

fn preview_row() -> ComponentRow {
    ComponentRow {
        row_id: uuid::Uuid::new_v4(),
        internal_pn: InternalPn::new("TEST_PN"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        content_hash: [0u8; 32],
    }
}

/// An app holding one unmounted Component Preview editor whose
/// parameter has just been edited through the real update path.
pub(crate) fn app_with_preview_edit() -> Signex {
    let (app, _t) = Signex::new();
    preview_edit_in(app, PathBuf::from("/tmp/signex-666/lib.snxlib"))
}

/// Same, with the row's library at `library_path` on an existing app.
fn preview_edit_in(mut app: Signex, library_path: PathBuf) -> Signex {
    let row = preview_row();
    let row_id = RowId::from_uuid(row.row_id);
    let table = "generics".to_string();
    app.library.editors.insert(
        EditorAddress::new(library_path.clone(), table.clone(), row_id),
        ComponentPreviewState::from_row(library_path.clone(), table.clone(), row),
    );
    let _ = app.update(Message::Library(LibraryMessage::EditorEvent {
        library_path,
        table,
        row_id,
        msg: EditorMsg::ParamSetText {
            name: "tolerance".into(),
            value: "1%".into(),
        },
    }));
    app
}

#[test]
fn component_preview_edit_makes_quit_prompt_instead_of_closing() {
    let mut app = app_with_preview_edit();
    assert!(app.library.editors.values().any(|s| s.dirty));

    request_quit(&mut app);

    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("an edited Component Preview row must open the quit modal");
    assert_eq!(modal.dirty_rows.len(), 1);
}

#[test]
fn quit_save_all_never_loses_a_row_it_cannot_save() {
    // The library is not mounted, so the row cannot be written. Save All
    // must keep Signex open and say why, not exit and drop the edit.
    let mut app = app_with_preview_edit();
    request_quit(&mut app);

    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveAll,
    )));

    assert!(app.document_state.error_notice.is_some());
    assert!(app.library.editors.values().all(|s| s.dirty));
}

#[test]
fn project_close_prompts_for_a_preview_row_under_the_project_and_discard_drops_it() {
    let tmp = TempDir::new().unwrap();
    let app = app_with_open_project(tmp.path());
    let mut app = preview_edit_in(app, tmp.path().join("lib.snxlib"));

    crate::close_support::close_first_project(&mut app);
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("a dirty Component Preview row under the project must prompt");
    assert_eq!(modal.dirty_rows.len(), 1);

    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(app.document_state.projects.is_empty());
    assert!(app.library.editors.is_empty(), "Discard drops the draft");
}

#[test]
fn project_close_save_all_keeps_the_project_when_a_row_cannot_be_saved() {
    let tmp = TempDir::new().unwrap();
    let app = app_with_open_project(tmp.path());
    let mut app = preview_edit_in(app, tmp.path().join("lib.snxlib"));
    crate::close_support::close_first_project(&mut app);

    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::SaveAll,
    )));

    assert_eq!(app.document_state.projects.len(), 1, "project stays open");
    assert!(app.document_state.error_notice.is_some());
    assert!(app.library.editors.values().all(|s| s.dirty));
}

#[test]
fn project_close_keeps_a_preview_row_of_a_project_nested_in_its_folder() {
    // B lives inside A's folder and mounts its own `lib.snxlib`. A row dirty in
    // B's library sits under A's directory but is B's: closing A must not offer
    // it to Discard All.
    let tmp = TempDir::new().unwrap();
    let mut app = app_with_open_project(tmp.path());
    push_project(&mut app, &tmp.path().join("sub"), "b");
    let mut app = preview_edit_in(app, tmp.path().join("sub").join("lib.snxlib"));

    crate::close_support::close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "the nested project's row is not A's to save or discard",
    );
    assert_eq!(app.document_state.projects.len(), 1, "A closes, B stays");
    assert!(app.library.editors.values().all(|s| s.dirty), "row kept");
}

#[test]
fn project_close_keeps_a_listed_library_row_that_sits_inside_another_projects_folder() {
    // A lists `sub/lib.snxlib`, but B is open at `sub`: the library is inside
    // another open project's directory, so the row is not provably A's alone.
    let tmp = TempDir::new().unwrap();
    let mut app = app_with_open_project(tmp.path());
    app.document_state.projects[0].data.libraries[0].path = PathBuf::from("sub/lib.snxlib");
    push_project(&mut app, &tmp.path().join("sub"), "b");
    let mut app = preview_edit_in(app, tmp.path().join("sub").join("lib.snxlib"));

    crate::close_support::close_first_project(&mut app);

    assert!(app.ui_state.project_close_confirm.is_none());
    assert!(app.library.editors.values().all(|s| s.dirty), "row kept");
}
