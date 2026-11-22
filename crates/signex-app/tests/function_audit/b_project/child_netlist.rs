//! Rows m27 (Create Sheet) and m28 (Netlist incomplete).

use signex_app::app::{ExportMsg, Message, ProjectMsg, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::toolbar::ToolMessage;

use super::support::{Bench, bench};

/// A bench whose root sheet just got a sheet symbol from the Place Sheet
/// tool; its file does not exist, so Create Sheet is up.
fn dialog() -> Bench {
    let mut b = bench(&[("root.snxsch", ""), ("b.snxsch", "")]);
    b.send(Message::Tool(ToolMessage::SelectTool(Tool::Sheet)));
    for (world_x, world_y) in [(100.0, 40.0), (140.0, 70.0)] {
        b.send(Message::CanvasEvent(CanvasEvent::Clicked {
            world_x,
            world_y,
        }));
    }
    assert!(
        b.app.ui_state.create_child_sheet_confirm.is_some(),
        "placing the symbol asks where its file goes"
    );
    b
}

fn type_path(b: &mut Bench, text: &str) {
    b.send(Message::Project(ProjectMsg::CreateChildSheetPathChanged(
        text.to_string(),
    )));
}

fn answer(b: &mut Bench, create: bool) {
    b.send(Message::Project(ProjectMsg::CreateChildSheetAnswered {
        create,
    }));
}

#[test]
fn m27_create_writes_the_file_sets_the_symbols_file_and_lists_it() {
    let mut b = dialog();
    type_path(&mut b, "power.snxsch");
    answer(&mut b, true);
    assert!(b.app.ui_state.create_child_sheet_confirm.is_none());
    assert!(b.file("power.snxsch").is_file());
    assert_eq!(
        b.sheet("root.snxsch").child_sheets[0].filename,
        "power.snxsch"
    );
    assert!(b.listed().contains(&"power.snxsch".to_string()));
}

#[test]
fn m27_later_keeps_the_symbol_and_writes_no_file() {
    let mut b = dialog();
    type_path(&mut b, "power.snxsch");
    answer(&mut b, false);
    assert!(b.app.ui_state.create_child_sheet_confirm.is_none());
    assert!(!b.file("power.snxsch").exists());
    assert_eq!(b.sheet("root.snxsch").child_sheets.len(), 1);
    assert!(!b.listed().contains(&"power.snxsch".to_string()));
}

#[test]
fn m27_a_path_outside_the_project_folder_is_refused() {
    let mut b = dialog();
    type_path(&mut b, "../outside.snxsch");
    answer(&mut b, true);
    let confirm = b
        .app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("the dialog stays");
    assert!(confirm.error.is_some(), "the refusal says why");
    assert!(!b.dir.join("../outside.snxsch").exists());
}

#[test]
fn m27_a_name_that_exists_offers_the_file_and_never_overwrites_it() {
    let mut b = dialog();
    let before = std::fs::read(b.file("b.snxsch")).expect("file");
    type_path(&mut b, "b.snxsch");
    answer(&mut b, true);
    let confirm = b
        .app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("the dialog stays");
    assert!(confirm.exists, "the primary action becomes use-the-file");
    assert!(confirm.error.is_some());
    assert!(
        confirm.reuse_warning.is_some(),
        "the file is already a page of the project: the dialog warns"
    );
    assert_eq!(std::fs::read(b.file("b.snxsch")).expect("file"), before);
}

#[test]
fn m27_editing_the_path_clears_the_exists_state() {
    let mut b = dialog();
    type_path(&mut b, "b.snxsch");
    answer(&mut b, true);
    type_path(&mut b, "free.snxsch");
    let confirm = b
        .app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("the dialog");
    assert!(!confirm.exists);
    assert!(confirm.error.is_none());
    assert!(confirm.reuse_warning.is_none());
}

#[test]
fn m27_use_existing_file_links_the_symbol_without_writing() {
    let mut b = dialog();
    let before = std::fs::read(b.file("b.snxsch")).expect("file");
    type_path(&mut b, "b.snxsch");
    answer(&mut b, true);
    b.send(Message::Project(ProjectMsg::CreateChildSheetOpenExisting));
    assert!(b.app.ui_state.create_child_sheet_confirm.is_none());
    assert_eq!(b.sheet("root.snxsch").child_sheets[0].filename, "b.snxsch");
    assert_eq!(std::fs::read(b.file("b.snxsch")).expect("file"), before);
}

/// A root placing a page whose file does not exist, with Export asked for.
fn prompt() -> (Bench, std::path::PathBuf) {
    let mut b = bench(&[("root.snxsch", "usb.snxsch")]);
    let save = b.file("out.net");
    let source = b.file("root.snxsch");
    b.send(Message::Export(ExportMsg::NetlistFinished {
        source,
        result: Ok(save.clone()),
    }));
    (b, save)
}

#[test]
fn m28_a_missing_page_raises_the_prompt_and_writes_nothing() {
    let (b, save) = prompt();
    let prompt = b
        .app
        .document_state
        .netlist_incomplete_prompt
        .as_ref()
        .expect("the prompt is up");
    assert!(!prompt.omitted.is_empty(), "it lists the page left out");
    assert!(
        prompt.messages.iter().any(|line| line.contains("usb")),
        "it names the page: {:?}",
        prompt.messages
    );
    assert!(!save.exists());
}

#[test]
fn m28_cancel_writes_no_file() {
    let (mut b, save) = prompt();
    b.send(Message::Export(ExportMsg::NetlistCancelIncomplete));
    assert!(b.app.document_state.netlist_incomplete_prompt.is_none());
    assert!(!save.exists());
}

#[test]
fn m28_export_anyway_writes_the_netlist() {
    let (mut b, save) = prompt();
    b.send(Message::Export(ExportMsg::NetlistExportAnyway));
    assert!(b.app.document_state.netlist_incomplete_prompt.is_none());
    assert!(save.is_file(), "the partial netlist is written");
}

#[test]
fn m28_a_complete_project_exports_without_the_prompt() {
    let mut b = bench(&[("root.snxsch", "")]);
    let save = b.file("out.net");
    let source = b.file("root.snxsch");
    b.send(Message::Export(ExportMsg::NetlistFinished {
        source,
        result: Ok(save.clone()),
    }));
    assert!(b.app.document_state.netlist_incomplete_prompt.is_none());
    assert!(save.is_file());
}
