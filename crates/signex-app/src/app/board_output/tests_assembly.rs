//! #2486 — the assembly drawings: as SVG files from Board Print / PDF and
//! `export_assembly_svg`, as SVG text for an agent, and as PDF pages.

use signex_output::{BoardLayerSet, BoardPrintOptions};

use super::tests::{app_with_board, output, run};
use super::{BoardOutputDialog, BoardOutputMsg};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

fn headless(
    app: &mut Signex,
    id: &str,
    operands: &[&str],
) -> (CommandOutcome, iced::Task<Message>) {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args)
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn the_drawing_command_opens_board_print_in_a_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "board_assembly_drawing");
    assert!(matches!(
        app.ui_state.board_output.dialog.as_deref(),
        Some(BoardOutputDialog::Print(_))
    ));
}

#[test]
fn export_svg_writes_both_drawings_when_no_assembly_set_is_chosen() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let task = output(
        &mut app,
        BoardOutputMsg::SvgFolderChosen {
            board,
            options: Box::new(BoardPrintOptions::default()),
            folder: dir.path().to_path_buf(),
        },
    );
    settle(&mut app, task);
    let top = std::fs::read_to_string(dir.path().join("board-assembly-top.svg")).expect("top");
    let bottom =
        std::fs::read_to_string(dir.path().join("board-assembly-bottom.svg")).expect("bottom");
    assert!(top.contains(">R1</text>") && top.contains(">R2</text>"));
    assert!(!top.contains(">C1</text>"), "C1 is on the bottom");
    assert!(bottom.contains(">C1</text>") && !bottom.contains(">R1</text>"));
}

#[test]
fn export_svg_writes_the_chosen_assembly_sets_only() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let options = BoardPrintOptions {
        sets: vec![BoardLayerSet::Top, BoardLayerSet::AssemblyBottom],
        ..BoardPrintOptions::default()
    };
    let task = output(
        &mut app,
        BoardOutputMsg::SvgFolderChosen {
            board,
            options: Box::new(options),
            folder: dir.path().to_path_buf(),
        },
    );
    settle(&mut app, task);
    assert!(dir.path().join("board-assembly-bottom.svg").exists());
    assert!(!dir.path().join("board-assembly-top.svg").exists());
}

#[test]
fn the_cli_writes_the_named_sides_into_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());
    let path = format!("path={}", dir.path().display());
    let (outcome, task) = headless(
        &mut app,
        "export_assembly_svg",
        &[&document, &path, "sides=bottom", "mirror_bottom=false"],
    );
    let answer = answered(outcome);
    let files = answer["files"].as_array().expect("files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["side"], "bottom");
    assert_eq!(files[0]["mirrored"], false);
    settle(&mut app, task);
    assert!(dir.path().join("board-assembly-bottom.svg").exists());
    assert!(
        app.ui_state.board_output.dialog.is_none(),
        "no dialog opens"
    );

    let (outcome, _) = headless(
        &mut app,
        "export_assembly_svg",
        &[&document, &path, "sides=inner"],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("side")),
        "{outcome:?}"
    );
}

#[test]
fn an_agent_reads_a_drawing_as_svg_text_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());
    let (outcome, task) = headless(
        &mut app,
        "board_assembly_drawing",
        &[&document, "side=bottom"],
    );
    let answer = answered(outcome);
    assert_eq!(task.units(), 0, "no write");
    assert_eq!(answer["side"], "bottom");
    assert_eq!(answer["mirrored"], true, "read from below by default");
    assert_eq!(answer["extent_mm"]["max"], serde_json::json!([50.0, 30.0]));
    let svg = answer["svg"].as_str().expect("svg");
    assert!(svg.starts_with("<?xml") && svg.contains(">C1</text>"));

    let (outcome, _) = headless(&mut app, "board_assembly_drawing", &[&document, "side=up"]);
    assert!(matches!(outcome, CommandOutcome::Disabled { .. }));
}

#[test]
fn the_cli_prints_the_assembly_drawings_as_pdf_pages() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());
    let path = format!("path={}", dir.path().join("assembly.pdf").display());
    let (outcome, task) = headless(
        &mut app,
        "export_board_pdf",
        &[&document, &path, "layers=assembly_top,assembly_bottom"],
    );
    let answer = answered(outcome);
    let pages = answer["pages"].as_array().expect("pages");
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0]["sets"], serde_json::json!(["assembly_top"]));
    assert_eq!(pages[1]["mirrored"], true);
    settle(&mut app, task);
    let bytes = std::fs::read(dir.path().join("assembly.pdf")).expect("written");
    assert!(bytes.starts_with(b"%PDF-"));
}
