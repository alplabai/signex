//! #828 — a typed `nan` / `inf` must not reach geometry, and a refused
//! keystroke must not count as an edit. These drive the real handlers
//! through `Signex::update`, so they fail if a handler loses its guard
//! (a helper-only test would stay green).

use signex_app::app::{
    EditMsg, FootprintEditorState, Message, MoveSelectionMsg, Signex, SymbolEditorState, TabInfo,
    TabKind,
};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::library::editor::footprint::state::{EditorPad, MoveByModal};
use signex_app::library::messages::{
    FootprintEditorMsg, LibraryMessage, PrimitiveEdit, SymbolEditorMsg,
};
use signex_app::panels::{GraphicFieldId, PanelMsg};
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use std::path::{Path, PathBuf};

// ── Move Selection ──────────────────────────────────────────────────

fn move_app() -> Signex {
    let uuid = uuid::Uuid::new_v4();
    let path = PathBuf::from("non-finite-move.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![signex_types::schematic::Symbol {
            uuid,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: Point::new(50.0, 20.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "non-finite-move".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(uuid, SelectedKind::Symbol)];
    app
}

fn position(app: &Signex) -> (f64, f64) {
    let p = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .position;
    (p.x, p.y)
}

fn apply_move(app: &mut Signex, dx: &str, dy: &str) {
    app.ui_state.move_selection.open = true;
    app.ui_state.move_selection.dx = dx.to_string();
    app.ui_state.move_selection.dy = dy.to_string();
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));
}

#[test]
fn a_refused_move_axis_moves_nothing_and_keeps_the_dialog_open() {
    let _ = diagnostics::init_logging();
    for (dx, dy) in [("inf", "5"), ("5", "nan"), ("-inf", "1e999")] {
        let mut app = move_app();
        apply_move(&mut app, dx, dy);

        assert_eq!(
            position(&app),
            (50.0, 20.0),
            "dx={dx:?} dy={dy:?}: a refused axis must not move the other axis either"
        );
        assert!(
            app.ui_state.move_selection.open,
            "dx={dx:?} dy={dy:?}: the dialog stays open so the user can correct it"
        );
    }
}

/// Whether the Messages panel got `needle` after entry `before`. Other
/// tests in this binary log refusals in parallel, so callers pass a value
/// no other test types.
fn logged_since(before: u64, needle: &str) -> bool {
    diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > before && e.message.contains(needle))
}

#[test]
fn a_refused_move_is_logged_to_the_messages_panel_naming_the_axis() {
    let _ = diagnostics::init_logging();
    let mut app = move_app();
    let before = diagnostics::newest_entry_id().unwrap_or(0);
    apply_move(&mut app, "1e400", "5");
    assert!(
        logged_since(before, "Move Selection X refused: \"1e400\""),
        "the refusal must reach the Messages panel and name the axis"
    );
}

#[test]
fn a_finite_move_still_applies_and_closes_the_dialog() {
    let mut app = move_app();
    apply_move(&mut app, "10", "-5");
    assert_eq!(position(&app), (60.0, 15.0));
    assert!(!app.ui_state.move_selection.open);
}

// ── Footprint editor Move Selection By ──────────────────────────────

fn footprint_app(stem: &str) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let file = FootprintFile::from_footprint(Footprint::empty(stem));
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor
        .state
        .pads
        .push(EditorPad::new_default("1".to_string(), (1.0, 2.0)));
    editor.state.selected_pad = Some(0);
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    (app, path)
}

fn confirm_move_by(app: &mut Signex, path: &Path, dx: &str, dy: &str) {
    let editor = app
        .document_state
        .footprint_editors
        .get_mut(path)
        .expect("editor");
    editor.state.move_by_modal = Some(MoveByModal {
        dx_buf: dx.to_string(),
        dy_buf: dy.to_string(),
    });
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByConfirm),
    }));
}

#[test]
fn a_refused_move_by_axis_leaves_the_pad_and_keeps_the_dialog_open() {
    let _ = diagnostics::init_logging();
    for (dx, dy) in [("inf", "0"), ("0", "nan"), ("-inf", "1e999")] {
        let (mut app, path) = footprint_app("non-finite-move-by");
        confirm_move_by(&mut app, &path, dx, dy);

        let editor = &app.document_state.footprint_editors[&path];
        assert_eq!(
            editor.state.pads[0].position_mm,
            (1.0, 2.0),
            "dx={dx:?} dy={dy:?}: a refused axis must not move the pad"
        );
        assert!(
            editor.state.move_by_modal.is_some(),
            "dx={dx:?} dy={dy:?}: the dialog stays open so the user can correct it"
        );
        assert!(!editor.dirty, "dx={dx:?} dy={dy:?}: nothing was edited");
        assert!(
            editor.history.is_empty(),
            "dx={dx:?} dy={dy:?}: a refused move is not an undo step"
        );
    }
}

#[test]
fn a_refused_move_by_is_logged_to_the_messages_panel_naming_the_axis() {
    let _ = diagnostics::init_logging();
    let (mut app, path) = footprint_app("non-finite-move-by-log");
    let before = diagnostics::newest_entry_id().unwrap_or(0);
    confirm_move_by(&mut app, &path, "0", "-1e500");
    assert!(
        logged_since(before, "Move Selection By Y refused: \"-1e500\""),
        "the refusal must reach the Messages panel and name the axis"
    );
}

#[test]
fn a_finite_move_by_still_moves_the_pad_and_closes_the_dialog() {
    let (mut app, path) = footprint_app("finite-move-by");
    confirm_move_by(&mut app, &path, "2.5", "-1");

    let editor = &app.document_state.footprint_editors[&path];
    assert_eq!(editor.state.pads[0].position_mm, (3.5, 1.0));
    assert!(editor.state.move_by_modal.is_none());
    assert!(editor.dirty);
    assert_eq!(editor.history.len(), 1);
}

// ── Symbol editor Properties edits ──────────────────────────────────

fn symbol_app() -> (Signex, PathBuf) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("non-finite.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("A"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolEditorState::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "nf".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddPin { x: 1.0, y: 2.0 }),
    }));
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddRectangle {
            from_x: 0.0,
            from_y: 0.0,
            to_x: 2.0,
            to_y: 2.0,
        }),
    }));
    // The fixture's own setup edits are not the thing under test.
    app.document_state.dirty_paths.clear();
    app.document_state.tabs[0].dirty = false;
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .expect("editor")
        .dirty = false;
    (app, path)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn sym<'a>(app: &'a Signex, path: &Path) -> &'a Symbol {
    app.document_state.symbol_editors[path].primitive()
}

fn is_dirty(app: &Signex) -> bool {
    !app.document_state.dirty_paths.is_empty()
        || app.document_state.tabs[0].dirty
        || app.document_state.symbol_editors.values().any(|e| e.dirty)
}

#[test]
fn a_non_finite_pin_length_leaves_the_pin_untouched() {
    let (mut app, path) = symbol_app();
    let before = sym(&app, &path).pins[0].length;
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        panel(
            &mut app,
            PanelMsg::SymEditorSetPinLength { pin_idx: 0, value },
        );
        assert_eq!(sym(&app, &path).pins[0].length, before, "value {value}");
    }
}

#[test]
fn a_non_finite_pin_x_or_y_leaves_the_pin_untouched() {
    let (mut app, path) = symbol_app();
    let before = sym(&app, &path).pins[0].position;
    for value in [f64::NAN, f64::INFINITY] {
        panel(&mut app, PanelMsg::SymEditorSetPinX { pin_idx: 0, value });
        panel(&mut app, PanelMsg::SymEditorSetPinY { pin_idx: 0, value });
        assert_eq!(sym(&app, &path).pins[0].position, before, "value {value}");
    }
}

#[test]
fn a_non_finite_graphic_field_leaves_the_graphic_untouched() {
    let (mut app, path) = symbol_app();
    let before = sym(&app, &path).graphics[0].clone();
    for field in [GraphicFieldId::FromX, GraphicFieldId::StrokeWidth] {
        for value in [f64::NAN, f64::INFINITY] {
            panel(
                &mut app,
                PanelMsg::SymEditorSetGraphicField {
                    idx: 0,
                    field,
                    value,
                },
            );
            assert_eq!(sym(&app, &path).graphics[0], before, "{field:?} {value}");
        }
    }
}

#[test]
fn a_refused_keystroke_is_not_an_edit() {
    let (mut app, path) = symbol_app();
    panel(
        &mut app,
        PanelMsg::SymEditorSetPinLength {
            pin_idx: 0,
            value: f64::NAN,
        },
    );
    panel(
        &mut app,
        PanelMsg::SymEditorSetPinX {
            pin_idx: 0,
            value: f64::INFINITY,
        },
    );
    assert!(
        !is_dirty(&app),
        "a refused value must not mark the tab dirty"
    );

    // No undo step either: the one Undo reverts the fixture's AddRectangle,
    // not a phantom refused edit.
    let graphics_before = sym(&app, &path).graphics.len();
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(sym(&app, &path).graphics.len(), graphics_before - 1);
}

#[test]
fn re_entering_the_same_value_is_not_an_edit() {
    let (mut app, path) = symbol_app();
    let x = sym(&app, &path).pins[0].position[0];
    panel(
        &mut app,
        PanelMsg::SymEditorSetPinX {
            pin_idx: 0,
            value: x,
        },
    );
    assert!(
        !is_dirty(&app),
        "an unchanged value must not mark the tab dirty"
    );

    let graphics_before = sym(&app, &path).graphics.len();
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(sym(&app, &path).graphics.len(), graphics_before - 1);
}
