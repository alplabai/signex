//! #1298, #2609 — the schematic PDF's watermark colour and links, through
//! `Signex::update` (the Export PDF dialog) and through the headless path
//! (`export_schematic_pdf`).

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_i18n::RenderTranslatable as _;
use signex_types::schematic::{SchematicSheet, TextNote};

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::handlers::menu::export::settle;
use crate::app::state::PdfExportRequest;
use crate::app::{ExportMsg, Message, PdfOptionMsg, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

const BLUE: [u8; 3] = [0x00, 0x00, 0xFF];

/// A sheet with one note whose text carries a web link.
fn sheet_with_link() -> SchematicSheet {
    let note: TextNote = serde_json::from_value(json!({
        "uuid": uuid::Uuid::now_v7(),
        "text": "see [the datasheet](https://example.com/ds.pdf)",
        "position": { "x": 30.0, "y": 30.0 },
        "font_size": 2.54,
    }))
    .expect("text note");
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".into(),
        text_notes: vec![note],
        ..Default::default()
    }
}

fn app_with_sheet(dir: &Path) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let sheet = dir.join("main.snxsch");
    add_sheet_with(&mut app, &sheet, sheet_with_link());
    (app, sheet)
}

fn run(app: &mut Signex, id: &str) {
    let _ = app.update(Message::Menu(MenuMessage::command(id)));
}

fn pdf_option(app: &mut Signex, msg: PdfOptionMsg) {
    let _ = app.update(Message::from(msg));
}

fn pdf_text(file: &Path) -> String {
    let bytes = std::fs::read(file).expect("the PDF was written");
    assert!(bytes.starts_with(b"%PDF-"));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// What the dialog's Export PDF… hands on once the save dialog answers.
fn export_from_dialog(app: &mut Signex, file: &Path) {
    let preview = app.document_view.preview.as_ref().expect("dialog open");
    let request = PdfExportRequest {
        source: preview.source.clone(),
        options: preview.output_options(),
        files: preview.selected_files.clone(),
        descended_placement: None,
    };
    let task = app.update(Message::Export(ExportMsg::PdfFinished {
        request: Box::new(request),
        result: Ok(file.to_path_buf()),
    }));
    settle(app, task);
}

#[test]
fn the_command_opens_export_pdf_on_the_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet) = app_with_sheet(dir.path());
    run(&mut app, "export_schematic_pdf");
    let preview = app.document_view.preview.as_ref();
    assert_eq!(preview.map(|p| &p.source), Some(&sheet), "Export PDF opens");
}

#[test]
fn the_picked_colour_prints_and_the_next_opening_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_sheet(dir.path());
    run(&mut app, "export_schematic_pdf");
    pdf_option(&mut app, PdfOptionMsg::WatermarkToggled(true));
    let typed = PdfOptionMsg::WatermarkColourChanged("#0000ff".into());
    pdf_option(&mut app, typed);
    let preview = app.document_view.preview.as_ref().expect("open");
    let printed = preview.output_options().watermark.expect("on");
    assert_eq!(printed.colour_rgb, BLUE, "the live preview draws this one");

    let file = dir.path().join("dialog.pdf");
    export_from_dialog(&mut app, &file);
    let text = pdf_text(&file);
    assert!(text.contains("0 0 1 rg\n"), "the watermark prints in blue");
    assert!(
        text.contains("/URI (https://example.com/ds.pdf)"),
        "the note's link is a link"
    );

    let _ = app.handle_print_preview_close();
    run(&mut app, "export_schematic_pdf");
    let reopened = &app.document_view.preview.as_ref().expect("open").dialog;
    assert_eq!(reopened.watermark.settings.colour_rgb, BLUE, "remembered");
    assert_eq!(reopened.watermark.colour_input, "#0000FF");
}

#[test]
fn a_swatch_picks_its_colour_and_a_bad_code_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_sheet(dir.path());
    run(&mut app, "export_schematic_pdf");
    let grey = crate::app::state::WATERMARK_SWATCHES[1];
    pdf_option(&mut app, PdfOptionMsg::WatermarkColourPicked(grey));
    pdf_option(&mut app, PdfOptionMsg::WatermarkColourChanged("#12".into()));
    let preview = app.document_view.preview.as_ref().expect("open");
    let draft = &preview.dialog.watermark;
    assert_eq!(draft.settings.colour_rgb, grey);
    assert_eq!(draft.colour_input, "#12", "kept as typed");
}

fn headless(app: &mut Signex, operands: &[&str]) -> (CommandOutcome, iced::Task<Message>) {
    let command = AppCommandId::new("export_schematic_pdf").unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args)
}

#[test]
fn the_cli_writes_a_watermarked_pdf_with_links() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet) = app_with_sheet(dir.path());
    let file = dir.path().join("cli.pdf");
    let document = format!("document={}", sheet.display());
    let path = format!("path={}", file.display());
    let (outcome, task) = headless(
        &mut app,
        &[
            &document,
            &path,
            "watermark=confidential",
            "watermark_color=#0000ff",
            "watermark_opacity=40",
            "watermark_angle=30",
            "watermark_pages=first",
        ],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["sheets"], 1);
    assert_eq!(answer["links"], true);
    assert_eq!(
        answer["watermark"],
        json!({
            "text": "CONFIDENTIAL",
            "color": "#0000FF",
            "opacity": 40,
            "angle": 30.0,
            "pages": "first",
            "revision_line": null,
        })
    );
    settle(&mut app, task);
    let text = pdf_text(&file);
    assert!(text.contains("0 0 1 rg\n"), "the watermark prints in blue");
    assert!(text.contains("/ca 0.4"), "at 40 %");
    assert!(text.contains("/URI (https://example.com/ds.pdf)"));
    assert!(app.document_view.preview.is_none(), "no dialog opens");
}

#[test]
fn the_cli_writes_no_watermark_by_default_and_no_links_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet) = app_with_sheet(dir.path());
    let file = dir.path().join("plain.pdf");
    let document = format!("document={}", sheet.display());
    let path = format!("path={}", file.display());
    let (outcome, task) = headless(&mut app, &[&document, &path, "links=false"]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["watermark"], serde_json::Value::Null);
    assert_eq!(answer["links"], false);
    settle(&mut app, task);
    let text = pdf_text(&file);
    assert!(!text.contains("/Annots"), "no link annotation");
    assert!(!text.contains("/ca "), "no watermark opacity state");
}

#[test]
fn the_cli_refuses_a_bad_watermark_operand() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet) = app_with_sheet(dir.path());
    let document = format!("document={}", sheet.display());
    let path = format!("path={}", dir.path().join("x.pdf").display());
    for (bad, word) in [
        ("watermark_opacity=150", "`watermark_opacity`"),
        ("watermark_angle=720", "`watermark_angle`"),
        ("watermark_pages=odd", "`watermark_pages`"),
    ] {
        let operands = [document.as_str(), path.as_str(), "watermark=draft", bad];
        let (outcome, _) = headless(&mut app, &operands);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains(word)),
            "{bad}: {outcome:?}"
        );
    }
    let style_alone = [document.as_str(), path.as_str(), "watermark_color=#0000ff"];
    let (outcome, _) = headless(&mut app, &style_alone);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("needs `watermark`")),
        "{outcome:?}"
    );
    assert!(!dir.path().join("x.pdf").exists(), "nothing is written");
}

#[test]
fn the_cli_refuses_a_document_that_is_not_an_open_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_sheet(dir.path());
    let path = format!("path={}", dir.path().join("x.pdf").display());
    let missing = format!("document={}", dir.path().join("other.snxsch").display());
    let (outcome, _) = headless(&mut app, &[&missing, &path]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
}
