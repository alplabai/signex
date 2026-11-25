//! #2796 (mockup row 171): the block diagram tab driven the way a person
//! drives it, through `Signex::update` with the messages its menu row,
//! toolbar, canvas and Properties send. Each edit is one undo step, the
//! dirty mark follows the file, and a save writes what a read gives back.

use std::path::{Path, PathBuf};

use signex_types::block_diagram::{Endpoint, Interface, read_block_diagram};
use signex_types::schematic::Point;
use uuid::Uuid;

use super::state::{DiagramItem, DiagramTool, PropertyField};
use super::{BlockDiagramMsg, TabEvent};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex, TabKind};
use crate::task_driver::TaskDriver;

fn app_in(dir: &tempfile::TempDir) -> Signex {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    app
}

/// File ▸ New ▸ Block Diagram, as the menu row's bridge arm sends it.
fn new_diagram(app: &mut Signex) -> PathBuf {
    let _ = app.update(Message::from(BlockDiagramMsg::NewRequested));
    let path = app
        .document_state
        .block_diagrams
        .editors
        .keys()
        .next()
        .cloned()
        .expect("a diagram was made");
    path
}

fn send(app: &mut Signex, path: &Path, event: TabEvent) -> iced::Task<Message> {
    app.update(Message::from(BlockDiagramMsg::Tab {
        path: path.to_path_buf(),
        event,
    }))
}

fn editor<'a>(app: &'a Signex, path: &Path) -> &'a super::BlockDiagramEditor {
    app.document_state
        .block_diagrams
        .editors
        .get(path)
        .expect("the diagram is open")
}

fn add_block(app: &mut Signex, path: &Path) -> Uuid {
    let _ = send(app, path, TabEvent::AddBlockPressed);
    match editor(app, path).single_selection() {
        Some(DiagramItem::Block(id)) => id,
        other => panic!("the new block is not selected: {other:?}"),
    }
}

#[test]
fn new_opens_an_unsaved_diagram_named_after_the_project_in_its_own_tab() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    assert_eq!(path, dir.path().join("ProjectA.snxblk"));
    assert!(!path.exists(), "nothing is written before a save");
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    assert!(matches!(&tab.kind, TabKind::BlockDiagram(p) if *p == path));
    assert_eq!(tab.title, "ProjectA");
    assert!(app.document_state.is_document_dirty(&path));
    // A second New for the same name is refused; the first is kept.
    let _ = app.update(Message::from(BlockDiagramMsg::NewRequested));
    assert_eq!(app.document_state.block_diagrams.editors.len(), 1);
}

#[test]
fn toolbar_edits_are_undo_steps_and_undo_back_to_the_file_reads_clean() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let a = add_block(&mut app, &path);
    let b = add_block(&mut app, &path);
    let diagram = &editor(&app, &path).diagram;
    assert_eq!(diagram.blocks.len(), 2);
    assert_eq!(diagram.block(a).unwrap().name, "Block 1");
    assert!(diagram.block(b).unwrap().position.x > diagram.block(a).unwrap().position.x);
    assert_eq!(editor(&app, &path).undo.len(), 2);

    let _ = send(&mut app, &path, TabEvent::UndoPressed);
    assert_eq!(editor(&app, &path).diagram.blocks.len(), 1);
    let _ = send(&mut app, &path, TabEvent::RedoPressed);
    assert_eq!(editor(&app, &path).diagram.blocks.len(), 2);
}

#[test]
fn a_dragged_block_lands_as_one_edit_and_selection_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let a = add_block(&mut app, &path);
    let steps = editor(&app, &path).undo.len();
    let _ = send(
        &mut app,
        &path,
        TabEvent::ItemPicked {
            item: None,
            add: false,
        },
    );
    assert!(editor(&app, &path).selection.is_empty());
    assert_eq!(editor(&app, &path).undo.len(), steps, "a pick is no edit");
    let to = Point::new(12.5, -7.0);
    let _ = send(&mut app, &path, TabEvent::BlockMoved { block: a, to });
    assert_eq!(editor(&app, &path).diagram.block(a).unwrap().position, to);
    assert_eq!(editor(&app, &path).undo.len(), steps + 1);
}

#[test]
fn the_connect_tool_joins_two_picked_ends_with_a_signal_connection() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let a = add_block(&mut app, &path);
    let b = add_block(&mut app, &path);
    let _ = send(&mut app, &path, TabEvent::ToolPicked(DiagramTool::Connect));
    let from = Endpoint {
        block: a,
        port: None,
    };
    let to = Endpoint {
        block: b,
        port: None,
    };
    let _ = send(&mut app, &path, TabEvent::EndPicked(from));
    assert_eq!(editor(&app, &path).connect_from, Some(from));
    assert!(editor(&app, &path).diagram.connections.is_empty());
    let _ = send(&mut app, &path, TabEvent::EndPicked(to));
    let diagram = &editor(&app, &path).diagram;
    assert_eq!(diagram.connections.len(), 1);
    assert_eq!(diagram.connections[0].from, from);
    assert_eq!(diagram.connections[0].to, to);
    assert_eq!(diagram.connections[0].interface, Interface::Signal);
    assert_eq!(editor(&app, &path).connect_from, None);

    // Properties: the interface picker and the label field.
    let _ = send(&mut app, &path, TabEvent::InterfacePicked("i2c"));
    let label = PropertyField::ConnectionLabel;
    let _ = send(
        &mut app,
        &path,
        TabEvent::FieldEdited(label, " SDA, SCL ".into()),
    );
    let _ = send(&mut app, &path, TabEvent::FieldCommitted(label));
    let link = &editor(&app, &path).diagram.connections[0];
    assert_eq!(link.interface, Interface::I2c);
    assert_eq!(link.label, "SDA, SCL");
}

#[test]
fn properties_commit_a_field_and_a_refused_value_leaves_the_block_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let a = add_block(&mut app, &path);
    let commit = |app: &mut Signex, field, text: &str| {
        let _ = send(app, &path, TabEvent::FieldEdited(field, text.to_string()));
        let _ = send(app, &path, TabEvent::FieldCommitted(field));
    };
    commit(&mut app, PropertyField::BlockName, "DC/DC Buck");
    commit(
        &mut app,
        PropertyField::BlockSpecs,
        "Vout=3.30 V; Fsw=570 kHz",
    );
    commit(&mut app, PropertyField::BlockRails, "VIN_12V, VOUT_3V3");
    commit(&mut app, PropertyField::BlockWidth, "50");
    let block = editor(&app, &path).diagram.block(a).unwrap().clone();
    assert_eq!(block.name, "DC/DC Buck");
    assert_eq!(block.specs.get("Fsw").map(String::as_str), Some("570 kHz"));
    assert_eq!(block.rails, ["VIN_12V", "VOUT_3V3"]);
    assert_eq!(block.width_mm, 50.0);

    let steps = editor(&app, &path).undo.len();
    commit(&mut app, PropertyField::BlockName, "   ");
    commit(&mut app, PropertyField::BlockHeight, "tall");
    assert_eq!(editor(&app, &path).diagram.block(a), Some(&block));
    assert_eq!(editor(&app, &path).undo.len(), steps);
    assert!(
        editor(&app, &path).drafts.is_empty(),
        "a commit drops its draft"
    );
}

#[test]
fn group_and_delete_act_on_the_selection_as_one_step_each() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let a = add_block(&mut app, &path);
    let b = add_block(&mut app, &path);
    let pick = |item, add| TabEvent::ItemPicked {
        item: Some(item),
        add,
    };
    let _ = send(&mut app, &path, pick(DiagramItem::Block(a), false));
    let _ = send(&mut app, &path, pick(DiagramItem::Block(b), true));
    let _ = send(&mut app, &path, TabEvent::GroupPressed);
    let diagram = &editor(&app, &path).diagram;
    assert_eq!(diagram.groups.len(), 1);
    let group = diagram.groups[0].id;
    assert!(
        diagram
            .blocks
            .iter()
            .all(|block| block.group == Some(group))
    );

    let steps = editor(&app, &path).undo.len();
    let _ = send(&mut app, &path, pick(DiagramItem::Block(a), false));
    let _ = send(&mut app, &path, pick(DiagramItem::Group(group), true));
    let _ = send(&mut app, &path, TabEvent::DeletePressed);
    let diagram = &editor(&app, &path).diagram;
    assert_eq!(diagram.blocks.len(), 1);
    assert!(diagram.groups.is_empty());
    assert_eq!(diagram.blocks[0].group, None, "its group went with it");
    assert_eq!(editor(&app, &path).undo.len(), steps + 1);
    assert!(editor(&app, &path).selection.is_empty());
}

#[test]
fn save_writes_the_diagram_a_read_gives_back_and_marks_it_clean() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let _ = add_block(&mut app, &path);
    let task = send(&mut app, &path, TabEvent::SavePressed);
    TaskDriver::new().settle(&mut app, task).expect("settles");
    let read = read_block_diagram(&path).expect("the file reads");
    assert_eq!(read.diagram, editor(&app, &path).diagram);
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(!editor(&app, &path).is_new());
    // Undo after the save makes it dirty again; redo back reads clean.
    let _ = send(&mut app, &path, TabEvent::UndoPressed);
    assert!(app.document_state.is_document_dirty(&path));
    let _ = send(&mut app, &path, TabEvent::RedoPressed);
    assert!(!app.document_state.is_document_dirty(&path));
}

#[test]
fn a_new_diagram_never_replaces_a_file_already_at_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = dir.path().join("ProjectA.snxblk");
    std::fs::write(&path, b"keep me").unwrap();
    let made = new_diagram(&mut app);
    assert_eq!(made, path);
    let task = send(&mut app, &path, TabEvent::SavePressed);
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert_eq!(std::fs::read(&path).unwrap(), b"keep me");
    assert!(app.document_state.is_document_dirty(&path));
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn opening_a_file_reads_it_in_a_task_and_closing_a_clean_tab_lets_it_go() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let a = add_block(&mut app, &path);
    let task = send(&mut app, &path, TabEvent::SavePressed);
    TaskDriver::new().settle(&mut app, task).expect("settles");
    let idx = app.block_diagram_tab(&path).expect("a tab");
    let _ = app.close_block_diagram_tab(idx, false);
    assert!(app.document_state.block_diagrams.editors.is_empty());

    let task = app.open_path_reported(path.clone()).expect("opens");
    assert!(
        app.document_state.block_diagrams.editors.is_empty(),
        "read in a task"
    );
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(editor(&app, &path).diagram.block(a).is_some());
    assert!(app.block_diagram_tab(&path).is_some());
}

#[test]
fn closing_a_dirty_tab_keeps_the_diagram_for_save_all_and_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let a = add_block(&mut app, &path);
    let idx = app.block_diagram_tab(&path).expect("a tab");
    let _ = app.close_block_diagram_tab(idx, false);
    assert!(app.block_diagram_tab(&path).is_none());
    assert!(app.document_state.is_document_dirty(&path));
    let task = app.open_block_diagram_file(path.clone());
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(
        editor(&app, &path).diagram.block(a).is_some(),
        "the edits are kept"
    );
    assert!(app.block_diagram_tab(&path).is_some());
}
