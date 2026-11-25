//! Mockup row 122b: a symbol's Pin table as a page tab of its symbol,
//! driven through `Signex::update`.

use std::path::PathBuf;

use iced::widget::text_editor;
use signex_library::{PinDirection, Symbol, SymbolFile, SymbolPin};

use super::keep_pin_tables_after_symbols;
use crate::app::document::DocumentKey;
use crate::app::{EditMsg, Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::library::editor::symbol::{PasteMode, PinTableMsg, SymbolPropsMsg};
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

/// An app with `sym` open in a symbol editor tab, active.
fn app_with(sym: Symbol) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from("opamps.snxsym");
    let file = SymbolFile::from_symbol(sym);
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

fn props(app: &mut Signex, msg: SymbolPropsMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SymbolProps(
        msg,
    ))));
}

fn table(app: &mut Signex, msg: PinTableMsg) {
    props(app, SymbolPropsMsg::PinTable(msg));
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn pins_of(app: &Signex, path: &PathBuf) -> Vec<SymbolPin> {
    app.document_state.symbol_editors[path]
        .primitive()
        .pins
        .clone()
}

fn active_is_table(app: &Signex) -> bool {
    let docs = &app.document_state;
    docs.tabs[docs.active_tab].kind.is_pin_table()
}

fn tables(app: &Signex) -> usize {
    let tabs = &app.document_state.tabs;
    tabs.iter().filter(|t| t.kind.is_pin_table()).count()
}

#[test]
fn edit_pins_in_table_opens_one_tab_after_the_symbol_and_focuses_it_again() {
    let (mut app, path) = app_with(opamp());
    let symbol_at = app.document_state.active_tab;
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    assert!(active_is_table(&app));
    assert_eq!(app.document_state.active_tab, symbol_at + 1);
    let tab = &app.document_state.tabs[symbol_at + 1];
    assert_eq!(tab.path, path);
    assert!(tab.title.contains("OPA2333"), "{}", tab.title);

    app.document_state.active_tab = symbol_at;
    app.sync_active_tab();
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    assert_eq!(tables(&app), 1, "one tab per symbol");
    assert!(active_is_table(&app));
}

#[test]
fn the_tab_shows_its_symbol_and_saves_and_undoes_on_it() {
    let (mut app, path) = app_with(opamp());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    let docs = &app.document_state;
    assert_eq!(
        DocumentKey::of(&docs.tabs[docs.active_tab]),
        Some(DocumentKey::Symbol(path.clone()))
    );
    let shown = docs
        .panel_ctx
        .symbol_editor
        .as_ref()
        .expect("the symbol context");
    assert_eq!(shown.pins.len(), 3);

    panel(
        &mut app,
        PanelMsg::SymEditorSetPinName {
            pin_idx: 1,
            value: "\u{2212}INA".into(),
        },
    );
    assert_eq!(pins_of(&app, &path)[1].name, "\u{2212}INA");
    assert!(app.document_state.is_document_dirty(&path));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(pins_of(&app, &path)[1].name, "INA");
    assert!(active_is_table(&app), "undo stays on the tab");
}

#[test]
fn apply_to_selected_edits_the_ticked_pins_as_one_undo_step() {
    let (mut app, path) = app_with(opamp());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut app, PinTableMsg::RowChecked(0, true));
    table(&mut app, PinTableMsg::RowChecked(2, true));
    table(
        &mut app,
        PinTableMsg::BulkElectricalPicked(Some(PinDirection::Input)),
    );
    table(&mut app, PinTableMsg::BulkLengthChanged("5,08".into()));
    table(&mut app, PinTableMsg::BulkHiddenPicked(Some(true)));
    table(&mut app, PinTableMsg::BulkApplied);
    let pins = pins_of(&app, &path);
    for idx in [0, 2] {
        assert_eq!(pins[idx].electrical, PinDirection::Input);
        assert!((pins[idx].length - 5.08).abs() < 1e-9);
        assert!(pins[idx].hidden);
    }
    assert_eq!(pins[1].electrical, PinDirection::Unclassified, "not ticked");
    let ui = &app.library.symbol_props[&path].table;
    assert_eq!(ui.bulk, Default::default(), "the bulk fields are cleared");
    assert_eq!(ui.checked.len(), 2, "the ticks stay");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    let pins = pins_of(&app, &path);
    assert!(
        pins.iter()
            .all(|p| p.electrical == PinDirection::Unclassified && !p.hidden),
        "one undo step takes the whole bulk edit back"
    );
}

#[test]
fn a_refused_bulk_length_keeps_the_lengths() {
    let (mut app, path) = app_with(opamp());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut app, PinTableMsg::RowChecked(0, true));
    table(&mut app, PinTableMsg::BulkLengthChanged("-1".into()));
    table(&mut app, PinTableMsg::BulkApplied);
    assert_eq!(pins_of(&app, &path)[0].length, 2.54);
}

#[test]
fn delete_selected_removes_the_ticked_pins_and_their_ticks() {
    let (mut app, path) = app_with(opamp());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    panel(&mut app, PanelMsg::SymEditorSelectPin(2));
    table(&mut app, PinTableMsg::RowChecked(1, true));
    table(&mut app, PinTableMsg::DeleteSelectedPressed);
    let numbers: Vec<String> = pins_of(&app, &path).into_iter().map(|p| p.number).collect();
    assert_eq!(numbers, ["1", "3"]);
    assert!(app.library.symbol_props[&path].table.checked.is_empty());
    assert!(
        app.document_state.symbol_editors[&path].selected.is_none(),
        "the selected pin index named another pin now"
    );
}

/// A dual op-amp's six pins, two of them outputs.
fn six_pins() -> Symbol {
    let mut sym = Symbol::empty("OPA2333");
    for (i, (number, name)) in [
        ("1", "OUTA"),
        ("2", "INA"),
        ("3", "INB"),
        ("4", "OUTB"),
        ("5", "VCC"),
        ("6", "GND"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut pin = SymbolPin::new(number, name);
        pin.position = [0.0, -2.54 * i as f64];
        sym.pins.push(pin);
    }
    sym
}

#[test]
fn delete_selected_removes_only_the_ticked_rows_the_filter_shows() {
    let (mut app, path) = app_with(six_pins());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    for idx in 0..4 {
        table(&mut app, PinTableMsg::RowChecked(idx, true));
    }
    table(&mut app, PinTableMsg::FilterChanged("out".into()));
    table(&mut app, PinTableMsg::DeleteSelectedPressed);
    let numbers: Vec<String> = pins_of(&app, &path).into_iter().map(|p| p.number).collect();
    assert_eq!(numbers, ["2", "3", "5", "6"], "the hidden ticked rows stay");
    let ticked: Vec<usize> = app.library.symbol_props[&path]
        .table
        .checked
        .iter()
        .copied()
        .collect();
    assert_eq!(
        ticked,
        [0, 1],
        "INA and INB keep their ticks, at their new rows"
    );
}

#[test]
fn apply_to_selected_skips_ticked_rows_the_filter_hides() {
    let (mut app, path) = app_with(six_pins());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut app, PinTableMsg::RowChecked(0, true));
    table(&mut app, PinTableMsg::RowChecked(2, true));
    table(&mut app, PinTableMsg::FilterChanged("outa".into()));
    table(&mut app, PinTableMsg::BulkHiddenPicked(Some(true)));
    table(&mut app, PinTableMsg::BulkApplied);
    let pins = pins_of(&app, &path);
    assert!(pins[0].hidden, "OUTA is listed and ticked");
    assert!(!pins[2].hidden, "INB is ticked but the filter hides it");
}

#[test]
fn plus_pin_adds_a_row_with_the_next_number() {
    let (mut app, path) = app_with(opamp());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut app, PinTableMsg::AddPinPressed);
    let pins = pins_of(&app, &path);
    assert_eq!(pins.len(), 4);
    assert_eq!(pins[3].number, "4");
}

#[test]
fn a_datasheet_paste_fills_an_empty_symbol() {
    let (mut app, path) = app_with(Symbol::empty("OPA2333"));
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    table(&mut app, PinTableMsg::PasteToggled);
    let sheet = "1\tOUTA\tO\tOutput channel A\n2\t\u{2013}INA\tI\tInverting input A";
    table(
        &mut app,
        PinTableMsg::PasteEdited(text_editor::Action::Edit(text_editor::Edit::Paste(
            std::sync::Arc::new(sheet.to_string()),
        ))),
    );
    table(&mut app, PinTableMsg::PasteModePicked(PasteMode::Append));
    table(&mut app, PinTableMsg::PasteApplied);
    let pins = pins_of(&app, &path);
    assert_eq!(pins.len(), 2);
    assert_eq!(pins[0].name, "OUTA");
    assert_eq!(pins[0].electrical, PinDirection::Output);
    assert_eq!(pins[1].name, "\u{2212}INA");
    assert_eq!(pins[1].description, "Inverting input A");
    assert!(
        app.library.symbol_props[&path].table.paste.is_none(),
        "the box closes"
    );
}

#[test]
fn closing_the_tab_keeps_the_symbol_and_closing_the_symbol_closes_the_tab() {
    let (mut app, path) = app_with(opamp());
    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    let page = app.document_state.active_tab;
    let _ = app.close_tab_parking_dirty(page);
    assert_eq!(tables(&app), 0);
    assert!(app.document_state.symbol_editors.contains_key(&path));

    props(&mut app, SymbolPropsMsg::EditPinsInTablePressed);
    let symbol_at = app
        .document_state
        .tabs
        .iter()
        .position(|t| matches!(t.kind, TabKind::SymbolEditor(_)))
        .unwrap();
    let _ = app.close_tab_parking_dirty(symbol_at);
    assert_eq!(tables(&app), 0, "the table goes with its symbol");
}

fn tab(kind: TabKind, path: &str) -> TabInfo {
    TabInfo {
        title: String::new(),
        path: PathBuf::from(path),
        project_id: None,
        kind,
    }
}

#[test]
fn a_table_dragged_before_its_symbol_lands_just_after_it() {
    let mut tabs = vec![
        tab(TabKind::PinTable, "/a.snxsym"),
        tab(TabKind::Schematic, "/s.snxsch"),
        tab(
            TabKind::SymbolEditor(PathBuf::from("/a.snxsym")),
            "/a.snxsym",
        ),
    ];
    let active = keep_pin_tables_after_symbols(&mut tabs, 0);
    assert!(matches!(tabs[1].kind, TabKind::SymbolEditor(_)));
    assert!(tabs[2].kind.is_pin_table());
    assert_eq!(active, 2, "the dragged tab stays active");
}
