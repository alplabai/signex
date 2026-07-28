//! Pad selection, clipboard, rotate/flip, courtyard recompute, and context-menu dispatch.

use signex_app::app::{Message, Signex};

use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────
// v0.26-E — pad clipboard (Cut / Copy / Paste)
//
// Drives `apply_footprint_clipboard_op` via the public Message
// surface so the split-borrow at dispatch/library.rs:3499 is
// exercised end-to-end (DocumentState.pad_clipboard + the path-keyed
// FootprintEditorState mutated together).
// ─────────────────────────────────────────────────────────────────

/// Helper — fresh standalone footprint editor with N pads parked at
/// `path` inside `document_state.footprint_editors`. Returns the app
/// and the path so the caller can dispatch and re-borrow.
fn fixture_footprint_with_pads(stem: &str, count: usize) -> (Signex, PathBuf) {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::{Footprint, FootprintFile};
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let fp = Footprint::empty(stem);
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    for i in 0..count {
        editor.state.pads.push(EditorPad::new_default(
            (i + 1).to_string(),
            (i as f64 * 2.0, 0.0),
        ));
    }
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    (app, path)
}

#[test]
fn v026e_copy_with_no_selection_is_noop() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026e-copy-empty", 1);
    // No pad selected.
    assert!(
        app.document_state
            .footprint_editors
            .get(&path)
            .unwrap()
            .state
            .selected_pad
            .is_none()
    );
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CopyPad),
    }));
    assert!(
        app.document_state.pad_clipboard.is_none(),
        "Copy with no selection must leave clipboard untouched"
    );
}

#[test]
fn v026e_copy_populates_clipboard_with_selected_pad() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026e-copy", 2);
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .selected_pad = Some(1);
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CopyPad),
    }));
    let clip = app
        .document_state
        .pad_clipboard
        .as_ref()
        .expect("Copy must populate the clipboard");
    assert_eq!(clip.number, "2", "clipboard holds the selected pad");
    // Source pad is still on the canvas — Copy doesn't mutate.
    assert_eq!(
        app.document_state
            .footprint_editors
            .get(&path)
            .unwrap()
            .state
            .pads
            .len(),
        2,
        "Copy must not delete the source pad"
    );
}

#[test]
fn v026e_cut_removes_pad_populates_clipboard_and_pushes_history() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026e-cut", 2);
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .selected_pad = Some(0);
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CutPad),
    }));
    // Clipboard now holds the cut pad.
    let clip = app
        .document_state
        .pad_clipboard
        .as_ref()
        .expect("Cut must populate the clipboard");
    assert_eq!(clip.number, "1");
    // Pad list shrunk and history grew so Ctrl+Z restores it.
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.pads.len(),
        1,
        "Cut must remove the pad from the canvas"
    );
    assert_eq!(
        editor.history.len(),
        1,
        "Cut must snapshot history so undo can restore the pad"
    );
    assert!(editor.dirty, "Cut marks the editor dirty");
}

#[test]
fn v026e_paste_at_cursor_with_bumped_designator() {
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026e-paste-cursor", 2);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.cursor_mm = Some((10.0, 5.0));
    }
    // Stash a pad in the clipboard directly — Paste reads it, no
    // need to drive Copy first.
    app.document_state.pad_clipboard = Some(EditorPad::new_default("99".into(), (0.0, 0.0)));
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PastePad),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.pads.len(),
        3,
        "Paste must append a new pad to the canvas"
    );
    let pasted = editor.state.pads.last().unwrap();
    // designator = max-existing + 1 (existing 1 + 2 → 3), NOT the
    // template's "99".
    assert_eq!(
        pasted.number, "3",
        "Paste designator must be max-existing + 1"
    );
    assert_eq!(
        pasted.position_mm,
        (10.0, 5.0),
        "Paste must place the pad at the cursor"
    );
    assert_eq!(
        editor.state.selected_pad,
        Some(2),
        "Pasted pad must be selected"
    );
}

#[test]
fn v026e_paste_resets_sketch_entity_links() {
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_sketch::id::SketchEntityId;
    let (mut app, path) = fixture_footprint_with_pads("v026e-paste-fresh-ids", 1);
    // Clipboard holds a pad with sketch links populated — the paste
    // path must reset both fields so the new pad re-mirrors freshly.
    let mut template = EditorPad::new_default("1".into(), (0.0, 0.0));
    template.sketch_entity_id = Some(SketchEntityId::new());
    template.corner_entity_ids = Some([
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    ]);
    app.document_state.pad_clipboard = Some(template);
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PastePad),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pasted = editor.state.pads.last().unwrap();
    assert!(
        pasted.sketch_entity_id.is_none(),
        "Paste must clear sketch_entity_id so the new pad re-mirrors"
    );
    assert!(
        pasted.corner_entity_ids.is_none(),
        "Paste must clear corner_entity_ids so the new pad re-mirrors"
    );
}

#[test]
fn v026e_paste_with_empty_clipboard_is_noop() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026e-paste-empty", 1);
    assert!(app.document_state.pad_clipboard.is_none());
    let pad_count_before = app
        .document_state
        .footprint_editors
        .get(&path)
        .unwrap()
        .state
        .pads
        .len();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PastePad),
    }));
    assert_eq!(
        app.document_state
            .footprint_editors
            .get(&path)
            .unwrap()
            .state
            .pads
            .len(),
        pad_count_before,
        "Paste with empty clipboard must not mint a pad"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.26-G — Pad Actions submenu (Rotate 90° / Flip Layer)
//
// The submenu items route through the active-bar handlers that
// Space / X also bind to, so these tests cover both gesture paths.
// ─────────────────────────────────────────────────────────────────

#[test]
fn v026g_rotate_selection_increments_rotation_by_90_degrees() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026g-rotate", 1);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.pads[0].rotation_deg = 0.0;
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarRotateSelection),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.pads[0].rotation_deg, 90.0,
        "Rotate must increment rotation_deg by 90"
    );
    assert!(editor.dirty, "Rotate marks the editor dirty");
}

#[test]
fn v026g_rotate_selection_wraps_at_360() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026g-rotate-wrap", 1);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.pads[0].rotation_deg = 270.0;
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarRotateSelection),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.pads[0].rotation_deg, 0.0,
        "Rotate from 270° must wrap to 0° via rem_euclid(360)"
    );
}

#[test]
fn v026g_flip_selection_swaps_top_to_bottom_layers() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::LayerId;
    let (mut app, path) = fixture_footprint_with_pads("v026g-flip", 1);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        // Default new_default layers: F.Cu / F.Mask / F.Paste.
        // Sanity-check before the flip.
        let before: Vec<&str> = editor.state.pads[0]
            .layers
            .iter()
            .map(|l| l.as_str())
            .collect();
        assert_eq!(before, vec!["F.Cu", "F.Mask", "F.Paste"]);
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarFlipSelection),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let after: Vec<String> = editor.state.pads[0]
        .layers
        .iter()
        .map(|l| l.as_str().to_string())
        .collect();
    assert_eq!(
        after,
        vec![
            "B.Cu".to_string(),
            "B.Mask".to_string(),
            "B.Paste".to_string()
        ],
        "Flip must swap the F. prefix to B. on every layer"
    );
    // Flipping again must round-trip back to the original.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarFlipSelection),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let round_trip: Vec<&str> = editor.state.pads[0]
        .layers
        .iter()
        .map(|l| l.as_str())
        .collect();
    assert_eq!(
        round_trip,
        vec!["F.Cu", "F.Mask", "F.Paste"],
        "Flip is its own inverse"
    );
    // Avoid an unused-import lint when LayerId isn't otherwise touched.
    let _: LayerId = LayerId::new("F.Cu");
}

#[test]
fn v026g_rotate_with_no_selection_is_noop() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026g-rotate-noop", 1);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = None;
        editor.state.pads[0].rotation_deg = 45.0;
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarRotateSelection),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.pads[0].rotation_deg, 45.0,
        "Rotate with no selection must not touch any pad"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.26-C — Fit-to-Window one-shot signal
//
// `FootprintContextMenuAction(FitToWindow)` arms a flag the canvas
// Program consumes on its next event tick; `FootprintFitConsumed`
// clears it so it can't re-trigger.
// ─────────────────────────────────────────────────────────────────

#[test]
fn v026c_fit_to_window_action_arms_fit_pending_and_closes_menu() {
    use signex_app::library::editor::footprint::state::{
        FootprintContextAction, FootprintContextMenuState, FootprintContextTarget,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026c-fit-arm", 1);
    // Open a context menu so the action's "close menu" side effect
    // has something visible to clear.
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.context_menu = Some(FootprintContextMenuState {
            x: 0.0,
            y: 0.0,
            target: FootprintContextTarget::Empty,
            submenu: None,
        });
        assert!(!editor.state.fit_pending, "fresh editor: fit_pending=false");
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ContextMenuAction(
            FootprintContextAction::FitToWindow,
        )),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert!(
        editor.state.fit_pending,
        "FitToWindow action must arm fit_pending for the canvas to consume"
    );
    assert!(
        editor.state.context_menu.is_none(),
        "FitToWindow action must close the context menu"
    );
}

#[test]
fn v026c_fit_consumed_clears_fit_pending() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026c-fit-consume", 1);
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .fit_pending = true;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::FitConsumed),
    }));
    assert!(
        !app.document_state
            .footprint_editors
            .get(&path)
            .unwrap()
            .state
            .fit_pending,
        "FitConsumed must clear fit_pending so the next event tick doesn't re-fit"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.26-I — auto-courtyard removed (default flipped to false)
//
// Courtyards are now authored shapes; recompute_courtyard early-
// returns when the toggle is off so existing call sites no-op
// without per-site touches.
// ─────────────────────────────────────────────────────────────────

#[test]
fn v026i_auto_fit_courtyard_default_is_false_after_from_footprint() {
    let (app, path) = fixture_footprint_with_pads("v026i-default", 0);
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert!(
        !editor.state.auto_fit_courtyard,
        "v0.26-I flipped the default to false (courtyards are authored, not auto-derived)"
    );
}

#[test]
fn v026i_recompute_courtyard_with_auto_fit_off_does_not_overwrite_courtyard() {
    let (mut app, path) = fixture_footprint_with_pads("v026i-noop", 2);
    let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
    editor.state.auto_fit_courtyard = false;
    editor.state.courtyard_mm = None;
    editor.state.recompute_courtyard();
    assert!(
        editor.state.courtyard_mm.is_none(),
        "recompute_courtyard must early-return when auto_fit_courtyard is off"
    );
}

#[test]
fn v026i_recompute_courtyard_with_auto_fit_on_still_computes_pad_bbox() {
    let (mut app, path) = fixture_footprint_with_pads("v026i-on", 2);
    let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
    // Pads are placed at (0, 0) and (2, 0) by the fixture; explicit
    // opt-in must still compute a non-empty bbox.
    editor.state.auto_fit_courtyard = true;
    editor.state.courtyard_mm = None;
    editor.state.recompute_courtyard();
    assert!(
        editor.state.courtyard_mm.is_some(),
        "with auto_fit_courtyard explicitly on, recompute must populate courtyard_mm"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.26-B / v0.26-D — right-click context menu target dispatch
//
// FootprintShowContextMenu opens the menu and (Altium parity) right-
// clicking on a pad / silk-graphic SELECTS that target. Bare-canvas
// right-click leaves the prior selection alone.
// ─────────────────────────────────────────────────────────────────

#[test]
fn v026b_show_context_menu_pad_target_selects_pad_and_clears_silk() {
    use signex_app::library::editor::footprint::state::FootprintContextTarget;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026b-pad-target", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        // Pre-state: a silk graphic is selected; no pad selected.
        editor.state.selected_pad = None;
        editor.state.selected_silk_f = Some(7);
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ShowContextMenu {
            x: 12.5,
            y: 7.0,
            target: FootprintContextTarget::Pad(2),
        }),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.selected_pad,
        Some(2),
        "Pad target must select the right-clicked pad (Altium parity)"
    );
    assert_eq!(
        editor.state.selected_silk_f, None,
        "Pad target must clear silk-graphic selection so Properties acts on the pad"
    );
    let menu = editor
        .state
        .context_menu
        .as_ref()
        .expect("context menu must be open");
    assert!(
        matches!(menu.target, FootprintContextTarget::Pad(2)),
        "menu target must carry the pad index"
    );
}

#[test]
fn v026d_show_context_menu_silk_target_selects_silk_and_clears_pad() {
    use signex_app::library::editor::footprint::state::FootprintContextTarget;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026d-silk-target", 1);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_silk_f = None;
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ShowContextMenu {
            x: -3.0,
            y: 4.0,
            target: FootprintContextTarget::SilkF(5),
        }),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.selected_silk_f,
        Some(5),
        "SilkF target must select the right-clicked silk graphic"
    );
    assert_eq!(
        editor.state.selected_pad, None,
        "SilkF target must clear pad selection so Delete acts on the silk graphic"
    );
    let menu = editor
        .state
        .context_menu
        .as_ref()
        .expect("context menu must be open");
    assert!(matches!(menu.target, FootprintContextTarget::SilkF(5)));
}

#[test]
fn v026b_show_context_menu_empty_target_preserves_selection_and_opens_menu() {
    use signex_app::library::editor::footprint::state::FootprintContextTarget;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, path) = fixture_footprint_with_pads("v026b-empty-target", 2);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(1);
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ShowContextMenu {
            x: 100.0,
            y: 100.0,
            target: FootprintContextTarget::Empty,
        }),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.state.selected_pad,
        Some(1),
        "Empty target must preserve the prior pad selection"
    );
    assert!(
        editor.state.context_menu.is_some(),
        "Empty target still opens the menu"
    );
}
