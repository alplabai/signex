//! #668 — sheet rename and Save As must re-key every path-keyed map.
//!
//! A schematic is keyed by its on-disk path in `engines`, `dirty_paths`,
//! the tab and the engine's own save target. Re-keying only some of those
//! leaves the quit guard / tab-close check looking at a path no engine
//! backs, so unsaved edits are dropped or Save All fails. Save As onto a
//! path another tab hosts must also refuse instead of replacing that
//! tab's engine.

use signex_app::app::{
    FileMsg, LoadedProject, Message, RenameDialogState, RenameMsg, Signex, TabInfo, TabKind,
};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::SchematicSheet;
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// Open a schematic tab + engine at `path`, optionally flagged dirty.
/// Returns the sheet uuid so a test can tell engines apart.
fn open_sheet(app: &mut Signex, path: &Path, dirty: bool) -> uuid::Uuid {
    let sheet = empty_sheet();
    let uuid = sheet.uuid;
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty,
        project_id: None,
        kind: TabKind::Schematic,
    });
    if dirty {
        app.document_state.dirty_paths.insert(path.to_path_buf());
    }
    uuid
}

fn activate(app: &mut Signex, tab: usize) {
    app.document_state.active_tab = tab;
    app.document_state.active_path = Some(app.document_state.tabs[tab].path.clone());
}

#[test]
fn renaming_a_dirty_sheet_moves_its_dirty_marker_and_dirties_the_project() {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();
    let prj_path = dir.join("Proj.snxprj");
    let old = dir.join("Power.snxsch");
    let new = dir.join("PSU.snxsch");
    std::fs::write(&old, b"x").expect("write sheet");

    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj_path.clone(),
        data: ProjectData {
            name: "Proj".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("Power.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "Power".to_string(),
                filename: "Power.snxsch".to_string(),
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
    app.document_state.active_project = Some(id);
    open_sheet(&mut app, &old, true);
    activate(&mut app, 0);

    app.ui_state.rename_dialog = Some(RenameDialogState {
        target_path: old.clone(),
        tree_path: vec![0],
        buffer: "PSU.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });
    let _ = app.update(Message::Rename(RenameMsg::Submit));

    let dirty = &app.document_state.dirty_paths;
    assert!(dirty.contains(&new), "dirty marker must follow the rename");
    assert!(!dirty.contains(&old), "stale dirty marker must be gone");
    assert!(
        dirty.contains(&prj_path),
        "the edited .snxprj must be marked dirty so the rename is saved"
    );
    let engine = app
        .document_state
        .engines
        .get(&new)
        .expect("engine re-keyed");
    assert_eq!(
        engine.path(),
        Some(new.as_path()),
        "engine save target must follow the rename"
    );
}

#[test]
fn renaming_a_non_sheet_file_does_not_dirty_the_project() {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();
    let prj_path = dir.join("Proj.snxprj");
    let old = dir.join("R0603.snxfpt");
    std::fs::write(&old, b"x").expect("write footprint");

    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj_path.clone(),
        data: ProjectData {
            name: "Proj".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("Power.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "Power".to_string(),
                filename: "Power.snxsch".to_string(),
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
    app.document_state.active_project = Some(id);

    app.ui_state.rename_dialog = Some(RenameDialogState {
        target_path: old.clone(),
        tree_path: vec![0],
        buffer: "R0402.snxfpt".to_string(),
        error: None,
        is_project_rename: false,
    });
    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(
        dir.join("R0402.snxfpt").exists(),
        "the rename itself must have happened"
    );
    assert!(
        !app.document_state.dirty_paths.contains(&prj_path),
        "renaming a file the project does not list must not dirty the .snxprj"
    );
}

#[test]
fn save_as_onto_another_open_sheet_is_refused() {
    let tmp = TempDir::new().expect("tempdir");
    let a = tmp.path().join("A.snxsch");
    let b = tmp.path().join("B.snxsch");

    let (mut app, _task) = Signex::new();
    let a_uuid = open_sheet(&mut app, &a, false);
    let b_uuid = open_sheet(&mut app, &b, true);
    activate(&mut app, 0);

    let _ = app.update(Message::File(FileMsg::SaveAs {
        from_path: a.clone(),
        to_path: b.clone(),
    }));

    let engines = &app.document_state.engines;
    assert_eq!(engines.get(&a).expect("A kept").document().uuid, a_uuid);
    assert_eq!(
        engines.get(&b).expect("B kept").document().uuid,
        b_uuid,
        "B's engine (with its unsaved edits) must not be replaced"
    );
    assert!(app.document_state.dirty_paths.contains(&b));
    assert!(!b.exists(), "nothing may be written over B");
}

#[test]
fn save_as_drops_the_old_path_from_dirty_paths() {
    let tmp = TempDir::new().expect("tempdir");
    let a = tmp.path().join("A.snxsch");
    let c = tmp.path().join("C.snxsch");

    let (mut app, _task) = Signex::new();
    open_sheet(&mut app, &a, true);
    activate(&mut app, 0);

    let _ = app.update(Message::File(FileMsg::SaveAs {
        from_path: a.clone(),
        to_path: c.clone(),
    }));

    assert!(c.exists(), "Save As must write the new file");
    assert!(app.document_state.engines.contains_key(&c));
    assert!(
        !app.document_state.dirty_paths.contains(&a),
        "old path has no engine; leaving it dirty makes Save All fail on quit: {:?}",
        app.document_state.dirty_paths
    );
}
