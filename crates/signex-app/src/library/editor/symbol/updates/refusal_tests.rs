//! #3129: pin text a save could not store is refused where it is entered,
//! and a file that opened repaired takes no edit until it is saved.

use std::path::PathBuf;

use signex_library::{Symbol, SymbolFile, SymbolPin, UnstorablePinText};

use super::*;
use crate::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::PanelMsg;

fn editor_with_pin() -> SymEditor {
    let mut symbol = Symbol::empty("OPAMP");
    symbol.pins.push(SymbolPin::new("7", "OUT"));
    SymEditor::new(PathBuf::from("t.snxsym"), SymbolFile::from_symbol(symbol))
}

fn status(editor: &SymEditor) -> String {
    editor
        .gui
        .status_message
        .clone()
        .unwrap_or_default()
        .replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn a_canvas_pin_rename_with_cr_nul_del_or_a_mix_is_refused_and_says_why() {
    for (name, character, code) in [
        ("A\rB", '\r', "U+000D"),
        ("A\0B", '\0', "U+0000"),
        ("A\u{7f}B", '\u{7f}', "U+007F"),
        ("A\u{1}B\rC", '\u{1}', "U+0001"),
    ] {
        let mut editor = editor_with_pin();
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::SetPinName {
                idx: 0,
                name: name.into(),
            },
        );
        assert_eq!(editor.primitive().pins[0].name, "OUT", "{name:?} refused");
        assert!(!editor.is_dirty(), "{name:?}: no edit");
        assert!(!editor.undo(), "{name:?}: no undo step");
        assert_eq!(
            editor.gui.pin_text_refusal,
            Some(UnstorablePinText {
                pin_index: 0,
                column: "name",
                character,
            })
        );
        let line = status(&editor);
        for part in ["name", "pin 7", code] {
            assert!(line.contains(part), "{part} missing from {line}");
        }
    }
}

#[test]
fn a_clean_rename_after_a_refusal_applies_and_clears_it() {
    let mut editor = editor_with_pin();
    let rename = |name: &str| SymbolEditorMsg::SetPinNumber {
        idx: 0,
        number: name.into(),
    };
    apply_symbol_primitive_edit(&mut editor, rename("8\r"));
    assert!(editor.gui.pin_text_refusal.is_some());
    apply_symbol_primitive_edit(&mut editor, rename("8"));
    assert_eq!(editor.primitive().pins[0].number, "8");
    assert!(editor.is_dirty());
    assert_eq!(editor.gui.pin_text_refusal, None);
}

#[test]
fn a_repaired_draft_refuses_edits_but_still_pans_until_saved() {
    let mut editor = editor_with_pin();
    editor.draft.open_repaired(SymbolDocument::HISTORY_DEPTH);
    assert!(editor.is_dirty(), "the repair is not on disk");
    let camera = editor.gui.camera.offset;
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 });
    assert_eq!(editor.primitive().pins.len(), 1, "no pin added");
    assert_eq!(editor.gui.selected, None);
    assert!(status(&editor).contains("Save it"), "{}", status(&editor));
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Pan { dx: 5.0, dy: 0.0 });
    assert_ne!(editor.gui.camera.offset, camera, "the view still moves");

    // A save taking its snapshot is not yet the repair on disk.
    editor.mark_saved();
    assert!(
        editor.draft.is_read_only(),
        "read-only until the write lands"
    );
    editor.draft.repair_written();
    apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 });
    assert_eq!(editor.primitive().pins.len(), 2, "editable once saved");
}

fn app_with_symbol(editor: SymEditor) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = editor.path.clone();
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "OPAMP".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, path)
}

fn set_pin_name(app: &mut Signex, value: &str) {
    let msg = PanelMsg::SymEditorSetPinName {
        pin_idx: 0,
        value: value.into(),
    };
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

#[test]
fn a_pasted_pin_name_with_cr_is_refused_in_the_properties_field() {
    let (mut app, path) = app_with_symbol(editor_with_pin());
    set_pin_name(&mut app, "OUT\r");
    let editor = app.document_state.symbol_editors.get(&path).unwrap();
    assert_eq!(editor.primitive().pins[0].name, "OUT");
    assert!(!editor.is_dirty());
    let shown = app
        .document_view
        .panel_ctx
        .symbol_editor
        .as_ref()
        .and_then(|ctx| ctx.pin_text_refusal.clone());
    assert_eq!(
        shown.map(|r| (r.pin_index, r.column, r.character)),
        Some((0, "name", '\r')),
        "the panel shows the refusal under the field"
    );
    set_pin_name(&mut app, "OUTA");
    let editor = app.document_state.symbol_editors.get(&path).unwrap();
    assert_eq!(editor.primitive().pins[0].name, "OUTA");
    assert_eq!(editor.gui.pin_text_refusal, None);
}

#[test]
fn a_repaired_draft_refuses_properties_edits() {
    let mut editor = editor_with_pin();
    editor.draft.open_repaired(SymbolDocument::HISTORY_DEPTH);
    let (mut app, path) = app_with_symbol(editor);
    set_pin_name(&mut app, "IN");
    let editor = app.document_state.symbol_editors.get(&path).unwrap();
    assert_eq!(editor.primitive().pins[0].name, "OUT");
    assert!(status(editor).contains("Save it"), "{}", status(editor));
}
