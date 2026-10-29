//! #368 — the keyboard is gated on the same enablement predicate as the menu.
//!
//! The menu used to grey Undo on the schematic engine's history alone, while
//! Ctrl+Z also undoes in a footprint editor tab from its own history. Gating
//! the keyboard on the old menu rule would have broken that shortcut.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind, UiMsg};
use signex_app::keymap::ShortcutTrigger;
use signex_library::{Footprint, FootprintFile};
use std::path::PathBuf;

fn press(app: &mut Signex, keys: &str) {
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("{keys} is a key, not a gesture");
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: strokes.remove(0),
    }));
}

#[test]
fn ctrl_z_still_undoes_in_a_footprint_editor_tab() {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("command-enablement.snxfpt");
    let file = FootprintFile::from_footprint(Footprint::empty("f"));
    let mut editor = FootprintDocument::new(path.clone(), file);
    editor.push_history();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "f".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;

    press(&mut app, "Ctrl+Z");

    assert!(
        app.document_state.footprint_editors[&path]
            .history
            .is_empty(),
        "Ctrl+Z must still reach the footprint editor's own history"
    );
}

/// #907 review: the change itself, end to end. Ctrl+W (Place Wire in the
/// default profile) with no document open used to arm the wire tool while
/// Place ▸ Wire was greyed; it now does nothing.
#[test]
fn place_wire_with_no_document_open_does_nothing() {
    let (mut app, _t) = Signex::new();

    press(&mut app, "Ctrl+W");

    assert_eq!(
        app.interaction_state.current_tool,
        signex_app::app::Tool::Select,
        "no schematic: the wire tool must not arm"
    );
}
