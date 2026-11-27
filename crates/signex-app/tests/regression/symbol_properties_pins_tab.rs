//! Mockup rows 122 and 123 — the symbol editor's Pins dock panel became the
//! Pins tab of Properties. These drive the real handlers through `Signex::update`: the
//! tab and filter are panel state of the editor, a row press selects the
//! pin (Properties then shows Pin properties), `‹ Pins` clears the selection
//! and shows the list again, `+ Pin` arms the Add Pin tool, and the Parts
//! stepper adds a part through the toolbar's path.

use signex_app::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::editor::symbol::{
    BodyScope, PartChip, PartStep, SymbolPropsMsg, SymbolPropsTab, SymbolSelection,
};
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};
use signex_app::panels::{PanelMsg, SymbolEditorSelection};
use signex_library::{Symbol, SymbolFile};
use std::path::PathBuf;

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn props(app: &mut Signex, msg: SymbolPropsMsg) {
    panel(app, PanelMsg::SymbolProps(msg));
}

/// A symbol tab with two pins and nothing selected.
fn symbol_app() -> (Signex, PathBuf) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("pins-tab.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("PINS"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "PINS".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    for y in [0.0, 2.54] {
        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddPin { x: 0.0, y }),
        }));
    }
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .unwrap()
        .gui
        .selected = None;
    props(&mut app, SymbolPropsMsg::TabPicked(SymbolPropsTab::Symbol));
    (app, path)
}

fn shown(app: &Signex) -> &signex_app::panels::SymbolEditorPanelContext {
    app.document_view
        .panel_ctx
        .symbol_editor
        .as_ref()
        .expect("a symbol tab is active")
}

#[test]
fn the_tab_filter_and_chip_reach_the_panel() {
    let (mut app, _path) = symbol_app();
    props(&mut app, SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins));
    props(&mut app, SymbolPropsMsg::PinFilterChanged("2".into()));
    props(&mut app, SymbolPropsMsg::PartChipPicked(PartChip::Part(1)));
    let ui = &shown(&app).props;
    assert_eq!(ui.tab, SymbolPropsTab::Pins);
    assert_eq!(ui.pin_filter, "2");
    assert_eq!(ui.part_chip, PartChip::Part(1));
}

#[test]
fn a_pins_tab_row_selects_its_pin_and_back_returns_to_the_list() {
    let (mut app, path) = symbol_app();
    props(&mut app, SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins));
    panel(&mut app, PanelMsg::SymEditorSelectPin(1));
    assert!(
        app.document_state.symbol_editors[&path]
            .gui
            .selected
            .is_some()
    );
    assert!(matches!(
        shown(&app).selected,
        SymbolEditorSelection::Pin(ref p) if p.idx == 1
    ));

    props(&mut app, SymbolPropsMsg::PinsBackPressed);
    assert!(
        app.document_state.symbol_editors[&path]
            .gui
            .selected
            .is_none()
    );
    assert_eq!(shown(&app).selected, SymbolEditorSelection::None);
    assert_eq!(shown(&app).props.tab, SymbolPropsTab::Pins);
}

#[test]
fn back_from_a_canvas_pick_shows_the_pins_tab() {
    let (mut app, _path) = symbol_app();
    panel(&mut app, PanelMsg::SymEditorSelectPin(0));
    props(&mut app, SymbolPropsMsg::PinsBackPressed);
    assert_eq!(shown(&app).props.tab, SymbolPropsTab::Pins);
}

#[test]
fn add_pin_arms_the_add_pin_tool() {
    let (mut app, path) = symbol_app();
    props(&mut app, SymbolPropsMsg::AddPinPressed);
    // Read through Debug so the test names no editor-slice internals.
    assert_eq!(
        format!("{:?}", app.document_state.symbol_editors[&path].gui.tool),
        "AddPin"
    );
}

#[test]
fn the_parts_stepper_adds_a_part_and_steps_back() {
    let (mut app, path) = symbol_app();
    assert_eq!(
        app.document_state.symbol_editors[&path].draft.active_part,
        1
    );
    props(&mut app, SymbolPropsMsg::PartStepPressed(PartStep::Added));
    assert_eq!(
        app.document_state.symbol_editors[&path].draft.active_part,
        2
    );
    props(
        &mut app,
        SymbolPropsMsg::PartStepPressed(PartStep::Previous),
    );
    assert_eq!(
        app.document_state.symbol_editors[&path].draft.active_part,
        1
    );
    assert_eq!(shown(&app).active_part, 1);
}

/// Mockup row 123: Applies to retags the selection as one undo step, and
/// the row follows.
#[test]
fn applies_to_retags_the_selection_as_one_undo_step() {
    let (mut app, path) = symbol_app();
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    editor.gui.selected = Some(SymbolSelection::All);
    let before = editor.draft.history.undo_len();
    props(&mut app, SymbolPropsMsg::BodyScopePicked(BodyScope::Both));
    let editor = &app.document_state.symbol_editors[&path];
    assert!(
        editor
            .primitive()
            .pins
            .iter()
            .all(|p| p.body_style == signex_library::BODY_COMMON)
    );
    assert_eq!(editor.draft.history.undo_len(), before + 1);
    assert_eq!(shown(&app).body_scope.scope, Some(BodyScope::Both));
}

#[test]
fn the_panel_state_belongs_to_its_editor() {
    let (mut app, _path) = symbol_app();
    props(&mut app, SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins));
    let other = PathBuf::from("other.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("OTHER"));
    app.document_state
        .symbol_editors
        .insert(other.clone(), SymbolDocument::new(other.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "OTHER".into(),
        path: other.clone(),
        project_id: None,
        kind: TabKind::SymbolEditor(other),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    props(&mut app, SymbolPropsMsg::PinFilterChanged(String::new()));
    assert_eq!(shown(&app).props.tab, SymbolPropsTab::Symbol);
}
