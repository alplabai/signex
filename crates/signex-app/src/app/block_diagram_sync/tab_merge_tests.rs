//! The tab's Re-sync (mockup row 171) and the project-write barrier of a
//! run that writes the file: on the GUI path (`Signex::update` with the
//! toolbar's message) and with operands (the headless route).

use std::path::{Path, PathBuf};

use signex_types::block_diagram::read_block_diagram;
use signex_types::schematic::Point;

use super::tests::{app_on, write_hierarchy};
use crate::app::block_diagram::{BlockDiagramMsg, TabEvent};
use crate::app::command::{CommandArgs, CommandOutcome, translator};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::task_driver::TaskDriver;

fn parsed(id: &str, operands: &[&str]) -> (AppCommandId, CommandArgs) {
    let command = AppCommandId::new(id).expect("catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    (command, args)
}

/// A project with sheets Alpha and Beta and its generated diagram on disk.
fn generated(dir: &Path) -> (Signex, PathBuf) {
    write_hierarchy(dir, &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")]);
    let mut app = app_on(dir);
    let project = format!("document={}", dir.join("ProjectA.snxprj").display());
    let (command, args) = parsed("generate_block_diagram", &[&project]);
    let (outcome, _) = app.run_headless_command_in_test(&command, args);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    (app, dir.join("ProjectA.snxblk"))
}

fn tab(app: &mut Signex, path: &Path, event: TabEvent) -> iced::Task<Message> {
    app.update(Message::from(BlockDiagramMsg::Tab {
        path: path.to_path_buf(),
        event,
    }))
}

fn open(app: &mut Signex, path: &Path) {
    let task = app.open_block_diagram_file(path.to_path_buf());
    TaskDriver::new().settle(app, task).expect("settles");
    assert!(app.block_diagram_tab(path).is_some(), "the tab is open");
}

/// A regenerate that writes holds its file as a project write until it
/// lands, so a folder rename waits for it.
#[test]
fn a_run_that_writes_is_a_project_write_until_it_lands() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    write_hierarchy(
        dir.path(),
        &[
            ("Alpha", "a.snxsch"),
            ("Beta", "b.snxsch"),
            ("Gamma", "c.snxsch"),
        ],
    );
    let project = format!("document={}", dir.path().join("ProjectA.snxprj").display());
    let (command, args) = parsed("regenerate_block_diagram", &[&project, "apply=true"]);
    let (outcome, task) = app.run_operands(&command, &args);
    assert!(
        matches!(outcome, CommandOutcome::Deferred { .. }),
        "{outcome:?}"
    );
    assert_eq!(app.document_state.project_writes, vec![file.clone()]);
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(app.document_state.project_writes.is_empty(), "released");
    let written = read_block_diagram(&file).expect("written").diagram;
    assert_eq!(written.blocks.len(), 3);

    // A regenerate that only lists its changes writes nothing and holds
    // nothing.
    let (command, args) = parsed("regenerate_block_diagram", &[&project]);
    let (_, task) = app.run_operands(&command, &args);
    assert!(app.document_state.project_writes.is_empty());
    TaskDriver::new().settle(&mut app, task).expect("settles");
}

/// A run that writes is refused while its project folder is being renamed,
/// and writes nothing.
#[test]
fn a_run_that_writes_waits_for_a_folder_rename() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    let before = std::fs::read(&file).expect("generated");
    let id = app.document_state.projects[0].id;
    app.document_state.project_folder_renames.insert(id);
    let project = format!("document={}", dir.path().join("ProjectA.snxprj").display());
    let (command, args) = parsed("regenerate_block_diagram", &[&project, "apply=true"]);
    let (outcome, _) = app.run_operands(&command, &args);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(app.document_state.project_writes.is_empty());
    assert_eq!(std::fs::read(&file).expect("kept"), before);
}

/// Re-sync on the tab merges the hierarchy into the open diagram as one
/// undo step: an unsaved edit is kept, the file is not written, and Save
/// writes the result.
#[test]
fn resync_merges_into_the_open_diagram_as_one_undo_step() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    let moved = app.document_state.block_diagrams.editors[&file]
        .diagram
        .blocks[0]
        .id;
    let to = Point::new(-40.0, 12.5);
    let _ = tab(&mut app, &file, TabEvent::BlockMoved { block: moved, to });
    write_hierarchy(
        dir.path(),
        &[
            ("Alpha", "a.snxsch"),
            ("Beta", "b.snxsch"),
            ("Gamma", "c.snxsch"),
        ],
    );
    let bytes = std::fs::read(&file).expect("bytes");
    let steps = app.document_state.block_diagrams.editors[&file].undo.len();

    let task = tab(&mut app, &file, TabEvent::ResyncPressed);
    assert!(translator::ran("regenerate_block_diagram"));
    TaskDriver::new().settle(&mut app, task).expect("settles");

    let editor = app.open_diagram(&file).expect("open");
    assert_eq!(editor.doc.diagram.blocks.len(), 3, "Gamma came in");
    assert_eq!(
        editor.doc.diagram.block(moved).map(|b| b.position),
        Some(to)
    );
    assert_eq!(editor.doc.undo.len(), steps + 1, "one undo step");
    assert!(app.document_state.is_document_dirty(&file));
    assert_eq!(
        std::fs::read(&file).expect("bytes"),
        bytes,
        "nothing written"
    );

    let task = tab(&mut app, &file, TabEvent::UndoPressed);
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert_eq!(
        app.document_state.block_diagrams.editors[&file]
            .diagram
            .blocks
            .len(),
        2,
        "undo takes the merge back"
    );
}

/// An edit made while Re-sync reads the hierarchy wins: the merge is not
/// applied over it.
#[test]
fn a_resync_is_not_applied_over_an_edit_made_while_it_ran() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    write_hierarchy(
        dir.path(),
        &[
            ("Alpha", "a.snxsch"),
            ("Beta", "b.snxsch"),
            ("Gamma", "c.snxsch"),
        ],
    );
    let task = tab(&mut app, &file, TabEvent::ResyncPressed);
    let _ = tab(&mut app, &file, TabEvent::AddBlockPressed);
    let after_edit = app.document_state.block_diagrams.editors[&file]
        .diagram
        .clone();
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert_eq!(
        app.document_state.block_diagrams.editors[&file].diagram,
        after_edit
    );
}

/// The palette's regenerate with `apply=true` from another surface still
/// refuses to write under the open diagram (the agent path is unchanged).
#[test]
fn an_agent_regenerate_still_refuses_an_open_diagram() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    let project = format!("document={}", dir.path().join("ProjectA.snxprj").display());
    let (command, args) = parsed("regenerate_block_diagram", &[&project, "apply=true"]);
    let (outcome, _) = app.run_headless_command_in_test(&command, args);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
}
