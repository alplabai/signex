//! #1295: Export… writes the listed parts to the picked file; Import… reads
//! a file into the preview, which changes nothing until Apply, and Apply is
//! one undo step.

use std::path::{Path, PathBuf};

use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use super::ParameterIoMsg;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::state::ModalId;
use crate::app::{EditMsg, Message, ParameterManagerMsg, Signex};
use crate::io_ports::{HeadlessAnswers, IoPorts};
use crate::task_driver::TaskDriver;

fn part(n: u128, reference: &str, tolerance: &str) -> Symbol {
    let mut symbol = Symbol {
        uuid: Uuid::from_u128(n),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: "R_0603".to_string(),
        ..Default::default()
    };
    symbol
        .fields
        .insert("Tolerance".to_string(), tolerance.to_string());
    symbol
}

/// An app with one sheet of two resistors in `dir`, dialogs answered from
/// the returned answers.
fn app_in(dir: &Path) -> (Signex, HeadlessAnswers, PathBuf) {
    let answers = HeadlessAnswers::default();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.io = IoPorts::headless(answers.clone());
    let sheet = dir.join("power.snxsch");
    let doc = SchematicSheet {
        uuid: Uuid::now_v7(),
        symbols: vec![part(1, "R1", "1%"), part(2, "R2", "5%")],
        ..Default::default()
    };
    add_sheet_with(&mut app, &sheet, doc);
    (app, answers, sheet)
}

fn send(app: &mut Signex, msg: ParameterIoMsg) {
    TaskDriver::new()
        .send(app, Message::from(msg))
        .expect("settles");
}

fn field(app: &Signex, n: u128, key: &str) -> Option<String> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == Uuid::from_u128(n))
        .and_then(|s| s.fields.get(key).cloned())
}

/// Pick `text`, written to `file` in `dir`, in the Import… dialog.
fn import(app: &mut Signex, answers: &HeadlessAnswers, dir: &Path, text: &str) -> PathBuf {
    let file = dir.join("edited.csv");
    std::fs::write(&file, text).expect("write the csv");
    answers.set("path", file.display().to_string());
    send(app, ParameterIoMsg::ImportRequested);
    file
}

const EDITED: &str = "Reference,Value,Footprint,Tolerance,MPN\r\n\
                      R1,22k,R_0805,2%,RC0603\r\n\
                      R2,10k,R_0603,5%,\r\n\
                      R9,1k,R_0402,1%,X\r\n";

#[test]
fn export_writes_the_listed_parts_to_the_picked_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, _sheet) = app_in(dir.path());
    let file = dir.path().join("out.csv");
    answers.set("path", file.display().to_string());
    send(&mut app, ParameterIoMsg::ExportRequested);
    let written = std::fs::read(&file).expect("the export was written");
    assert_eq!(
        String::from_utf8(written).expect("utf-8"),
        "\u{feff}Unique Id,Reference,Value,Footprint,Tolerance\r\n\
         00000000-0000-0000-0000-000000000001,R1,10k,R_0603,1%\r\n\
         00000000-0000-0000-0000-000000000002,R2,10k,R_0603,5%\r\n"
    );
}

/// The file is keyed by unique id: a line whose Reference cell names
/// another part still writes the part its id names.
#[test]
fn apply_writes_the_part_the_unique_id_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, _sheet) = app_in(dir.path());
    import(
        &mut app,
        &answers,
        dir.path(),
        "Unique Id,Reference,Tolerance\r\n\
         00000000-0000-0000-0000-000000000002,R1,10%\r\n\
         00000000-0000-0000-0000-000000000077,R2,20%\r\n",
    );
    let plan = app.parameter_import_plan().expect("plan");
    assert_eq!(plan.unmatched_lines, 1, "the unknown id matches nothing");
    send(&mut app, ParameterIoMsg::PreviewAccepted);
    assert_eq!(field(&app, 1, "Tolerance").as_deref(), Some("1%"));
    assert_eq!(field(&app, 2, "Tolerance").as_deref(), Some("10%"));
}

#[test]
fn a_cancelled_export_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _answers, _sheet) = app_in(dir.path());
    send(&mut app, ParameterIoMsg::ExportRequested);
    let csv_files = std::fs::read_dir(dir.path())
        .expect("dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "csv"))
        .count();
    assert_eq!(csv_files, 0, "no answer is a cancel");
}

#[test]
fn import_opens_a_preview_that_changes_nothing_until_apply() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, sheet) = app_in(dir.path());
    import(&mut app, &answers, dir.path(), EDITED);

    let state = app.ui_state.parameter_import.as_ref().expect("preview");
    assert_eq!(state.sheet, sheet);
    let plan = app.parameter_import_plan().expect("plan");
    assert_eq!(plan.changes.len(), 2, "R1's Tolerance and MPN");
    assert_eq!(plan.unmatched, vec!["R9".to_string()]);
    assert_eq!(field(&app, 1, "Tolerance").as_deref(), Some("1%"));
    assert_eq!(field(&app, 1, "MPN"), None);
    assert_eq!(
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .symbols[0]
            .value,
        "10k",
        "Value is never imported"
    );
}

#[test]
fn apply_writes_every_change_and_one_undo_takes_it_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, _sheet) = app_in(dir.path());
    import(&mut app, &answers, dir.path(), EDITED);
    send(&mut app, ParameterIoMsg::PreviewAccepted);

    assert!(app.ui_state.parameter_import.is_none(), "Apply closes it");
    assert_eq!(field(&app, 1, "Tolerance").as_deref(), Some("2%"));
    assert_eq!(field(&app, 1, "MPN").as_deref(), Some("RC0603"));
    assert_eq!(field(&app, 2, "MPN"), None, "an empty cell adds nothing");
    assert_eq!(
        app.parameter_session_changes().len(),
        2,
        "the imported cells join the Parameter Manager session"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(field(&app, 1, "Tolerance").as_deref(), Some("1%"));
    assert_eq!(
        field(&app, 1, "MPN"),
        None,
        "one Undo takes the whole import back"
    );
}

#[test]
fn an_import_after_cell_typing_is_its_own_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, sheet) = app_in(dir.path());
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        sheet: sheet.clone(),
        symbol_uuid: Uuid::from_u128(2),
        key: "Tolerance".to_string(),
        value: "10%".to_string(),
    }));
    import(
        &mut app,
        &answers,
        dir.path(),
        "Reference,Tolerance\nR1,2%\nR2,10%\n",
    );
    send(&mut app, ParameterIoMsg::PreviewAccepted);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(field(&app, 1, "Tolerance").as_deref(), Some("1%"));
    assert_eq!(
        field(&app, 2, "Tolerance").as_deref(),
        Some("10%"),
        "the typed cell is an older step"
    );
}

#[test]
fn cancel_closes_the_preview_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, _sheet) = app_in(dir.path());
    import(&mut app, &answers, dir.path(), EDITED);
    assert_eq!(
        app.detached_modal_escape_message(ModalId::ParameterImport)
            .map(|m| format!("{m:?}")),
        Some(format!(
            "{:?}",
            Message::from(ParameterIoMsg::PreviewCancelled)
        )),
        "Esc in the preview window is its Cancel"
    );
    send(&mut app, ParameterIoMsg::PreviewCancelled);
    assert!(app.ui_state.parameter_import.is_none());
    assert_eq!(field(&app, 1, "Tolerance").as_deref(), Some("1%"));
}

#[test]
fn apply_waits_while_another_sheet_is_active() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, _sheet) = app_in(dir.path());
    import(&mut app, &answers, dir.path(), EDITED);
    let other = dir.path().join("other.snxsch");
    add_sheet_with(&mut app, &other, SchematicSheet::default());
    send(&mut app, ParameterIoMsg::PreviewAccepted);
    assert!(
        app.ui_state.parameter_import.is_some(),
        "the preview stays open"
    );
    let first = app
        .document_state
        .engines
        .get(&dir.path().join("power.snxsch"))
        .expect("first sheet");
    assert_eq!(
        first.document().symbols[0]
            .fields
            .get("Tolerance")
            .map(String::as_str),
        Some("1%")
    );
}

#[test]
fn a_file_with_no_reference_column_opens_no_preview() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, _sheet) = app_in(dir.path());
    import(&mut app, &answers, dir.path(), "Value,MPN\n10k,x\n");
    assert!(app.ui_state.parameter_import.is_none());
}

#[test]
fn an_export_imports_back_to_no_change() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, answers, _sheet) = app_in(dir.path());
    let bytes = app
        .active_parameter_csv()
        .expect("a sheet")
        .expect("export");
    let text = String::from_utf8(bytes).expect("utf-8");
    import(&mut app, &answers, dir.path(), &text);
    let plan = app.parameter_import_plan().expect("plan");
    assert!(plan.changes.is_empty() && plan.unmatched.is_empty());
    assert_eq!(plan.unmatched_lines, 0);
}
