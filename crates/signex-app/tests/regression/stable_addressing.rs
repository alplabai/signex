//! #672 — a position captured before a state change must not retarget
//! a later action. Closing a tab left of the active one, or deleting a
//! sibling primitive before the active one, used to leave the stored
//! index pointing at the next document / primitive.

use signex_app::app::{
    ContextMenuMsg, FootprintEditorState, LoadedProject, Message, Signex, SymbolEditorState,
    TabContextAction, TabInfo, TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use signex_types::project::ProjectData;
use std::path::{Path, PathBuf};

fn push_tab(app: &mut Signex, path: &Path, project_id: Option<signex_app::app::ProjectId>) {
    app.document_state.tabs.push(TabInfo {
        title: path.display().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id,
        kind: TabKind::FootprintEditor(path.to_path_buf()),
    });
    let file = FootprintFile::from_footprint(Footprint::empty("fp"));
    app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        FootprintEditorState::new(path.to_path_buf(), file),
    );
}

fn active_path(app: &Signex) -> PathBuf {
    app.document_state.tabs[app.document_state.active_tab]
        .path
        .clone()
}

#[test]
fn closing_a_tab_left_of_the_active_one_keeps_the_active_document() {
    let (mut app, _t) = Signex::new();
    for n in ["a.snxfpt", "b.snxfpt", "c.snxfpt", "d.snxfpt"] {
        push_tab(&mut app, Path::new(n), None);
    }
    app.document_state.active_tab = 2;

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));

    assert_eq!(active_path(&app), PathBuf::from("c.snxfpt"));
}

fn close_tab(app: &mut Signex, idx: usize) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(idx),
    )));
}

#[test]
fn closing_the_active_tab_focuses_the_tab_that_took_its_slot() {
    let (mut app, _t) = Signex::new();
    for n in ["a.snxfpt", "b.snxfpt", "c.snxfpt"] {
        push_tab(&mut app, Path::new(n), None);
    }
    app.document_state.active_tab = 1;

    close_tab(&mut app, 1);

    assert_eq!(app.document_state.tabs.len(), 2);
    assert_eq!(active_path(&app), PathBuf::from("c.snxfpt"));
}

#[test]
fn closing_the_tail_active_tab_focuses_the_one_before_it() {
    let (mut app, _t) = Signex::new();
    for n in ["a.snxfpt", "b.snxfpt", "c.snxfpt"] {
        push_tab(&mut app, Path::new(n), None);
    }
    app.document_state.active_tab = 2;

    close_tab(&mut app, 2);

    assert_eq!(app.document_state.tabs.len(), 2);
    assert_eq!(active_path(&app), PathBuf::from("b.snxfpt"));
}

#[test]
fn closing_the_only_tab_leaves_an_empty_list() {
    let (mut app, _t) = Signex::new();
    push_tab(&mut app, Path::new("a.snxfpt"), None);
    app.document_state.active_tab = 0;

    close_tab(&mut app, 0);

    assert!(app.document_state.tabs.is_empty());
    assert_eq!(app.document_state.active_tab, 0);
}

#[test]
fn close_project_with_interleaved_tabs_keeps_the_active_document() {
    let (mut app, _t) = Signex::new();
    let mut ids = Vec::new();
    for n in ["p1", "p2"] {
        let id = app.document_state.mint_project_id();
        ids.push(id);
        app.document_state.projects.push(LoadedProject {
            id,
            path: PathBuf::from(format!("{n}.snxprj")),
            data: ProjectData {
                name: n.into(),
                dir: String::new(),
                schematic_root: None,
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                unknown: Default::default(),
            },
            pending_libraries: std::collections::HashMap::new(),
        });
    }
    push_tab(&mut app, Path::new("p1a.snxfpt"), Some(ids[0]));
    push_tab(&mut app, Path::new("p2a.snxfpt"), Some(ids[1]));
    push_tab(&mut app, Path::new("p2b.snxfpt"), Some(ids[1]));
    push_tab(&mut app, Path::new("p1b.snxfpt"), Some(ids[0]));
    app.document_state.active_tab = 1;

    crate::close_support::close_first_project(&mut app);

    assert_eq!(app.document_state.tabs.len(), 2);
    assert_eq!(active_path(&app), PathBuf::from("p2a.snxfpt"));
}

#[test]
fn deleting_a_footprint_before_the_active_one_keeps_the_active_primitive() {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("lib.snxfpt");
    let mut file = FootprintFile::from_footprint(Footprint::empty("a"));
    for n in ["b", "c", "d"] {
        file.footprints.push(Footprint::empty(n));
    }
    let active_uuid = file.footprints[2].uuid;
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.active_idx = 2;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "lib".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpLibraryDeleteInternal(0),
    )));

    let editor = &app.document_state.footprint_editors[&path];
    assert_eq!(editor.primitive().uuid, active_uuid);
    assert!(app.document_state.tabs[0].dirty, "tab chip must show dirty");
}

#[test]
fn deleting_a_symbol_before_the_active_one_keeps_the_active_primitive() {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("lib.snxsym");
    let mut file = SymbolFile::from_symbol(Symbol::empty("a"));
    for n in ["b", "c", "d"] {
        file.symbols.push(Symbol::empty(n));
    }
    let active_uuid = file.symbols[2].uuid;
    let mut editor = SymbolEditorState::new(path.clone(), file);
    editor.active_idx = 2;
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "lib".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SchLibraryDeleteSymbol(0),
    )));

    let editor = &app.document_state.symbol_editors[&path];
    assert_eq!(editor.file.symbols[editor.active_idx].uuid, active_uuid);
}
