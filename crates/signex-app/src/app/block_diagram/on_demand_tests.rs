//! A command names a diagram that is not open (#2796): the headless route
//! reads it first and runs the command when the read lands, as `open_document`
//! would have; the diagram is held without a tab. A file that does not read,
//! or a diagram outside every open project, is refused with why.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::block_diagram::read_block_diagram;

use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;
use crate::task_driver::TaskDriver;

const DCDC: &str =
    include_str!("../../../../signex-types/tests/corpus/block_diagram/dcdc-buck.snxblk");

fn app_in(dir: &tempfile::TempDir) -> Signex {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    app
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    TaskDriver::new().settle(app, task).expect("settles");
    outcome
}

fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

/// The corpus diagram saved in the project folder, not open.
fn on_disk(dir: &tempfile::TempDir) -> PathBuf {
    let path = dir.path().join("dcdc-buck.snxblk");
    std::fs::write(&path, DCDC).expect("write the diagram");
    path
}

#[test]
fn an_edit_on_a_diagram_not_open_reads_it_first_and_holds_it_without_a_tab() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = on_disk(&dir);
    let tabs = app.document_state.tabs.len();

    let added = answer(&mut app, "block_add", &[&document(&path), "name=Sensor"]);
    let id = added["id"]
        .as_str()
        .expect("the new block's id")
        .to_string();
    let editor = &app.document_state.block_diagrams.editors[&path];
    assert!(editor.diagram.blocks.iter().any(|b| b.id.to_string() == id));
    assert!(editor.is_dirty(), "unsaved until saved");
    assert_eq!(app.document_state.tabs.len(), tabs, "no tab was opened");
    assert_eq!(
        std::fs::read_to_string(&path).expect("kept"),
        DCDC,
        "nothing written"
    );

    // The next command finds it open; save writes it.
    let queried = answer(&mut app, "query_block_diagram", &[&document(&path)]);
    assert_eq!(queried["dirty"], true, "{queried}");
    let saved = run(&mut app, "save_document", &[&document(&path)]);
    assert!(matches!(saved, CommandOutcome::Ran), "{saved:?}");
    let read = read_block_diagram(&path).expect("reads").diagram;
    assert!(read.blocks.iter().any(|b| b.id.to_string() == id));
}

#[test]
fn a_query_on_a_diagram_not_open_reads_it_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = on_disk(&dir);
    let queried = answer(&mut app, "query_block_diagram", &[&document(&path)]);
    assert_eq!(queried["dirty"], false, "{queried}");
    assert_eq!(
        queried["diagram"]["blocks"].as_array().map(Vec::len),
        Some(6)
    );
}

#[test]
fn a_file_that_does_not_read_or_lies_outside_every_project_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let broken = dir.path().join("broken.snxblk");
    std::fs::write(&broken, "not a diagram").expect("write");
    let outcome = run(&mut app, "block_add", &[&document(&broken), "name=A"]);
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("{outcome:?}");
    };
    assert!(
        reason
            .render_en_us()
            .contains("could not be read as a block diagram"),
        "{}",
        reason.render_en_us()
    );
    assert!(app.document_state.block_diagrams.editors.is_empty());

    let elsewhere = tempfile::tempdir().unwrap();
    let outside = on_disk(&elsewhere);
    let outcome = run(&mut app, "block_add", &[&document(&outside), "name=A"]);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(app.document_state.block_diagrams.editors.is_empty());
}

#[test]
fn export_names_a_diagram_that_is_not_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = on_disk(&dir);
    let svg = dir.path().join("blocks.svg");
    let target = format!("path={}", svg.display());
    let exported = answer(
        &mut app,
        "export_block_diagram",
        &[&document(&path), &target],
    );
    assert_eq!(exported["blocks"], 6, "{exported}");
    assert!(svg.exists());
    assert!(
        app.document_state.block_diagrams.editors.is_empty(),
        "the export reads the saved file and opens nothing"
    );
}

#[test]
fn a_person_opening_the_file_later_sees_the_held_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = on_disk(&dir);
    let added = answer(&mut app, "block_add", &[&document(&path), "name=Later"]);
    let id = added["id"].as_str().expect("id").to_string();
    let task = app.open_block_diagram_file(path.clone());
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(app.block_diagram_tab(&path).is_some());
    let editor = &app.document_state.block_diagrams.editors[&path];
    assert!(editor.diagram.blocks.iter().any(|b| b.id.to_string() == id));
}
