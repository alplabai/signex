//! Hierarchical child-sheet path resolution (#339, #406).
//!
//! A `ChildSheet.filename` is relative to the sheet that references it, not to
//! the project root — the convention `state::scope::parent_of` and
//! `project_sheets::project_children_map` already use. Resolving it against the
//! project directory instead opens the wrong file (or reports "not found") for
//! any sheet that does not sit directly in the project root.

use std::path::PathBuf;

use crate::app::state::LoadedProject;
use crate::app::{Signex, TabInfo, TabKind};
use signex_types::project::{ProjectData, SheetEntry};

/// An app with one loaded project whose `.snxprj` is at `/w/a`, listing both
/// `top.snxsch` and `sub/mid.snxsch`, and one tab focused on `focused`.
///
/// Both sheets are *listed*, which matters: it is what makes
/// `active_document_project()` resolve to ProjectA, which is what made the old
/// project-relative base directory fire. A fixture whose focused sheet is
/// unowned falls through to the loose-document branch and cannot go red.
fn app_focused_on(focused: &str) -> Signex {
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: PathBuf::from("/w/a/ProjectA.snxprj"),
        data: ProjectData {
            name: "ProjectA".to_string(),
            dir: "/w/a".to_string(),
            schematic_root: Some("top.snxsch".to_string()),
            pcb_file: None,
            sheets: ["top.snxsch", "sub/mid.snxsch"]
                .iter()
                .map(|f| SheetEntry {
                    name: (*f).to_string(),
                    filename: (*f).to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
        },
        pending_libraries: std::collections::HashMap::new(),
    });
    app.document_state.active_project = Some(id);
    let path = PathBuf::from(focused);
    app.document_state.tabs.push(TabInfo {
        title: "sheet".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    app
}

#[test]
fn child_of_a_subdirectory_sheet_resolves_beside_its_parent() {
    // The exact shape `state::scope::grandchild_resolves_through_two_hops`
    // encodes: /w/a/top.snxsch references "sub/mid.snxsch", and that sheet
    // references "leaf.snxsch" sitting next to it. Resolving against the
    // project directory yields /w/a/leaf.snxsch — the wrong file.
    let app = app_focused_on("/w/a/sub/mid.snxsch");
    assert_eq!(
        app.resolve_child_sheet_path("leaf.snxsch"),
        Some(PathBuf::from("/w/a/sub/leaf.snxsch"))
    );
}

#[test]
fn child_of_a_root_sheet_still_resolves_in_the_project_root() {
    // The flat case both conventions agree on — kept so the fix cannot
    // over-correct it.
    let app = app_focused_on("/w/a/top.snxsch");
    assert_eq!(
        app.resolve_child_sheet_path("child.snxsch"),
        Some(PathBuf::from("/w/a/child.snxsch"))
    );
}

#[test]
fn a_relative_child_reference_keeps_its_own_subpath() {
    let app = app_focused_on("/w/a/top.snxsch");
    assert_eq!(
        app.resolve_child_sheet_path("sub/mid.snxsch"),
        Some(PathBuf::from("/w/a/sub/mid.snxsch"))
    );
}

#[test]
fn an_absolute_child_reference_is_taken_verbatim() {
    let app = app_focused_on("/w/a/sub/mid.snxsch");
    assert_eq!(
        app.resolve_child_sheet_path("/elsewhere/leaf.snxsch"),
        Some(PathBuf::from("/elsewhere/leaf.snxsch"))
    );
}

#[test]
fn an_empty_child_reference_resolves_to_nothing() {
    let app = app_focused_on("/w/a/top.snxsch");
    assert_eq!(app.resolve_child_sheet_path("   "), None);
}
