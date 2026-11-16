//! #2402 — Board Print / PDF through `Signex::update` (File ▸ Print, Export
//! ▸ PDF, the dialog's options and Export) and through the headless path.

use signex_output::{BoardLayerSet, BoardPrintOptions, BoardPrintPages, BoardPrintScale};

use super::tests::{app_with_board, output, run};
use super::{BoardOutputDialog, BoardOutputMsg, BoardPrintFlag};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

fn shown_options(app: &Signex) -> Option<BoardPrintOptions> {
    match app.ui_state.board_output.dialog.as_ref()? {
        BoardOutputDialog::Print(state) => Some(state.options.clone()),
        BoardOutputDialog::Reports(_) => None,
    }
}

/// `/Type /Page` objects in a PDF, the page tree left out.
fn pages_in(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page").count() - text.matches("/Type /Pages").count()
}

#[test]
fn print_and_export_pdf_open_board_print_on_a_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    assert!(
        crate::app::command::is_enabled(
            &AppCommandId::new("print").unwrap(),
            &app.command_context(None)
        ),
        "Print is live on a board"
    );

    run(&mut app, "print");
    assert_eq!(shown_options(&app), Some(BoardPrintOptions::default()));
    assert!(app.document_state.preview.is_none(), "no sheet preview");

    let _ = output(&mut app, BoardOutputMsg::Closed);
    let _ = app.update(Message::Menu(MenuMessage::ExportPdf));
    assert!(shown_options(&app).is_some(), "Export > PDF opens it too");

    let _ = output(&mut app, BoardOutputMsg::Closed);
    run(&mut app, "export_board_pdf");
    assert!(shown_options(&app).is_some(), "and its own command");
}

#[test]
fn the_options_change_and_the_next_opening_keeps_them() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "print");
    let _ = output(
        &mut app,
        BoardOutputMsg::PrintSetToggled(BoardLayerSet::Mechanical),
    );
    let _ = output(
        &mut app,
        BoardOutputMsg::PrintSetToggled(BoardLayerSet::Inner),
    );
    let _ = output(
        &mut app,
        BoardOutputMsg::PrintPagesPicked(BoardPrintPages::Composite),
    );
    let _ = output(
        &mut app,
        BoardOutputMsg::PrintScalePicked(BoardPrintScale::OneToOne),
    );
    let _ = output(&mut app, BoardOutputMsg::PrintMonochromePicked(true));
    let _ = output(
        &mut app,
        BoardOutputMsg::PrintFlagToggled(BoardPrintFlag::TitleBlock),
    );
    let options = shown_options(&app).expect("open");
    assert_eq!(
        options.sets,
        [
            BoardLayerSet::Top,
            BoardLayerSet::Bottom,
            BoardLayerSet::Inner
        ]
    );
    assert_eq!(options.pages, BoardPrintPages::Composite);
    assert_eq!(options.scale, BoardPrintScale::OneToOne);
    assert!(options.monochrome);
    assert!(!options.title_block);

    let _ = output(&mut app, BoardOutputMsg::Closed);
    run(&mut app, "print");
    assert_eq!(shown_options(&app), Some(options), "remembered");
}

#[test]
fn export_writes_one_page_per_chosen_set() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    run(&mut app, "print");
    let options = shown_options(&app).expect("open");
    assert_eq!(options.page_plan().len(), 3, "top, bottom, mechanical");
    let file = dir.path().join("board.pdf");
    let task = output(
        &mut app,
        BoardOutputMsg::PdfPathChosen {
            board,
            options: Box::new(options),
            path: file.clone(),
        },
    );
    settle(&mut app, task);
    let bytes = std::fs::read(&file).expect("the PDF was written");
    assert!(bytes.starts_with(b"%PDF-"));
    assert_eq!(pages_in(&bytes), 3);
}

#[test]
fn export_is_refused_with_no_set_chosen() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "print");
    for set in [
        BoardLayerSet::Top,
        BoardLayerSet::Bottom,
        BoardLayerSet::Mechanical,
    ] {
        let _ = output(&mut app, BoardOutputMsg::PrintSetToggled(set));
    }
    let task = app.dispatch_board_output_message(BoardOutputMsg::PdfExportRequested);
    assert_eq!(task.units(), 0, "no save dialog");
    assert!(
        app.document_state.error_notice.is_some(),
        "the notice says why"
    );
}

fn headless(app: &mut Signex, operands: &[&str]) -> (CommandOutcome, iced::Task<Message>) {
    let command = AppCommandId::new("export_board_pdf").unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args)
}

#[test]
fn the_cli_prints_a_board_at_one_to_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let file = dir.path().join("top.pdf");
    let document = format!("document={}", board.display());
    let path = format!("path={}", file.display());
    let (outcome, task) = headless(
        &mut app,
        &[&document, &path, "layers=top,bottom", "scale=one_to_one"],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    let pages = answer["pages"].as_array().expect("pages");
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0]["scale"], 1.0);
    assert_eq!(pages[0]["printed_size_mm"], serde_json::json!([50.0, 30.0]));
    assert_eq!(pages[1]["mirrored"], true, "the bottom reads from below");
    settle(&mut app, task);
    let bytes = std::fs::read(&file).expect("written");
    assert_eq!(pages_in(&bytes), 2);
    assert!(
        app.ui_state.board_output.dialog.is_none(),
        "no dialog opens"
    );
}

#[test]
fn the_cli_warns_when_a_one_to_one_board_runs_off_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.path().to_string_lossy(),
        &[],
    );
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let board = dir.path().join("board.snxpcb");
    // 400 mm x 300 mm: larger than A4 at 1:1.
    let mut large = super::tests::board();
    for point in large.outline.iter_mut() {
        point.x *= 8.0;
        point.y *= 10.0;
    }
    app.open_pcb_tab(board.clone(), "board".into(), large);
    let document = format!("document={}", board.display());

    let path = format!("path={}", dir.path().join("fit.pdf").display());
    let (outcome, _) = headless(&mut app, &[&document, &path, "layers=top"]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["pages"][0]["overflows"], false, "fitted");
    assert_eq!(answer["warnings"], serde_json::json!([]));

    let path = format!("path={}", dir.path().join("full.pdf").display());
    let (outcome, _) = headless(
        &mut app,
        &[&document, &path, "layers=top", "scale=one_to_one"],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["pages"][0]["overflows"], true);
    let warnings = answer["warnings"].as_array().expect("warnings");
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().is_some_and(|w| w.contains("runs off page(s) 1"))),
        "{warnings:?}"
    );
}

#[test]
fn the_cli_refuses_an_unknown_option() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());
    let path = format!("path={}", dir.path().join("x.pdf").display());
    for (bad, word) in [
        ("pages=booklet", "`pages`"),
        ("scale=2:1", "`scale`"),
        ("layers=gerber", "layer set"),
    ] {
        let (outcome, _) = headless(&mut app, &[&document, &path, bad]);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains(word)),
            "{bad}: {outcome:?}"
        );
    }
}
