//! #2796 (mockup row 171): the block diagram commands on the headless path,
//! as `signex-cli run` sends them: every edit names its diagram by path or
//! id and answers with JSON, `query_block_diagram` answers the whole
//! diagram, `undo`, `redo` and `save_document` take a diagram, and a
//! refusal changes nothing.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_types::block_diagram::{Interface, read_block_diagram};
use uuid::Uuid;

use super::TabEvent;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::task_driver::TaskDriver;

fn app_in(dir: &tempfile::TempDir) -> Signex {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    app
}

/// Run `id` with `operands` as a headless run does, its tasks driven.
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

fn refused(app: &mut Signex, id: &str, operands: &[&str]) {
    let outcome = run(app, id, operands);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{id} {operands:?} was not refused: {outcome:?}"
    );
}

fn id_of(value: &Value) -> String {
    value["id"].as_str().expect("an id").to_string()
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

fn query(app: &mut Signex, path: &Path) -> Value {
    answer(app, "query_block_diagram", &[&document(path)])
}

fn new_diagram(app: &mut Signex) -> PathBuf {
    let made = answer(app, "new_block_diagram", &["name=System"]);
    assert_eq!(made["kind"], "block_diagram");
    PathBuf::from(made["path"].as_str().expect("a path"))
}

#[test]
fn the_edits_answer_with_what_they_made_and_the_query_reads_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    assert_eq!(path, dir.path().join("System.snxblk"));
    let doc = document(&path);
    let a = id_of(&answer(
        &mut app,
        "block_add",
        &[
            &doc,
            "name=DC/DC Buck",
            "at=10,20",
            "function=3 A buck",
            "shape=rounded_rectangle",
            "rails=VIN_12V,VOUT_3V3",
            "specs=Vout=3.30 V;Fsw=570 kHz",
        ],
    ));
    let b = id_of(&answer(&mut app, "block_add", &[&doc, "name=MCU"]));
    let port = id_of(&answer(
        &mut app,
        "block_port_add",
        &[
            &doc,
            &format!("block={a}"),
            "name=SPI",
            "side=right",
            "direction=output",
        ],
    ));
    let link = id_of(&answer(
        &mut app,
        "block_link",
        &[
            &doc,
            &format!("from={port}"),
            &format!("to={b}"),
            "interface=spi",
            "label=SPI1",
        ],
    ));
    let set = answer(
        &mut app,
        "block_link_set",
        &[
            &doc,
            &format!("id={link}"),
            "interface=I2C",
            "label=SDA, SCL",
        ],
    );
    assert_eq!(set["connection"]["interface"], "i2c");
    let group = id_of(&answer(
        &mut app,
        "block_group",
        &[&doc, "name=Power", &format!("ids={a},{b}")],
    ));
    answer(
        &mut app,
        "block_set",
        &[&doc, &format!("id={b}"), "clear=group"],
    );

    let read = query(&mut app, &path);
    assert_eq!(read["dirty"], true);
    assert_eq!(read["saved"], false);
    let diagram = &read["diagram"];
    let blocks = diagram["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0]["name"], "DC/DC Buck");
    assert_eq!(blocks[0]["shape"], "rounded_rectangle");
    assert_eq!(blocks[0]["specs"]["Fsw"], "570 kHz");
    assert_eq!(blocks[0]["group"], group.as_str());
    assert_eq!(blocks[1]["group"], Value::Null);
    let links = diagram["connections"].as_array().unwrap();
    assert_eq!(links[0]["from"]["port"], port.as_str());
    assert_eq!(links[0]["to"]["block"], b.as_str());
    assert_eq!(links[0]["label"], "SDA, SCL");
}

#[test]
fn a_block_added_without_a_point_lands_where_the_toolbar_puts_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let doc = document(&path);
    answer(&mut app, "block_add", &[&doc, "name=A"]);
    let by_command = id_of(&answer(&mut app, "block_add", &[&doc, "name=B"]));
    let by_command = Uuid::parse_str(&by_command).unwrap();
    let editor = app.open_diagram(&path).expect("open");
    let at = editor.doc.diagram.block(by_command).unwrap().position;
    let _ = app.update(Message::from(super::BlockDiagramMsg::Tab {
        path: path.clone(),
        event: TabEvent::UndoPressed,
    }));
    let _ = app.update(Message::from(super::BlockDiagramMsg::Tab {
        path: path.clone(),
        event: TabEvent::AddBlockPressed,
    }));
    let editor = app.open_diagram(&path).expect("open");
    assert_eq!(editor.doc.diagram.blocks[1].position, at);
}

#[test]
fn a_diagram_is_named_by_its_id_too_and_undo_redo_and_save_take_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let id = query(&mut app, &path)["id"].as_str().unwrap().to_string();
    let by_id = format!("document={id}");
    answer(&mut app, "block_add", &[&by_id, "name=A"]);
    answer(&mut app, "block_add", &[&by_id, "name=B"]);
    assert!(matches!(
        run(&mut app, "undo", &[&by_id]),
        CommandOutcome::Ran
    ));
    let blocks = |app: &mut Signex| {
        query(app, &path)["diagram"]["blocks"]
            .as_array()
            .unwrap()
            .len()
    };
    assert_eq!(blocks(&mut app), 1);
    assert!(matches!(
        run(&mut app, "redo", &[&by_id]),
        CommandOutcome::Ran
    ));
    assert_eq!(blocks(&mut app), 2);
    refused(&mut app, "redo", &[&by_id]);

    assert!(matches!(
        run(&mut app, "save_document", &[&document(&path)]),
        CommandOutcome::Ran
    ));
    let read = read_block_diagram(&path).expect("saved");
    assert_eq!(read.diagram.blocks.len(), 2);
    assert_eq!(read.diagram.id.to_string(), id);
    let after = query(&mut app, &path);
    assert_eq!(after["dirty"], false);
    assert_eq!(after["saved"], true);
}

#[test]
fn a_refused_command_says_why_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let doc = document(&path);
    let a = id_of(&answer(&mut app, "block_add", &[&doc, "name=A"]));
    let before = query(&mut app, &path);
    refused(
        &mut app,
        "block_set",
        &[&doc, &format!("id={a}"), "shape=circle"],
    );
    refused(
        &mut app,
        "block_set",
        &[&doc, &format!("id={a}"), "size=0,10"],
    );
    refused(
        &mut app,
        "block_set",
        &[&doc, &format!("id={a}"), "clear=name"],
    );
    refused(
        &mut app,
        "block_set",
        &[&doc, &format!("id={a}"), "specs=Vout"],
    );
    refused(
        &mut app,
        "block_remove",
        &[&doc, &format!("id={}", Uuid::nil())],
    );
    refused(
        &mut app,
        "block_link",
        &[&doc, &format!("from={a}"), &format!("to={a}")],
    );
    refused(
        &mut app,
        "block_port_add",
        &[&doc, &format!("block={a}"), "name=X", "side=middle"],
    );
    let missing = document(&dir.path().join("missing.snxblk"));
    refused(&mut app, "block_add", &[&missing, "name=A"]);
    assert_eq!(query(&mut app, &path), before);
}

#[test]
fn removing_a_block_takes_its_connections_and_answers_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(&dir);
    let path = new_diagram(&mut app);
    let doc = document(&path);
    let a = id_of(&answer(&mut app, "block_add", &[&doc, "name=A"]));
    let b = id_of(&answer(&mut app, "block_add", &[&doc, "name=B"]));
    let link = id_of(&answer(
        &mut app,
        "block_link",
        &[
            &doc,
            &format!("from={a}"),
            &format!("to={b}"),
            "interface=power",
        ],
    ));
    let removed = answer(&mut app, "block_remove", &[&doc, &format!("id={a}")]);
    assert_eq!(removed["connections"][0], link.as_str());
    let read = query(&mut app, &path);
    assert!(
        read["diagram"]["connections"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(Interface::from_token("power"), Interface::Power);
}

#[test]
fn new_needs_an_open_project() {
    let (mut app, _boot) = Signex::new();
    refused(&mut app, "new_block_diagram", &[]);
    assert!(app.document_state.block_diagrams.editors.is_empty());
}
