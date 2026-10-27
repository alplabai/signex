//! Hierarchical child-sheet path resolution (#339, #406).
//!
//! A `ChildSheet.filename` is relative to the sheet that references it, not to
//! the project root — the convention `state::scope::parent_of` and
//! `project_sheets::project_graph` already use. Resolving it against the
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
                    unknown: Default::default(),
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
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
fn an_absolute_child_reference_outside_the_root_is_rejected() {
    // #463 — an absolute reference used to be taken verbatim, letting a
    // malicious `.snxsch`/`.snxprj` point a child sheet anywhere on disk;
    // it's writable via Ctrl+S once opened as a tab.
    let app = app_focused_on("/w/a/sub/mid.snxsch");
    assert_eq!(app.resolve_child_sheet_path("/elsewhere/leaf.snxsch"), None);
}

#[test]
fn a_traversal_child_reference_outside_the_root_is_rejected() {
    // #463 — a relative `..` reference used to be joined onto the parent
    // directory verbatim with no containment check, walking out of it.
    let app = app_focused_on("/w/a/sub/mid.snxsch");
    assert_eq!(app.resolve_child_sheet_path("../../../etc/passwd"), None);
}

#[test]
fn an_empty_child_reference_resolves_to_nothing() {
    let app = app_focused_on("/w/a/top.snxsch");
    assert_eq!(app.resolve_child_sheet_path("   "), None);
}

/// Double-clicking a child sheet whose tab was closed with unsaved edits
/// must reattach the parked engine, not replace it with the disk copy (#1135).
#[test]
fn child_sheet_double_click_keeps_the_dirty_engine_of_a_tab_less_sheet() {
    use crate::app::handlers::menu::export::tests::{open_with, sheet_with_net};
    use signex_types::format::SnxSchematic;

    let dir = std::env::temp_dir().join(format!("signex-dbl-dirty-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let path = dir.join("child.snxsch");
    let text = SnxSchematic::new(sheet_with_net("R_DISK", "DISK_NET", &[]))
        .write_string()
        .expect("serialize");
    std::fs::write(&path, text).expect("write sheet");

    let (mut app, _task) = Signex::new();
    open_with(
        &mut app.document_state,
        &path,
        sheet_with_net("R_EDIT", "EDITED_NET", &[]),
    );
    app.document_state.dirty_paths.insert(path.clone());

    app.open_or_focus_sheet_path(path.clone());

    let held: Vec<String> = app.document_state.engines[&path]
        .document()
        .labels
        .iter()
        .map(|l| l.text.clone())
        .collect();
    let tabs = app.document_state.tabs.len();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(held, vec!["EDITED_NET".to_string()]);
    assert_eq!(tabs, 1);
}

/// #1050: a canvas drag of a bus, through the real handler, carries the entry
/// on it and the wire on its far end as one undo step.
#[test]
fn dragging_a_bus_on_the_canvas_carries_entry_and_wire() {
    use signex_types::schematic::{
        Bus, BusEntry, Point, SchematicSheet, SelectedItem, SelectedKind, Wire,
    };
    let (mut app, _task) = Signex::new();
    let (bus_id, entry_id, wire_id) = (
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
    );
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        buses: vec![Bus {
            uuid: bus_id,
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 20.0),
        }],
        bus_entries: vec![BusEntry {
            uuid: entry_id,
            position: Point::new(0.0, 10.0),
            size: (2.54, 2.54),
        }],
        wires: vec![Wire {
            uuid: wire_id,
            start: Point::new(2.54, 12.54),
            end: Point::new(12.54, 12.54),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
        }],
        ..Default::default()
    };
    let path = PathBuf::from("bus-drag.snxsch");
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "bus-drag".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    app.ui_state.snap_enabled = false;
    app.ui_state.move_keeps_wires = true;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(bus_id, SelectedKind::Bus)];

    let _ = app.handle_canvas_interaction_event(crate::canvas::CanvasEvent::MoveSelected {
        dx: 5.0,
        dy: 0.0,
    });

    let engine = app.document_state.active_engine().expect("engine");
    let doc = engine.document();
    assert_eq!(doc.buses[0].start, Point::new(5.0, 0.0));
    assert_eq!(doc.bus_entries[0].position, Point::new(5.0, 10.0));
    assert_eq!(doc.wires[0].start, Point::new(7.54, 12.54));
    assert_eq!(doc.wires[0].end, Point::new(12.54, 12.54));
}
