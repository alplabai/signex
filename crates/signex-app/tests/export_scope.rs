//! Regression: export scope follows the active document, not the sticky
//! workspace `active_project` pointer (#406).
//!
//! Opening a loose schematic from another directory leaves
//! `DocumentState.active_project` pointing at the last-loaded project —
//! that stickiness is deliberate (the Projects panel, ERC and annotate all
//! depend on it). What must NOT happen is the exporter inheriting it:
//! before the fix, Print Preview / Export PDF scoped off `active_project`
//! and emitted the *project's* sheets with the loose document's netlist
//! resolved to `None`, blanking every `NET_NAME()` annotation.

use signex_app::app::{LoadedProject, Signex};
use signex_types::project::{ProjectData, SheetEntry};

use std::path::PathBuf;
use tempfile::TempDir;

fn sheet(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
    }
}

/// Workspace with one loaded + active project owning two sheets.
fn workspace_with_project(sheets: &[&str]) -> (Signex, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();

    let (mut app, _initial_task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("ProjectA.snxprj"),
        data: ProjectData {
            name: "ProjectA".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some(sheets[0].to_string()),
            pcb_file: None,
            sheets: sheets.iter().map(|f| sheet(f)).collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
        },
        pending_libraries: std::collections::HashMap::new(),
    });
    app.document_state.active_project = Some(id);

    (app, tmp)
}

#[test]
fn project_sheet_exports_with_its_own_project_scope() {
    let (mut app, tmp) = workspace_with_project(&["top.snxsch", "power.snxsch"]);
    app.document_state.active_path = Some(tmp.path().join("power.snxsch"));

    let scope = app
        .document_state
        .export_scope_project()
        .expect("a listed project sheet resolves to its project");
    assert_eq!(scope.data.name, "ProjectA");
    assert_eq!(scope.data.sheets.len(), 2);
}

#[test]
fn loose_schematic_does_not_inherit_the_sticky_project_scope() {
    let (mut app, _project_dir) = workspace_with_project(&["top.snxsch", "power.snxsch"]);
    let project_id = app.document_state.active_project;

    // File > Open a schematic from an unrelated directory. No companion
    // .snxprj, so no project is loaded or activated for it.
    let loose_dir = TempDir::new().expect("tempdir");
    let loose_path: PathBuf = loose_dir.path().join("scratch.snxsch");
    app.document_state.active_path = Some(loose_path);

    // The sticky pointer is untouched — other subsystems still rely on it.
    assert_eq!(
        app.document_state.active_project, project_id,
        "active_project must stay sticky (#54 phase 2.4)"
    );
    // ...but the export no longer follows it.
    assert!(
        app.document_state.export_scope_project().is_none(),
        "a loose schematic must export as a standalone sheet, not \
         as the sticky project's sheet set"
    );
}

#[test]
fn schematic_in_the_project_directory_but_not_in_its_sheet_list_is_loose() {
    // Weaker variant of the same desync: parent-directory matching
    // (`project_for_path`, which assigns `TabInfo.project_id`) would claim
    // this file for ProjectA and drag its sheet list into the export.
    let (mut app, tmp) = workspace_with_project(&["top.snxsch"]);
    let stray = tmp.path().join("scratch.snxsch");
    app.document_state.active_path = Some(stray.clone());

    assert!(
        app.document_state.project_for_path(&stray).is_some(),
        "precondition: parent-dir matching does claim this file"
    );
    assert!(
        app.document_state.export_scope_project().is_none(),
        "export scope must be the sheet list, not the parent directory"
    );
}
