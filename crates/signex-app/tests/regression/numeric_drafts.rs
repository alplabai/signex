//! #1352 — a plain numeric text box keeps what the user typed while the box
//! is being edited. `-`, `1.` and an emptied box used to be re-formatted
//! from the stored value on the next frame, so they snapped away before
//! the number could be finished. The draft belongs to its tab, symbol and
//! selected field: it ends on Enter, on a tab or selection switch and on
//! any change to the stored value from outside the box (undo, redo), but a
//! setter's own clamp does not end it. These drive the real handlers
//! through `Signex::update` and read what the Properties panel is handed.

use signex_app::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};
use signex_app::panels::{GraphicFieldId, NumericFieldKey, PanelMsg};
use signex_app::tab_bar::TabMessage;
use signex_library::{Symbol, SymbolFile};
use std::path::{Path, PathBuf};

const X: NumericFieldKey = NumericFieldKey::PinX(0);

fn sym_edit(app: &mut Signex, path: &Path, msg: SymbolEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(msg),
    }));
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

/// Open a symbol tab with one pin at (1, 2) and select that pin.
fn open_symbol(app: &mut Signex, name: &str) -> PathBuf {
    let path = PathBuf::from(format!("numeric-drafts-{name}.snxsym"));
    let file = SymbolFile::from_symbol(Symbol::empty(name));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: name.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    sym_edit(app, &path, SymbolEditorMsg::AddPin { x: 1.0, y: 2.0 });
    panel(app, PanelMsg::SymEditorSelectPin(0));
    path
}

fn symbol_app() -> (Signex, PathBuf) {
    let (mut app, _t) = Signex::new();
    let path = open_symbol(&mut app, "A");
    (app, path)
}

fn pin(app: &Signex, path: &Path) -> signex_library::SymbolPin {
    app.document_state.symbol_editors[path].primitive().pins[0].clone()
}

fn type_into(app: &mut Signex, key: NumericFieldKey, text: &str) {
    panel(app, PanelMsg::NumericFieldTyped(key, text.to_string()));
}

/// What the box for `key` shows, from the context the Properties panel is
/// built from (`None` when that box is not on screen).
fn shown(app: &Signex, key: NumericFieldKey) -> Option<String> {
    let ctx = &app.document_state.panel_ctx;
    let (_, stored) = signex_app::panels::numeric_field_state(
        key,
        None,
        ctx.symbol_editor.as_ref(),
        ctx.pre_placement.as_ref(),
    )?;
    Some(signex_app::panels::numeric_shown_text(
        ctx.numeric_draft.as_ref(),
        key,
        stored,
    ))
}

#[test]
fn typing_one_dot_then_five_yields_one_point_five() {
    let (mut app, path) = symbol_app();
    type_into(&mut app, X, "1");
    type_into(&mut app, X, "1.");
    assert_eq!(shown(&app, X).as_deref(), Some("1."), "the dot survives");
    type_into(&mut app, X, "1.5");
    assert_eq!(pin(&app, &path).position[0], 1.5);
    assert_eq!(shown(&app, X).as_deref(), Some("1.5"));
}

#[test]
fn a_lone_minus_stays_in_the_box_and_the_value_is_untouched() {
    let (mut app, path) = symbol_app();
    type_into(&mut app, X, "3");
    type_into(&mut app, X, "");
    assert_eq!(
        shown(&app, X).as_deref(),
        Some(""),
        "an emptied box stays empty"
    );
    type_into(&mut app, X, "-");
    assert_eq!(shown(&app, X).as_deref(), Some("-"));
    assert_eq!(
        pin(&app, &path).position[0],
        3.0,
        "a half-typed number commits nothing"
    );
    type_into(&mut app, X, "-2");
    assert_eq!(pin(&app, &path).position[0], -2.0);
}

#[test]
fn invalid_text_reverts_to_the_stored_value_on_submit() {
    let (mut app, path) = symbol_app();
    type_into(&mut app, X, "4");
    type_into(&mut app, X, "4x");
    assert_eq!(shown(&app, X).as_deref(), Some("4x"));
    panel(&mut app, PanelMsg::NumericFieldSubmitted(X));
    assert_eq!(shown(&app, X).as_deref(), Some("4.000"));
    assert_eq!(pin(&app, &path).position[0], 4.0);
}

#[test]
fn another_boxs_submit_keeps_this_boxs_draft() {
    let (mut app, _path) = symbol_app();
    type_into(&mut app, X, "-");
    panel(
        &mut app,
        PanelMsg::NumericFieldSubmitted(NumericFieldKey::PinY(0)),
    );
    assert_eq!(shown(&app, X).as_deref(), Some("-"));
}

#[test]
fn typing_half_into_pin_length_stores_it_and_never_shows_the_clamp() {
    // Pin length is `value.max(0.1)`: the `0` keystroke stores 0.1. The box
    // must keep showing what was typed, not snap to `0.100`.
    let (mut app, path) = symbol_app();
    let len = NumericFieldKey::PinLength(0);
    for text in ["0", "0.", "0.5"] {
        type_into(&mut app, len, text);
        assert_eq!(shown(&app, len).as_deref(), Some(text), "after {text:?}");
        // Any panel refresh between keystrokes re-checks the draft against
        // the stored (clamped) value; it must still show what was typed.
        panel(&mut app, PanelMsg::SymEditorSelectPin(0));
        assert_eq!(
            shown(&app, len).as_deref(),
            Some(text),
            "refresh after {text:?}"
        );
    }
    assert_eq!(pin(&app, &path).length, 0.5);
}

#[test]
fn pin_length_clamp_shows_once_the_box_is_submitted() {
    let (mut app, path) = symbol_app();
    let len = NumericFieldKey::PinLength(0);
    type_into(&mut app, len, "0");
    assert_eq!(pin(&app, &path).length, 0.1);
    panel(&mut app, PanelMsg::NumericFieldSubmitted(len));
    assert_eq!(shown(&app, len).as_deref(), Some("0.100"));
}

#[test]
fn arc_start_angle_minus_sixty_ends_at_three_hundred() {
    // Start/End degrees wrap with `rem_euclid(360.0)`: `-6` stores 354,
    // and the box still shows what was typed.
    let (mut app, path) = symbol_app();
    sym_edit(
        &mut app,
        &path,
        SymbolEditorMsg::AddArc {
            cx: 0.0,
            cy: 0.0,
            radius: 2.0,
            start_deg: 10.0,
            end_deg: 90.0,
        },
    );
    panel(&mut app, PanelMsg::SymEditorSelectGraphic(0));
    let start = NumericFieldKey::Graphic {
        idx: 0,
        field: GraphicFieldId::StartDeg,
    };
    for text in ["-", "-6", "-60"] {
        type_into(&mut app, start, text);
        assert_eq!(shown(&app, start).as_deref(), Some(text), "after {text:?}");
    }
    let stored = match &app.document_state.symbol_editors[&path]
        .primitive()
        .graphics[0]
        .kind
    {
        signex_library::SymbolGraphicKind::Arc { start_deg, .. } => *start_deg,
        other => panic!("expected an arc, got {other:?}"),
    };
    assert_eq!(stored, 300.0);
}

#[test]
fn a_draft_does_not_follow_the_user_to_another_symbol_tab() {
    // Both symbols have pin 0 at the same X, so only the owner tells the
    // two boxes apart.
    let (mut app, _a) = symbol_app();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    let _b = open_symbol(&mut app, "B");
    let activate = |app: &mut Signex, tab: usize| {
        let _ = app.update(Message::Tab {
            window_id: window,
            msg: TabMessage::Select(tab),
        });
    };
    activate(&mut app, 0);
    type_into(&mut app, X, "-");
    assert_eq!(shown(&app, X).as_deref(), Some("-"));
    activate(&mut app, 1);
    assert_eq!(
        shown(&app, X).as_deref(),
        Some("1.000"),
        "B's box shows B's stored X, not A's draft"
    );
    activate(&mut app, 0);
    assert_eq!(
        shown(&app, X).as_deref(),
        Some("1.000"),
        "and A's draft is gone"
    );
}

#[test]
fn a_draft_does_not_survive_selecting_another_pin_and_back() {
    let (mut app, path) = symbol_app();
    sym_edit(&mut app, &path, SymbolEditorMsg::AddPin { x: 5.0, y: 6.0 });
    panel(&mut app, PanelMsg::SymEditorSelectPin(0));
    type_into(&mut app, X, "-");
    panel(&mut app, PanelMsg::SymEditorSelectPin(1));
    panel(&mut app, PanelMsg::SymEditorSelectPin(0));
    assert_eq!(shown(&app, X).as_deref(), Some("1.000"));
}

#[test]
fn undo_after_typing_restores_the_box_to_the_stored_value() {
    let (mut app, path) = symbol_app();
    type_into(&mut app, X, "5");
    assert_eq!(pin(&app, &path).position[0], 5.0);
    assert_eq!(shown(&app, X).as_deref(), Some("5"));
    sym_edit(&mut app, &path, SymbolEditorMsg::Undo);
    assert_eq!(pin(&app, &path).position[0], 1.0, "undo restores the old X");
    assert!(
        app.document_state.panel_ctx.numeric_draft.is_none(),
        "the undone edit's draft is dropped"
    );
    panel(&mut app, PanelMsg::SymEditorSelectPin(0));
    assert_eq!(shown(&app, X).as_deref(), Some("1.000"));
}

#[test]
fn non_finite_text_never_reaches_the_pin() {
    // #828: `nan`, `inf` and an overflow are refused, so the stored value is
    // kept; Enter then shows it again.
    let (mut app, path) = symbol_app();
    type_into(&mut app, X, "2");
    for text in ["nan", "inf", "-inf", "1e999"] {
        type_into(&mut app, X, text);
        assert_eq!(pin(&app, &path).position[0], 2.0, "{text} was applied");
    }
    panel(&mut app, PanelMsg::NumericFieldSubmitted(X));
    assert_eq!(shown(&app, X).as_deref(), Some("2.000"));
}
