//! #2488 — the board STEP export through `Signex::update` (File ▸ Export ▸
//! STEP 3D Model, the save dialog's answer, the made model) and through the
//! headless path.

use super::tests::{app_with_board, output};
use super::{BoardOutputMsg, BoardStepBuilt};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

fn headless(app: &mut Signex, operands: &[&str]) -> (CommandOutcome, iced::Task<Message>) {
    let command = AppCommandId::new("export_board_step").unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args)
}

fn read_step(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).expect("the STEP file was written");
    String::from_utf8(bytes).expect("STEP is ASCII")
}

#[test]
fn the_command_is_live_on_a_board_and_asks_for_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    assert!(crate::app::command::is_enabled(
        &AppCommandId::new("export_board_step").unwrap(),
        &app.command_context(None)
    ));
    let task = app.dispatch_board_output_message(BoardOutputMsg::StepExportRequested);
    assert_eq!(task.units(), 1, "the save dialog");
}

#[test]
fn the_chosen_file_gets_the_board_and_its_parts() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let file = dir.path().join("board.step");
    let task = output(
        &mut app,
        BoardOutputMsg::StepPathChosen {
            board,
            path: file.clone(),
        },
    );
    settle(&mut app, task);
    let text = read_step(&file);
    assert!(text.starts_with("ISO-10303-21;"));
    assert!(text.contains("AUTOMOTIVE_DESIGN"), "AP214 by default");
    assert_eq!(text.matches("=MANIFOLD_SOLID_BREP('Board'").count(), 1);
    assert!(text.contains("=SHELL_BASED_SURFACE_MODEL("), "the parts");
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_model_that_could_not_be_made_is_reported_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let file = dir.path().join("board.step");
    let task = output(
        &mut app,
        BoardOutputMsg::StepBuilt {
            board,
            path: file.clone(),
            built: Err("the kernel failed".to_owned()),
        },
    );
    assert_eq!(task.units(), 0);
    assert!(app.document_state.error_notice.is_some());
    assert!(!file.exists());
}

#[test]
fn warnings_name_dropped_holes_and_unread_models() {
    let built = BoardStepBuilt {
        step: signex_renderer::board3d::BoardStep {
            bytes: Vec::new(),
            parts: 0,
            dropped_holes: 2,
        },
        unread_models: vec![("lib:fp@1".to_owned(), "not found".to_owned())],
    };
    let warnings = super::step::step_warnings(&built);
    assert_eq!(warnings.len(), 2);
    assert!(warnings[0].contains("2 hole(s)"));
    assert!(warnings[1].contains("lib:fp@1") && warnings[1].contains("not found"));
}

#[test]
fn the_cli_writes_the_board_step_in_the_asked_schema() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let file = dir.path().join("out.stp");
    let document = format!("document={}", board.display());
    let path = format!("path={}", file.display());
    let (outcome, task) = headless(&mut app, &[&document, &path, "schema=ap242"]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["schema"], "ap242");
    settle(&mut app, task);
    let text = read_step(&file);
    assert!(text.contains("AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF"));
    assert!(text.contains("FILE_NAME('out.stp'"));
}

#[test]
fn the_cli_refuses_an_unknown_schema() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());
    let path = format!("path={}", dir.path().join("x.step").display());
    let (outcome, task) = headless(&mut app, &[&document, &path, "schema=iges"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("ap214")),
        "{outcome:?}"
    );
    assert_eq!(task.units(), 0);
}
