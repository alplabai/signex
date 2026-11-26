//! #2796 — a block diagram exported as SVG or PDF: through the window's
//! dialogs (`Signex::update`) and through `export_block_diagram` with
//! operands, the path a script and `signex-cli` take.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable as _;

use super::BlockDiagramExportMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::io_ports::{HeadlessAnswers, IoPorts};
use crate::keymap::AppCommandId;

const DCDC: &str =
    include_str!("../../../../signex-types/tests/corpus/block_diagram/dcdc-buck.snxblk");

/// An app with a project in `dir` and the corpus diagram saved beside it.
fn app_with_diagram(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    let diagram = dir.join("dcdc-buck.snxblk");
    std::fs::write(&diagram, DCDC).expect("write the diagram");
    (app, diagram)
}

fn send(app: &mut Signex, msg: BlockDiagramExportMsg) {
    let task = app.update(Message::from(msg));
    settle(app, task);
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new("export_block_diagram").expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    let (outcome, task) = app.run_operands(&id, &args);
    match outcome {
        CommandOutcome::Deferred { ticket } => app.settle_deferred(ticket, task).0,
        refused => refused,
    }
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

#[test]
fn the_save_dialogs_answer_writes_the_svg_the_extension_names() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, diagram) = app_with_diagram(dir.path());
    let path = dir.path().join("blocks.svg");
    send(
        &mut app,
        BlockDiagramExportMsg::PathChosen {
            diagram,
            path: path.clone(),
        },
    );
    let svg = std::fs::read_to_string(&path).expect("written");
    assert!(svg.starts_with("<?xml") && svg.contains(">DC/DC Buck</text>"));
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_picked_diagram_asks_for_the_file_and_writes_a_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, diagram) = app_with_diagram(dir.path());
    let answers = HeadlessAnswers::default();
    app.ui_state.io = IoPorts::headless(answers.clone());
    let path = dir.path().join("blocks.pdf");
    answers.set("path", path.display().to_string());
    send(&mut app, BlockDiagramExportMsg::DiagramPicked(diagram));
    let bytes = std::fs::read(&path).expect("written");
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn a_file_that_is_neither_svg_nor_pdf_or_the_diagram_itself_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, diagram) = app_with_diagram(dir.path());
    let before = std::fs::read(&diagram).unwrap();
    let path = dir.path().join("blocks.png");
    send(
        &mut app,
        BlockDiagramExportMsg::PathChosen {
            diagram: diagram.clone(),
            path: path.clone(),
        },
    );
    assert!(!path.exists());
    let notice = app.document_state.error_notice.take().expect("a notice");
    assert!(notice.detail.render_en_us().contains("blocks.png"));

    // The diagram's own name is not .svg or .pdf, so it is never written.
    send(
        &mut app,
        BlockDiagramExportMsg::PathChosen {
            diagram: diagram.clone(),
            path: diagram.clone(),
        },
    );
    assert!(app.document_state.error_notice.is_some());
    assert_eq!(std::fs::read(&diagram).unwrap(), before, "never written");
}

#[test]
fn the_command_writes_the_file_and_answers_once_it_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, diagram) = app_with_diagram(dir.path());
    let document = format!("document={}", diagram.display());
    let path = dir.path().join("out.svg");
    let target = format!("path={}", path.display());
    let answer = answered(run(&mut app, &[&document, &target]));
    assert_eq!(answer["kind"], "svg");
    assert_eq!(answer["blocks"], 6);
    assert_eq!(answer["connections"], 6);
    assert_eq!(answer["groups"], 1);
    assert_eq!(answer["read_only"], false);
    assert_eq!(answer["scale"], serde_json::Value::Null);
    let written = std::fs::read(&path).expect("written before the answer");
    assert_eq!(answer["bytes"], written.len());
    assert!(
        app.document_state.error_notice.is_none(),
        "no dialog, no card"
    );

    // `kind` wins over the extension; the PDF is fitted to the page.
    let pdf = dir.path().join("out.print");
    let target = format!("path={}", pdf.display());
    let answer = answered(run(
        &mut app,
        &[&document, &target, "kind=pdf", "monochrome=true"],
    ));
    assert_eq!(answer["kind"], "pdf");
    assert!(answer["scale"].as_f64().is_some_and(|s| s > 0.0));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn a_refused_command_writes_nothing_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, diagram) = app_with_diagram(dir.path());
    let document = format!("document={}", diagram.display());
    let out = dir.path().join("out.svg");
    let target = format!("path={}", out.display());

    let why = refusal(run(&mut app, &[&document, &target, "kind=png"]));
    assert!(why.contains("png"), "{why}");
    let unknown = format!("path={}", dir.path().join("out.txt").display());
    let why = refusal(run(&mut app, &[&document, &unknown]));
    assert!(why.contains("kind"), "{why}");
    let sheet = format!("document={}", dir.path().join("main.snxsch").display());
    let why = refusal(run(&mut app, &[&sheet, &target]));
    assert!(why.contains("not a block diagram"), "{why}");
    assert!(!out.exists());
    let before = std::fs::read(&diagram).unwrap();
    let onto_itself = format!("path={}", diagram.display());
    let why = refusal(run(&mut app, &[&document, &onto_itself, "kind=svg"]));
    assert!(why.contains("block diagram itself"), "{why}");
    assert_eq!(std::fs::read(&diagram).unwrap(), before, "never written");

    // Checked up front, then read: a file that is gone fails after the read.
    let gone = format!("document={}", dir.path().join("gone.snxblk").display());
    let why = refusal(run(&mut app, &[&gone, &target]));
    assert!(why.contains("gone.snxblk"), "{why}");
    assert!(!out.exists());
}
