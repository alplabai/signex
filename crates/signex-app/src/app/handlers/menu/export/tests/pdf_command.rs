//! #2610, #2787: `export_schematic_pdf`, the headless path of Export PDF.
//! The CLI names the sheet and the file; the pages take the Pages box's
//! list, the layers include the Not-fitted parts group, and the answer
//! names the pages the file prints.

use std::path::Path;

use signex_types::schematic::{Point, SchematicSheet, Symbol};

use super::{app_workspace, open_with};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// The project's sheets, numbered 1, 2 and 3 in this order.
pub(super) const SHEETS: [&str; 3] = ["a.snxsch", "b.snxsch", "c.snxsch"];

/// A project of [`SHEETS`] under `dir`, all open, the first active; the
/// first sheet holds R1, marked DNP.
pub(super) fn three_sheets(dir: &Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &SHEETS);
    for (n, name) in SHEETS.iter().enumerate() {
        let mut sheet = SchematicSheet {
            uuid: uuid::Uuid::from_u128(0x2787_0000 + n as u128),
            ..Default::default()
        };
        if n == 0 {
            sheet.symbols.push(Symbol {
                uuid: uuid::Uuid::from_u128(0x2610),
                reference: "R1".to_string(),
                position: Point::new(50.0, 50.0),
                dnp: true,
                ..Default::default()
            });
        }
        open_with(&mut app.document_state, &dir.join(name), sheet);
    }
    app.document_state.active_path = Some(dir.join(SHEETS[0]));
    app
}

fn run(app: &mut Signex, operands: &[&str]) -> (CommandOutcome, iced::Task<Message>) {
    let command = AppCommandId::new("export_schematic_pdf").unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args)
}

fn pages_in(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page").count() - text.matches("/Type /Pages").count()
}

#[test]
fn the_cli_prints_the_pages_the_box_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = three_sheets(dir.path());
    let file = dir.path().join("out.pdf");
    let document = format!("document={}", dir.path().join(SHEETS[0]).display());
    let path = format!("path={}", file.display());
    let (outcome, task) = run(&mut app, &[&document, &path, "pages=1, 3"]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["pages"], serde_json::json!([1, 3]));
    assert_eq!(answer["variant"], "Base", "the project's active variant");
    assert_eq!(answer["layers"], serde_json::json!([]), "a flat PDF");
    settle(&mut app, task);
    let bytes = std::fs::read(&file).expect("written");
    assert_eq!(pages_in(&bytes), 2);
    assert!(app.document_state.preview.is_none(), "no dialog opens");
}

#[test]
fn the_cli_writes_the_not_fitted_layer() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = three_sheets(dir.path());
    let file = dir.path().join("layers.pdf");
    let document = format!("document={}", dir.path().join(SHEETS[0]).display());
    let path = format!("path={}", file.display());
    let (outcome, task) = run(
        &mut app,
        &[
            &document,
            &path,
            "pages=1",
            "layers=wires,parts,title_block",
        ],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    let layers = answer["layers"].as_array().expect("layers");
    assert!(
        layers.iter().any(|name| name == "Not-fitted parts"),
        "{layers:?}"
    );
    settle(&mut app, task);
    let text = String::from_utf8_lossy(&std::fs::read(&file).expect("written")).into_owned();
    assert!(text.contains("/Name (Not-fitted parts)"), "the group");
    // R1 is marked DNP: its drawing is in the group (the sixth, `OC5`), and
    // the group is not listed among those visible on open.
    let begin = signex_output::PdfLayer::ALL
        .iter()
        .position(|layer| *layer == signex_output::PdfLayer::NotFittedParts)
        .map(|index| format!("/OC /OC{index} BDC"))
        .expect("listed");
    assert!(text.contains(&begin), "R1 draws in the group");
    let off_at = text.find("/OFF").expect("hidden groups");
    let off = &text[off_at..off_at + text[off_at..].find(']').expect("array")];
    assert_eq!(
        off.matches(" 0 R").count(),
        3,
        "net labels, power ports, not fitted: {off}"
    );
}

/// In a window the command opens the Export PDF dialog on the sheet in
/// view, as `print` does; it is live only on a sheet.
#[test]
fn in_a_window_the_command_opens_export_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = three_sheets(dir.path());
    let command = AppCommandId::new("export_schematic_pdf").unwrap();
    assert!(crate::app::command::is_enabled(
        &command,
        &app.command_context(None)
    ));
    let task = app.update(Message::Menu(crate::menu_bar::MenuMessage::command(
        "export_schematic_pdf",
    )));
    settle(&mut app, task);
    let preview = app.document_state.preview.as_ref().expect("the dialog");
    assert_eq!(preview.source, dir.path().join(SHEETS[0]));

    app.document_state.preview = None;
    app.document_state.active_path = None;
    assert!(!crate::app::command::is_enabled(
        &command,
        &app.command_context(None)
    ));
}

#[test]
fn the_cli_refuses_what_the_dialog_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = three_sheets(dir.path());
    let document = format!("document={}", dir.path().join(SHEETS[0]).display());
    let path = format!("path={}", dir.path().join("no.pdf").display());
    for bad in ["pages=1-", "pages=9", "layers=roads", "variant=Lite"] {
        let (outcome, _) = run(&mut app, &[&document, &path, bad]);
        assert!(
            matches!(outcome, CommandOutcome::Disabled { .. }),
            "{bad}: {outcome:?}"
        );
    }
    assert!(!dir.path().join("no.pdf").exists());
}
