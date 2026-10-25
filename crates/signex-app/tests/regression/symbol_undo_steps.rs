//! #760 follow-up — with whole-container symbol snapshots, every edit that
//! reaches a symbol editor must record exactly one undo step. A missed
//! edit is silently reverted by the next unrelated Undo, and the next
//! recorded edit then clears its redo.

use signex_app::app::{
    EditMsg, LocalColorSlot, Message, Signex, SymbolEditorState, SymbolViewRequest, TabInfo,
    TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};
use signex_app::panels::{GraphicFieldId, PanelMsg};
use signex_library::{PinDirection, Symbol, SymbolFile};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn symbol_app() -> (Signex, PathBuf) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("lib.snxsym");
    let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
    file.symbols.push(Symbol::empty("B"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolEditorState::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "lib".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn canvas(app: &mut Signex, path: &Path, msg: SymbolEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(msg),
    }));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn editor<'a>(app: &'a Signex, path: &Path) -> &'a SymbolEditorState {
    &app.document_state.symbol_editors[path]
}

fn summary(app: &Signex, path: &Path) -> Vec<(String, usize)> {
    editor(app, path)
        .file
        .symbols
        .iter()
        .map(|s| (s.name.clone(), s.pins.len()))
        .collect()
}

#[test]
fn add_symbol_is_its_own_undo_step() {
    let (mut app, path) = symbol_app();
    canvas(&mut app, &path, SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 });
    panel(&mut app, PanelMsg::SchLibraryAddSymbol);
    assert_eq!(summary(&app, &path).len(), 3);

    undo(&mut app);

    assert_eq!(
        summary(&app, &path),
        [("A".into(), 1), ("B".into(), 0)],
        "undo removes the new symbol and keeps A's pin"
    );
}

#[test]
fn symbol_rename_undoes_alone() {
    let (mut app, path) = symbol_app();
    canvas(&mut app, &path, SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 });
    panel(&mut app, PanelMsg::SymEditorSetSymbolName("Renamed".into()));
    assert_eq!(summary(&app, &path)[0], ("Renamed".into(), 1));

    undo(&mut app);

    assert_eq!(
        summary(&app, &path)[0],
        ("A".into(), 1),
        "undo restores the old name and keeps the pin"
    );
}

#[test]
fn typing_in_one_field_is_one_undo_step() {
    let (mut app, path) = symbol_app();
    canvas(&mut app, &path, SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 });
    let original_pin_name = editor(&app, &path).file.symbols[0].pins[0].name.clone();
    for text in ["R", "Re", "Res"] {
        panel(&mut app, PanelMsg::SymEditorSetSymbolName(text.into()));
    }
    for text in ["1", "12"] {
        panel(
            &mut app,
            PanelMsg::SymEditorSetPinName {
                pin_idx: 0,
                value: text.into(),
            },
        );
    }

    undo(&mut app);
    assert_eq!(
        editor(&app, &path).file.symbols[0].pins[0].name,
        original_pin_name,
        "one undo reverts the whole pin-name burst"
    );
    assert_eq!(editor(&app, &path).file.symbols[0].name, "Res");
    undo(&mut app);
    assert_eq!(
        editor(&app, &path).file.symbols[0].name,
        "A",
        "one undo reverts the whole rename burst"
    );
    assert_eq!(editor(&app, &path).file.symbols[0].pins.len(), 1);
}

#[test]
fn every_properties_edit_records_exactly_one_step() {
    let (mut app, path) = symbol_app();
    canvas(&mut app, &path, SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 });
    canvas(
        &mut app,
        &path,
        SymbolEditorMsg::AddRectangle {
            from_x: 0.0,
            from_y: 0.0,
            to_x: 2.0,
            to_y: 2.0,
        },
    );
    let edits: Vec<(&str, PanelMsg)> = vec![
        (
            "pin electrical",
            PanelMsg::SymEditorSetPinElectrical {
                pin_idx: 0,
                value: PinDirection::Output,
            },
        ),
        (
            "pin x",
            PanelMsg::SymEditorSetPinX {
                pin_idx: 0,
                value: 5.08,
            },
        ),
        (
            "pin y",
            PanelMsg::SymEditorSetPinY {
                pin_idx: 0,
                value: 5.08,
            },
        ),
        (
            "pin length",
            PanelMsg::SymEditorSetPinLength {
                pin_idx: 0,
                value: 3.0,
            },
        ),
        (
            "pin number",
            PanelMsg::SymEditorSetPinNumber {
                pin_idx: 0,
                value: "9".into(),
            },
        ),
        (
            "pin description",
            PanelMsg::SymEditorSetPinDescription {
                pin_idx: 0,
                value: "d".into(),
            },
        ),
        ("pin hidden", PanelMsg::SymEditorTogglePinHidden(0)),
        (
            "symbol designator",
            PanelMsg::SymEditorSetSymbolDesignator("U?".into()),
        ),
        ("symbol mirrored", PanelMsg::SymEditorToggleSymbolMirrored),
        (
            "graphic stroke",
            PanelMsg::SymEditorSetGraphicField {
                idx: 0,
                field: GraphicFieldId::StrokeWidth,
                value: 0.3,
            },
        ),
        (
            "graphic fill",
            PanelMsg::SymEditorSetGraphicFill {
                idx: 0,
                color: [1, 2, 3, 255],
            },
        ),
        (
            "local colour",
            PanelMsg::SymEditorSetLocalColor {
                slot: LocalColorSlot::Fill,
                color: [1, 2, 3, 255],
            },
        ),
    ];
    for (label, msg) in edits {
        let before = editor(&app, &path).undo_snapshots.len();
        panel(&mut app, msg);
        assert_eq!(
            editor(&app, &path).undo_snapshots.len(),
            before + 1,
            "{label} must record exactly one undo step"
        );
    }
}

#[test]
fn undo_that_switches_symbol_resets_part_and_viewport() {
    let (mut app, path) = symbol_app();
    {
        let ed = app.document_state.symbol_editors.get_mut(&path).unwrap();
        ed.file.symbols[0].part_count = 3;
        ed.file.symbols[1].part_count = 3;
        let snap = ed.snapshot();
        ed.commit_history(snap);
        ed.active_idx = 1;
        ed.active_part = 3;
        ed.pending_view = None;
        ed.cursor_mm = Some((9.0, 9.0));
    }
    undo(&mut app);

    let ed = editor(&app, &path);
    assert_eq!(ed.active_idx, 0);
    assert_eq!(ed.active_part, 1, "switching symbol starts on unit 1");
    assert_eq!(ed.pending_view, Some(SymbolViewRequest::CenterOrigin));
    assert_eq!(ed.cursor_mm, None);
}

#[test]
fn undo_after_save_keeps_the_quit_gate_armed() {
    let (mut app, path) = symbol_app();
    canvas(&mut app, &path, SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 });
    // What a save does: clear the editor flag and the workspace marker.
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .unwrap()
        .dirty = false;
    app.document_state.dirty_paths.remove(&path);

    undo(&mut app);

    assert!(
        app.document_state.dirty_paths.contains(&path),
        "undo re-dirties the document, so the quit gate must see it"
    );
}

#[test]
fn history_shares_unchanged_symbols_between_snapshots() {
    let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
    file.symbols.push(Symbol::empty("B"));
    file.symbols.push(Symbol::empty("C"));
    let mut ed = SymbolEditorState::new(PathBuf::from("t.snxsym"), file);
    let first = ed.snapshot();
    ed.commit_history(first);
    ed.file.symbols[1].name = "B2".into();
    let second = ed.snapshot();

    let first = &ed.undo_snapshots[0];
    assert!(Arc::ptr_eq(&first.symbols[0], &second.symbols[0]));
    assert!(!Arc::ptr_eq(&first.symbols[1], &second.symbols[1]));
    assert!(Arc::ptr_eq(&first.symbols[2], &second.symbols[2]));
}
