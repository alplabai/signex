//! #2817 (mockup row 122b): `edit_symbol_pins`, the Pin table tab's bulk
//! edit and Paste rows as a command. Headless on the symbol file `document`
//! names, one undo step that `undo document=..` and the tab's Undo take
//! back; the tab's own edits through `Signex::update` leave the same pins;
//! the tab lays out what the command wrote.

use std::path::{Path, PathBuf};

use iced::Size;
use iced::widget::text_editor;
use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_library::{PinDirection, Symbol, SymbolFile, SymbolPin};

use super::{CommandArgs, CommandOutcome};
use crate::app::{EditMsg, Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::layout_probe::lay_out_inputs;
use crate::library::editor::symbol::{PasteMode, PinTableMsg, SymbolPropsMsg};
use crate::menu_bar::MenuMessage;
use crate::panels::PanelMsg;

fn opamp() -> Symbol {
    let mut sym = Symbol::empty("OPA2333");
    for (i, (number, name)) in [("1", "OUTA"), ("2", "INA"), ("3", "VCC")]
        .into_iter()
        .enumerate()
    {
        let mut pin = SymbolPin::new(number, name);
        pin.position = [0.0, -2.54 * i as f64];
        sym.pins.push(pin);
    }
    sym
}

/// An app with `file` open in a symbol editor tab, active, in `dir`.
fn app_with_file(dir: &Path, file: SymbolFile) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = dir.join("opamps.snxsym");
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "opamps".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.sync_active_tab();
    (app, path)
}

fn app_with(dir: &Path, sym: Symbol) -> (Signex, PathBuf) {
    app_with_file(dir, SymbolFile::from_symbol(sym))
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a command id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn edit(app: &mut Signex, path: &Path, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    run(app, "edit_symbol_pins", &all)
}

fn answered(outcome: CommandOutcome) -> Value {
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

fn pins_of(app: &Signex, path: &Path) -> Vec<SymbolPin> {
    app.document_state.symbol_editors[path]
        .primitive()
        .pins
        .clone()
}

fn table(app: &mut Signex, msg: PinTableMsg) {
    props(app, SymbolPropsMsg::PinTable(msg));
}

fn props(app: &mut Signex, msg: SymbolPropsMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SymbolProps(
        msg,
    ))));
}

/// The bulk edit below as rows; one line, as an operand cannot carry a
/// line break.
const BULK: &str = concat!(
    r#"pins={"1": {"electrical": "input", "length": 5.08, "hidden": true}, "#,
    r#""3": {"electrical": "input", "length": 5.08, "hidden": true}}"#
);

#[test]
fn headless_rows_set_the_pins_as_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path(), opamp());
    let answer = answered(edit(
        &mut app,
        &path,
        &[r#"pins={"2": {"name": "-INA", "electrical": "input"}, "3": {"number": "8"}}"#],
    ));
    assert_eq!(answer["changed"], true);
    assert_eq!(answer["updated"], 2);
    assert_eq!(answer["symbol"], "OPA2333");
    assert_eq!(answer["pins"][1]["name"], "-INA");
    assert_eq!(answer["pins"][1]["electrical"], "input");
    assert_eq!(answer["pins"][2]["number"], "8");
    let pins = pins_of(&app, &path);
    assert_eq!(pins[1].electrical, PinDirection::Input);
    assert_eq!(pins[2].number, "8");
    assert!(app.document_state.is_document_dirty(&path));

    let document = format!("document={}", path.display());
    assert_eq!(run(&mut app, "undo", &[&document]), CommandOutcome::Ran);
    assert_eq!(
        pins_of(&app, &path),
        opamp().pins,
        "one step takes it all back"
    );
    assert_eq!(run(&mut app, "redo", &[&document]), CommandOutcome::Ran);
    assert_eq!(pins_of(&app, &path)[2].number, "8");
}

#[test]
fn the_tabs_bulk_edit_and_the_command_leave_the_same_pins() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_tab, tab_path) = app_with(dir.path(), opamp());
    props(&mut by_tab, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut by_tab, PinTableMsg::RowChecked(0, true));
    table(&mut by_tab, PinTableMsg::RowChecked(2, true));
    table(
        &mut by_tab,
        PinTableMsg::BulkElectricalPicked(Some(PinDirection::Input)),
    );
    table(&mut by_tab, PinTableMsg::BulkLengthChanged("5,08".into()));
    table(&mut by_tab, PinTableMsg::BulkHiddenPicked(Some(true)));
    table(&mut by_tab, PinTableMsg::BulkApplied);

    let other = tempfile::tempdir().expect("tempdir");
    let (mut by_command, command_path) = app_with(other.path(), opamp());
    answered(edit(&mut by_command, &command_path, &[BULK]));
    assert_eq!(
        pins_of(&by_command, &command_path),
        pins_of(&by_tab, &tab_path)
    );

    // Each is one step on the symbol, taken back by the tab's Undo.
    let _ = by_command.update(Message::Edit(EditMsg::Undo));
    assert_eq!(pins_of(&by_command, &command_path), opamp().pins);
}

#[test]
fn the_tabs_paste_and_the_command_add_the_same_pins() {
    let sheet = "1\tOUTA\tO\tOutput channel A\n2\t\u{2013}INA\tI\tInverting input A";
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_tab, tab_path) = app_with(dir.path(), Symbol::empty("OPA2333"));
    props(&mut by_tab, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut by_tab, PinTableMsg::PasteToggled);
    table(
        &mut by_tab,
        PinTableMsg::PasteEdited(text_editor::Action::Edit(text_editor::Edit::Paste(
            std::sync::Arc::new(sheet.to_string()),
        ))),
    );
    table(&mut by_tab, PinTableMsg::PasteModePicked(PasteMode::Append));
    table(&mut by_tab, PinTableMsg::PasteApplied);

    let other = tempfile::tempdir().expect("tempdir");
    let (mut by_command, command_path) = app_with(other.path(), Symbol::empty("OPA2333"));
    let paste = format!(
        r"paste=1\tOUTA\tO\tOutput channel A\n2\t{}INA\tI\tInverting input A",
        '\u{2013}'
    );
    let answer = answered(edit(
        &mut by_command,
        &command_path,
        &[&paste, "mode=append"],
    ));
    assert_eq!(answer["added"], 2);
    assert_eq!(answer["pins"][1]["name"], "\u{2212}INA");
    let pins = pins_of(&by_command, &command_path);
    assert_eq!(pins, pins_of(&by_tab, &tab_path));
    assert_eq!(pins[0].electrical, PinDirection::Output);
}

#[test]
fn a_paste_updates_by_number_through_its_column_map() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path(), opamp());
    let answer = answered(edit(
        &mut app,
        &path,
        &[
            r"paste=3  V+  -  2\n9  NC  -  1",
            "fields=number,name,ignore,part",
        ],
    ));
    assert_eq!(answer["updated"], 1);
    assert_eq!(answer["added"], 1);
    let pins = pins_of(&app, &path);
    assert_eq!(pins.len(), 4);
    assert_eq!(pins[2].name, "V+");
    assert_eq!(pins[2].part_number, 2);
    assert_eq!(pins[2].electrical, PinDirection::Unclassified, "ignored");
    assert_eq!(pins[3].number, "9");
}

#[test]
fn dry_run_answers_with_the_result_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path(), opamp());
    let answer = answered(edit(
        &mut app,
        &path,
        &[r#"pins={"2": null}"#, "dry_run=true"],
    ));
    assert_eq!(answer["dry_run"], true);
    assert_eq!(answer["changed"], true);
    assert_eq!(answer["removed"], 1);
    assert_eq!(answer["pins"].as_array().map(Vec::len), Some(2));
    assert_eq!(pins_of(&app, &path), opamp().pins);
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(
        !app.document_state.symbol_editors[&path]
            .draft
            .history
            .can_undo()
    );
}

#[test]
fn the_same_values_again_change_nothing_and_make_no_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path(), opamp());
    let answer = answered(edit(&mut app, &path, &[r#"pins={"1": {"name": "OUTA"}}"#]));
    assert_eq!(answer["changed"], false);
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(
        !app.document_state.symbol_editors[&path]
            .draft
            .history
            .can_undo()
    );
}

#[test]
fn a_refused_edit_writes_nothing_and_says_why() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path(), opamp());
    for (operands, needle) in [
        (vec![r#"pins={"1": null}"#, r"paste=4\tX"], "not both"),
        (vec![], "needs `pins`"),
        (vec![r#"pins={"1": {"name": "A"}, "7": null}"#], "`7`"),
        (vec![r#"pins={"1": {"length": -1}}"#], "length"),
        (
            vec![r#"pins={"1": null}"#, "mode=append"],
            "go with `paste`",
        ),
        (vec![r"paste=4\tX", "mode=merge"], "update_by_number"),
        (vec![r"paste=4\tX", "fields=number,type"], "`type`"),
        (vec!["paste= "], "no row"),
        (
            vec![r#"pins={"1": null}"#, "symbol=LM358"],
            "no symbol named",
        ),
    ] {
        let why = refusal(edit(&mut app, &path, &operands));
        assert!(why.contains(needle), "{operands:?}: {why}");
    }
    let gone = dir.path().join("gone.snxsym");
    let why = refusal(edit(&mut app, &gone, &[r#"pins={"1": null}"#]));
    assert!(why.contains("not an open"), "{why}");
    assert_eq!(pins_of(&app, &path), opamp().pins);
    assert!(!app.document_state.is_document_dirty(&path));
}

#[test]
fn a_file_of_several_symbols_needs_the_symbol_named() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut file = SymbolFile::from_symbol(opamp());
    let mut second = opamp();
    second.name = "LM358".into();
    file.symbols.push(second);
    let (mut app, path) = app_with_file(dir.path(), file);
    let why = refusal(edit(&mut app, &path, &[r#"pins={"1": null}"#]));
    assert!(why.contains("OPA2333, LM358"), "{why}");

    let answer = answered(edit(
        &mut app,
        &path,
        &[r#"pins={"1": null}"#, "symbol=LM358"],
    ));
    assert_eq!(answer["symbol"], "LM358");
    let file = &app.document_state.symbol_editors[&path].draft.file;
    assert_eq!(
        file.symbols[0].pins.len(),
        3,
        "the shown symbol is untouched"
    );
    assert_eq!(file.symbols[1].pins.len(), 2);
}

#[test]
fn the_tab_shows_the_commands_pins_and_its_ticks_follow_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path(), opamp());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut app, PinTableMsg::RowChecked(2, true));
    answered(edit(
        &mut app,
        &path,
        &[r#"pins={"1": null, "3": {"name": "V+"}}"#],
    ));
    let ticked: Vec<usize> = app.library.symbol_props[&path]
        .table
        .checked
        .iter()
        .copied()
        .collect();
    assert_eq!(ticked, [1], "VCC's tick moved with it");

    let inputs: Vec<String> =
        lay_out_inputs(app.view_pin_table_tab(&path), Size::new(1400.0, 900.0))
            .into_iter()
            .map(|input| input.text)
            .collect();
    assert!(inputs.iter().any(|t| t == "V+"), "{inputs:?}");
    assert!(!inputs.iter().any(|t| t == "OUTA"), "{inputs:?}");
}

#[test]
fn in_a_window_without_operands_it_says_what_it_needs_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path(), opamp());
    let command = AppCommandId::new("edit_symbol_pins").expect("a command id");
    let (outcome, _) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    let reason = super::operand_handlers::needs_operands_reason(&command).expect("a reason");
    assert_eq!(outcome, CommandOutcome::disabled(reason));
    let _ = app.update(Message::Menu(MenuMessage::command("edit_symbol_pins")));
    assert_eq!(pins_of(&app, &path), opamp().pins);
    assert!(!app.document_state.is_document_dirty(&path));
}
