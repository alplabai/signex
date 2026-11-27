//! #2470 — `fabrication_outputs` from its operands: headless, as the CLI
//! runs it, and typed with operands in a window; the same files as the
//! dialog's Generate, each listed with its kind and SHA-256, and no undo
//! step.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use signex_i18n::RenderTranslatable;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::{FabOutput, FabOutputsMsg};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

/// R1 on the top with two pads on VCC and GND, a track and a via, inside a
/// 50 x 30 mm outline.
fn board(outline: bool) -> PcbBoard {
    let pad = |number: &str, net: u32, name: &str, x: f64| {
        serde_json::json!({
            "uuid": Uuid::now_v7(), "number": number, "pad_type": "smd",
            "shape": "rect", "position": {"x": x, "y": 0.0},
            "size": {"x": 1.0, "y": 1.0}, "drill": null,
            "net": {"number": net, "name": name}
        })
    };
    let outline = if outline {
        serde_json::json!([
            {"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0},
            {"x": 50.0, "y": 30.0}, {"x": 0.0, "y": 30.0}
        ])
    } else {
        serde_json::json!([])
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "outline": outline,
        "nets": [{"number": 1, "name": "VCC"}, {"number": 2, "name": "GND"}],
        "footprints": [{
            "uuid": Uuid::now_v7(), "reference": "R1", "value": "10k",
            "footprint_id": "R_0603", "position": {"x": 10.0, "y": 10.0},
            "layer": "TopCopper",
            "pads": [pad("1", 1, "VCC", -0.8), pad("2", 2, "GND", 0.8)]
        }],
        "segments": [{
            "uuid": Uuid::now_v7(), "start": {"x": 2.0, "y": 2.0},
            "end": {"x": 8.0, "y": 2.0}, "width": 0.25,
            "layer": "TopCopper", "net": 1
        }],
        "vias": [{"uuid": Uuid::now_v7(), "position": {"x": 8.0, "y": 2.0},
                  "diameter": 0.6, "drill": 0.3, "net": 1}]
    }))
    .expect("the fixture board")
}

/// An app with the board open in its tab, in a project that names it.
fn app_with_board(dir: &Path, outline: bool) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board(outline));
    (app, path)
}

fn command() -> AppCommandId {
    AppCommandId::new("fabrication_outputs").unwrap()
}

/// The command run headless, as `signex-cli run` runs it: its outcome once
/// the files are written.
fn headless(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let args = CommandArgs::parse_operands(&command(), operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command(), args);
    settle(app, task);
    outcome
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn operand(name: &str, value: impl std::fmt::Display) -> String {
    format!("{name}={value}")
}

fn sha256(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("written");
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn undo_depth(app: &Signex, board: &Path) -> usize {
    app.document_state
        .board_engine(board)
        .expect("the board's engine")
        .undo_depth()
}

#[test]
fn the_cli_writes_the_named_kinds_and_lists_each_file_with_its_hash() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path(), true);
    let out = dir.path().join("fab");
    let depth = undo_depth(&app, &board);
    let answer = answered(headless(
        &mut app,
        &[
            &operand("document", board.display()),
            &operand("path", out.display()),
            "kinds=gerber,drill,centroid,odb_folder",
        ],
    ));
    assert_eq!(answer["document"], board.display().to_string());
    assert_eq!(answer["folder"], out.display().to_string());
    assert_eq!(answer["name"], "board", "the board's file name");
    assert_eq!(
        answer["kinds"],
        serde_json::json!(["gerber", "drill", "centroid", "odb_folder"])
    );
    assert_eq!(answer["whole_panel"], false, "the board has no panel");
    let files = answer["files"].as_array().expect("files");
    let kinds: Vec<&str> = files.iter().filter_map(|f| f["kind"].as_str()).collect();
    for kind in ["gerber", "drill", "centroid", "odb", "job"] {
        assert!(kinds.contains(&kind), "{kind}: {kinds:?}");
    }
    for kind in ["ipc2581", "test_netlist", "assembly_pdf", "step"] {
        assert!(!kinds.contains(&kind), "{kind} was not asked for");
    }
    for file in files {
        let path = PathBuf::from(file["path"].as_str().expect("path"));
        assert!(path.starts_with(&out), "{path:?}");
        assert_eq!(file["sha256"], sha256(&path), "{path:?}");
        let size = std::fs::metadata(&path).expect("written").len();
        assert_eq!(file["bytes"], size, "{path:?}");
    }
    let centroid = files
        .iter()
        .find(|f| f["kind"] == "centroid")
        .expect("the pick-and-place file");
    assert_eq!(centroid["name"], "board-centroid.csv");
    let odb = files.iter().find(|f| f["kind"] == "odb").expect("odb");
    assert!(
        odb["name"]
            .as_str()
            .is_some_and(|n| n.starts_with("board-odb/")),
        "the ODB++ job as its folder: {odb}"
    );
    assert!(
        app.ui_state.fab_outputs.is_none(),
        "no dialog opens without a window"
    );
    assert_eq!(undo_depth(&app, &board), depth, "no undo step");
}

/// Without `kinds` the command writes what the dialog's Generate writes:
/// the project's ticks, the same names and the same bytes.
#[test]
fn the_cli_writes_the_same_files_as_the_dialogs_generate() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path(), true);
    let by_dialog = dir.path().join("by-dialog");
    let task = app.update(Message::Menu(MenuMessage::command("fabrication_outputs")));
    settle(&mut app, task);
    for msg in [
        FabOutputsMsg::OutputToggled(FabOutput::AssemblyPdf),
        FabOutputsMsg::FolderEdited(by_dialog.display().to_string()),
        FabOutputsMsg::GenerateRequested,
    ] {
        let task = app.update(Message::from(msg));
        settle(&mut app, task);
    }
    let shown = app.ui_state.fab_outputs.as_ref().expect("the dialog");
    let Some(Ok(written)) = &shown.outcome else {
        panic!("Generate wrote nothing: {:?}", shown.outcome);
    };
    let dialog_files = written.files.clone();
    let _ = app.update(Message::from(FabOutputsMsg::CancelRequested));

    let by_command = dir.path().join("by-command");
    let answer = answered(headless(
        &mut app,
        &[
            &operand("document", board.display()),
            &operand("path", by_command.display()),
        ],
    ));
    assert!(
        !answer["kinds"]
            .as_array()
            .expect("kinds")
            .contains(&serde_json::json!("assembly_pdf")),
        "the project's ticks: {answer}"
    );
    let names: Vec<String> = answer["files"]
        .as_array()
        .expect("files")
        .iter()
        .filter_map(|f| f["name"].as_str().map(str::to_string))
        .collect();
    assert_eq!(names, dialog_files);
    for name in &names {
        let (dialog, command) = (by_dialog.join(name), by_command.join(name));
        assert_eq!(
            std::fs::read(&dialog).expect("the dialog's file"),
            std::fs::read(&command).expect("the command's file"),
            "{name}"
        );
    }
}

/// The operand handler answers only once the worker has written the files
/// (`CommandOutcome::Deferred`), and opens no dialog.
#[test]
fn the_operand_run_is_answered_once_the_files_are_written() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path(), true);
    let out = dir.path().join("fab");
    let document = operand("document", board.display());
    let path = operand("path", out.display());
    let args = CommandArgs::parse_operands(&command(), &[&document, &path, "kinds=ipc2581"])
        .expect("operands parse");
    let (outcome, task) = app.run_operands(&command(), &args);
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("answered once written: {outcome:?}");
    };
    assert!(!out.exists(), "nothing is written inside `update`");
    settle(&mut app, task);
    let answer = answered(app.take_finished_edit(ticket).expect("the answer"));
    let files = answer["files"].as_array().expect("files");
    assert_eq!(
        files.len(),
        2,
        "the IPC-2581 file and the job file: {answer}"
    );
    assert_eq!(files[0]["kind"], "ipc2581");
    assert!(out.join("board-ipc2581.xml").is_file());
    assert!(app.ui_state.fab_outputs.is_none(), "no dialog");
}

#[test]
fn a_bad_kind_or_a_board_without_an_outline_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path(), true);
    let out = dir.path().join("fab");
    let document = operand("document", board.display());
    let path = operand("path", out.display());

    let reason = refusal(headless(&mut app, &[&document, &path, "kinds=gerber,dxf"]));
    assert!(
        reason.contains("`dxf`") && reason.contains("gerber"),
        "{reason}"
    );
    let reason = refusal(headless(&mut app, &[&document, &path, "kinds=,"]));
    assert!(reason.contains("no kind"), "{reason}");
    assert!(!out.exists(), "nothing written");

    let (mut bare, board) = app_with_board(dir.path(), false);
    let document = operand("document", board.display());
    let reason = refusal(headless(&mut bare, &[&document, &path]));
    assert!(reason.contains("no outline"), "{reason}");
    assert!(!out.exists(), "nothing written");
}
