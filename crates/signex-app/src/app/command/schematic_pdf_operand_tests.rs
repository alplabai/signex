//! #2663: `export_schematic_pdf` headless (every page of the named sheet's
//! project, the contents rows as links) and in a window (the sheet Print
//! Preview, whose Export writes the same links).

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_types::schematic::{Point, SchDrawing, SchTable, SchematicSheet, TableSource};

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::state::PdfExportRequest;
use crate::app::{ExportMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

/// A contents sheet whose table lists itself and power.snxsch.
fn contents_sheet() -> SchematicSheet {
    let mut table = SchTable::new_typed(uuid::Uuid::from_u128(1), Point::new(20.0, 20.0));
    table.source = TableSource::ProjectSheets { per_channel: true };
    table.columns = SchTable::columns_for(&table.source);
    table.rows = vec![
        vec!["1".into(), "Contents".into()],
        vec!["2".into(), "Power".into()],
    ];
    table.links = vec!["contents.snxsch".into(), "power.snxsch".into()];
    let mut sheet = SchematicSheet::default();
    sheet.drawings.push(SchDrawing::Table(table));
    sheet
}

/// A two-page project in `dir`, the contents sheet in view.
fn project(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &["contents.snxsch", "power.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    let contents = dir.join("contents.snxsch");
    let power = dir.join("power.snxsch");
    open_with(&mut app.document_state, &contents, contents_sheet());
    open_with(&mut app.document_state, &power, SchematicSheet::default());
    app.document_state.active_path = Some(contents.clone());
    (app, contents, power)
}

fn headless(app: &mut Signex, operands: &[&str]) -> (CommandOutcome, iced::Task<Message>) {
    let command = AppCommandId::new("export_schematic_pdf").expect("a command id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args)
}

fn links_in(file: &Path) -> usize {
    let bytes = std::fs::read(file).expect("written");
    String::from_utf8_lossy(&bytes)
        .matches("/Subtype /Link")
        .count()
}

#[test]
fn headless_it_writes_every_page_and_answers_with_the_linked_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, contents, power) = project(dir.path());
    let file = dir.path().join("out.pdf");
    let document = format!("document={}", contents.display());
    let path = format!("path={}", file.display());
    let (outcome, task) = headless(&mut app, &[&document, &path]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    let pages: Vec<String> = answer["pages"]
        .as_array()
        .expect("pages")
        .iter()
        .map(|p| p.as_str().expect("a path").to_string())
        .collect();
    let page_of = |file: &Path| {
        let at = pages.iter().position(|p| Path::new(p) == file);
        at.map(|i| i + 1).expect("a printed page")
    };
    let links = answer["table_links"].as_array().expect("links");
    assert_eq!(links.len(), 2, "{answer}");
    assert_eq!(links[0]["link"], "contents.snxsch");
    assert_eq!(links[0]["target_page"], page_of(&contents));
    assert_eq!(links[1]["row"], 2);
    assert_eq!(links[1]["target_page"], page_of(&power));
    assert_eq!(links[1]["page"], page_of(&contents));
    settle(&mut app, task);
    assert_eq!(links_in(&file), 2, "one link annotation per row");
    assert!(app.document_view.preview.is_none(), "no preview opens");
}

#[test]
fn headless_a_board_or_an_unknown_document_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _, _) = project(dir.path());
    let file = dir.path().join("none.pdf");
    let path = format!("path={}", file.display());
    let nowhere = format!("document={}", dir.path().join("gone.snxsch").display());
    let (outcome, task) = headless(&mut app, &[&nowhere, &path]);
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(
        reason.render_en_us().contains("not an open sheet"),
        "{reason:?}"
    );
    settle(&mut app, task);
    assert!(!file.exists());
}

#[test]
fn in_a_window_it_opens_print_preview_and_its_export_writes_the_links() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, contents, _) = project(dir.path());
    let _ = app.update(Message::Menu(MenuMessage::command("export_schematic_pdf")));
    let preview = app
        .document_view
        .preview
        .as_ref()
        .expect("Print Preview opens");
    assert_eq!(preview.source, contents);
    let request = PdfExportRequest {
        source: preview.source.clone(),
        options: Default::default(),
        files: preview.selected_files.clone(),
        descended_placement: None,
    };
    let file = dir.path().join("gui.pdf");
    let task = app.update(Message::Export(ExportMsg::PdfFinished {
        request: Box::new(request),
        result: Ok(file.clone()),
    }));
    settle(&mut app, task);
    assert_eq!(
        links_in(&file),
        2,
        "the preview's Export writes the links too"
    );
}
