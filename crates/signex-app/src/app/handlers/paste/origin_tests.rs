//! Mockup row 155: Paste Special opens the Smart Paste dialog only for a person
//! at a window; an agent's Paste Special, even in a windowed app, pastes at
//! once, since nobody would answer the dialog.

use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};

use crate::app::agent::Origin;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, doc};
use crate::app::{EditMsg, Message, PasteIntent, Signex, Tool};
use crate::paste_prefs::PasteDesignators;

/// A windowed app whose sheet has one wire, copied.
fn app_with_a_copied_wire() -> Signex {
    let mut app = app_with_tool(Tool::Select);
    let path = app.document_state.active_path.clone().expect("active");
    let id = uuid::Uuid::from_u128(5);
    let mut sheet = doc(&app).clone();
    sheet.wires.push(Wire {
        uuid: id,
        start: Point::new(0.0, 0.0),
        end: Point::new(10.16, 0.0),
        ..Default::default()
    });
    app.document_state
        .engines
        .insert(path, signex_engine::Engine::new(sheet).expect("engine"));
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Wire)];
    let _ = app.update(Message::Edit(EditMsg::Copy));
    // Keep reads no project numbers, so the clipboard answer comes next.
    app.ui_state.paste_designators = PasteDesignators::Keep;
    assert!(app.headless_answers().is_none(), "a windowed app");
    assert!(
        app.interaction_state.clipboard.is_some(),
        "the wire is copied"
    );
    app
}

/// The system clipboard's answer to a Paste Special made under `origin`.
fn paste_special_as(app: &mut Signex, origin: Origin) {
    let request = app.with_origin(origin, |app| app.new_paste_request(PasteIntent::SmartPaste));
    let _ = app.update(Message::Edit(EditMsg::PasteTextRead {
        request,
        text: None,
    }));
}

#[test]
fn an_agents_paste_special_in_a_window_pastes_at_once() {
    let mut app = app_with_a_copied_wire();
    paste_special_as(&mut app, Origin::agent(Some(42)));
    assert!(
        app.ui_state.smart_paste.is_none(),
        "no dialog waits for a person"
    );
    assert_eq!(doc(&app).wires.len(), 2, "the copy is pasted");
}

#[test]
fn a_persons_paste_special_in_a_window_opens_the_dialog() {
    let mut app = app_with_a_copied_wire();
    paste_special_as(&mut app, Origin::Person);
    assert!(app.ui_state.smart_paste.is_some(), "the dialog opens");
    assert_eq!(doc(&app).wires.len(), 1, "nothing is pasted yet");
}
