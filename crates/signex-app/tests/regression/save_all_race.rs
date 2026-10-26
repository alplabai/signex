//! #859 / #1068 — Close Project's Save All writes off the UI thread while
//! the window stays live. A document edited in that window must be asked
//! about again, never dropped by the close.

use signex_app::app::{
    LoadedProject, Message, ProjectCloseChoice, ProjectMsg, Signex, TabInfo, TabKind,
};
use signex_types::project::{ProjectData, SheetEntry};
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

use crate::close_support::close_first_project;
use crate::settle_support::settle_task;

fn sheet_entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// A project at `dir/P.snxprj` listing `X.snxsch` (its root) and `Y.snxsch`,
/// both open in tabs; only X is dirty.
fn project_with_two_sheets(dir: &Path) -> Signex {
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            name: "P".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("X.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![sheet_entry("X.snxsch"), sheet_entry("Y.snxsch")],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    for (name, dirty) in [("X", true), ("Y", false)] {
        let path = dir.join(format!("{name}.snxsch"));
        let mut engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            paper_size: "A4".to_string(),
            ..Default::default()
        })
        .expect("engine");
        engine.set_path(Some(path.clone()));
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(TabInfo {
            title: name.to_string(),
            path: path.clone(),
            cached_document: None,
            dirty,
            project_id: Some(id),
            kind: TabKind::Schematic,
        });
        if dirty {
            app.document_state.dirty_paths.insert(path);
        }
    }
    app
}

#[test]
fn a_clean_sheet_edited_while_close_project_save_all_runs_is_prompted_not_dropped() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = project_with_two_sheets(tmp.path());
    let (x, y) = (tmp.path().join("X.snxsch"), tmp.path().join("Y.snxsch"));
    close_first_project(&mut app);
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("X is dirty, so close prompts");
    assert_eq!(modal.dirty_paths, vec![x.clone()]);

    let save_all = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::SaveAll,
    )));
    // An edit to Y while X is being written.
    app.document_state.dirty_paths.insert(y.clone());
    settle_task(&mut app, save_all);

    assert!(x.exists(), "Save All wrote X");
    assert_eq!(
        app.document_state.projects.len(),
        1,
        "the project stays open"
    );
    assert!(
        app.document_state.engines.contains_key(&y),
        "Y is not dropped"
    );
    assert!(app.document_state.dirty_paths.contains(&y));
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("the close gate asks about Y");
    assert_eq!(modal.dirty_paths, vec![y]);
}

#[test]
fn close_project_save_all_with_nothing_edited_meanwhile_closes() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = project_with_two_sheets(tmp.path());
    close_first_project(&mut app);
    let save_all = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::SaveAll,
    )));
    settle_task(&mut app, save_all);

    assert!(app.document_state.projects.is_empty(), "the project closed");
    assert!(app.ui_state.project_close_confirm.is_none());
}
