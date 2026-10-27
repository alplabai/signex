//! #1042 — the Properties panel edits the active project's net-class table
//! through engine commands: each edit is undoable and marks the `.snxprj`
//! dirty, and nothing is written to disk until the user saves.

use signex_app::app::{LoadedProject, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{NetClassField, NetClassMsg, PanelMsg};
use signex_types::net_class::NetLineStyle;
use signex_types::project::ProjectData;
use std::collections::HashMap;
use std::path::PathBuf;

fn fixture() -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join("signex-net-classes-panel-1042");
    let path = dir.join("P.snxprj");
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: path.clone(),
        data: ProjectData {
            name: "P".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
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
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    (app, path)
}

fn send(app: &mut Signex, msg: NetClassMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(msg))));
}

fn commit(app: &mut Signex, field: NetClassField, text: &str) {
    send(app, NetClassMsg::Draft(field, text.to_string()));
    send(app, NetClassMsg::Commit(field));
}

fn classes(app: &Signex) -> &[signex_types::net_class::NetClass] {
    &app.document_state.projects[0].data.net_classes
}

#[test]
fn editing_a_class_changes_the_project_and_marks_it_dirty() {
    let (mut app, path) = fixture();
    assert!(!app.document_state.dirty_paths.contains(&path));

    send(&mut app, NetClassMsg::Add);
    commit(&mut app, NetClassField::Name, "Power");
    commit(&mut app, NetClassField::Width, "0.4");
    send(&mut app, NetClassMsg::SetLine(NetLineStyle::Dashed));

    let class = &classes(&app)[0];
    assert_eq!(class.name, "Power");
    assert_eq!(class.style.wire_width_mm, Some(0.4));
    assert_eq!(class.style.line, NetLineStyle::Dashed);
    assert!(app.document_state.dirty_paths.contains(&path));

    let shown = &app.document_state.panel_ctx.net_classes;
    assert!(shown.available);
    assert_eq!(shown.classes, classes(&app));
    assert!(shown.can_undo);
}

#[test]
fn undo_and_redo_walk_the_table_back_and_forth() {
    let (mut app, _path) = fixture();
    send(&mut app, NetClassMsg::Add);
    commit(&mut app, NetClassField::Name, "Power");

    send(&mut app, NetClassMsg::Undo);
    assert_eq!(classes(&app)[0].name, "Class 1");
    assert!(app.document_state.panel_ctx.net_classes.can_redo);
    send(&mut app, NetClassMsg::Redo);
    assert_eq!(classes(&app)[0].name, "Power");
    send(&mut app, NetClassMsg::Undo);
    send(&mut app, NetClassMsg::Undo);
    assert!(classes(&app).is_empty());
}

#[test]
fn a_refused_edit_leaves_the_project_clean() {
    let (mut app, path) = fixture();
    send(&mut app, NetClassMsg::Add);
    app.document_state.dirty_paths.clear();
    commit(&mut app, NetClassField::Color, "not-a-colour");
    assert!(classes(&app)[0].style.is_default());
    assert!(!app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.panel_ctx.net_classes.notice.is_some());
}

fn second_project(app: &mut Signex) {
    let dir = std::env::temp_dir().join("signex-net-classes-panel-1042-b");
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("B.snxprj"),
        data: ProjectData {
            name: "B".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: vec![signex_types::net_class::NetClass::named("Sig")],
            rules: None,
            annotation: None,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
}

#[test]
fn switching_project_resyncs_the_selection_and_drafts() {
    let (mut app, _path) = fixture();
    send(&mut app, NetClassMsg::Add);
    commit(&mut app, NetClassField::Name, "Power");
    commit(&mut app, NetClassField::Width, "0.4");

    second_project(&mut app);
    // Enter on A's still-shown Width field must not write A's value onto B.
    send(&mut app, NetClassMsg::Commit(NetClassField::Width));
    let b = &app.document_state.projects[1].data.net_classes[0];
    assert_eq!(b.name, "Sig");
    assert_eq!(b.style.wire_width_mm, None);

    let shown = &app.document_state.panel_ctx.net_classes;
    assert_eq!(shown.classes[0].name, "Sig");
    assert_eq!(shown.draft.name, "Sig");
    assert_eq!(shown.draft.width, "");
}
