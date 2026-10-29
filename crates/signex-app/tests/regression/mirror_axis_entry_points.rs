//! #1404 — the X / Y keys, the context-menu Mirror X / Y and the active
//! bar's Flip Selected Sheet Symbols Along X / Y are three entry points to
//! one operation, so each pair must flip a placed symbol the same way.
//! The active bar once crossed the axes the other two agree on.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{ContextAction, ContextMenuMsg, Message, Signex, TabInfo, TabKind, UiMsg};
use signex_app::keymap::{KeyStroke, KeyToken, Modifiers};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::path::PathBuf;

/// A fresh app with one unmirrored symbol, selected, on the Altium keymap.
fn app_with_selected_symbol() -> Signex {
    let symbol = Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(10.0, 10.0),
        ..Default::default()
    };
    let selected = vec![SelectedItem::new(symbol.uuid, SelectedKind::Symbol)];
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![symbol],
        ..Default::default()
    };
    let path = PathBuf::from("mirror-axis-entry-points.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.grid_size_mm = 1.0;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "mirror-axis-entry-points".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected = selected;
    app
}

/// The symbol's saved `(mirror_x, mirror_y)` after `message` ran on a
/// fresh app.
fn mirror_after(message: Message) -> (bool, bool) {
    let mut app = app_with_selected_symbol();
    let _ = app.update(message);
    let sheet = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    (sheet.symbols[0].mirror_x, sheet.symbols[0].mirror_y)
}

fn key(letter: &str) -> Message {
    Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: KeyStroke {
            modifiers: Modifiers::default(),
            key: KeyToken::Character(letter.into()),
        },
    })
}

fn context(action: ContextAction) -> Message {
    Message::ContextMenu(ContextMenuMsg::Action(action))
}

fn active_bar(action: ActiveBarAction) -> Message {
    Message::ActiveBar(ActiveBarMsg::Action(action))
}

#[test]
fn x_flips_agree_across_key_context_menu_and_active_bar() {
    let by_key = mirror_after(key("x"));
    assert_ne!(by_key, (false, false), "the X key must flip the symbol");
    assert_eq!(mirror_after(context(ContextAction::MirrorX)), by_key);
    assert_eq!(
        mirror_after(active_bar(ActiveBarAction::FlipSelectedX)),
        by_key
    );
}

#[test]
fn y_flips_agree_across_key_context_menu_and_active_bar() {
    let by_key = mirror_after(key("y"));
    assert_ne!(by_key, (false, false), "the Y key must flip the symbol");
    assert_eq!(mirror_after(context(ContextAction::MirrorY)), by_key);
    assert_eq!(
        mirror_after(active_bar(ActiveBarAction::FlipSelectedY)),
        by_key
    );
}

#[test]
fn x_and_y_flip_different_axes() {
    assert_ne!(mirror_after(key("x")), mirror_after(key("y")));
}
