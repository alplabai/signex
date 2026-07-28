//! `push_history` / `undo` / `redo` in isolation, no placement or geometry involved.

use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 1 (Track B) — footprint editor undo / redo
// ─────────────────────────────────────────────────────────────────

#[test]
fn footprint_editor_push_history_then_undo_restores_pads() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    let snapshot_before = editor.state.pads.clone();

    editor.push_history();
    // Mutate: append a pad.
    editor.state.pads.push(
        signex_app::library::editor::footprint::state::EditorPad::new_default(
            "1".into(),
            (0.0, 0.0),
        ),
    );
    assert_eq!(editor.state.pads.len(), snapshot_before.len() + 1);

    // Undo restores the pre-push state.
    let undone = editor.undo();
    assert!(undone, "undo must succeed when history is non-empty");
    assert_eq!(editor.state.pads.len(), snapshot_before.len());

    // Redo applies the mutation again.
    let redone = editor.redo();
    assert!(redone);
    assert_eq!(editor.state.pads.len(), snapshot_before.len() + 1);
}

#[test]
fn footprint_editor_undo_returns_false_on_empty_history() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    assert!(!editor.undo(), "fresh editor must have no undoable history");
    assert!(!editor.redo(), "fresh editor must have no redoable history");
}

#[test]
fn footprint_editor_history_caps_at_depth_limit() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    // Push twice as many entries as the cap.
    for _ in 0..(FootprintEditorState::HISTORY_DEPTH * 2) {
        editor.push_history();
    }
    assert_eq!(editor.history.len(), FootprintEditorState::HISTORY_DEPTH);
}

#[test]
fn footprint_editor_new_mutation_clears_redo_stack() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    editor.push_history();
    editor.state.pads.push(
        signex_app::library::editor::footprint::state::EditorPad::new_default(
            "1".into(),
            (0.0, 0.0),
        ),
    );
    editor.undo(); // moves snapshot to redo
    assert_eq!(editor.redo.len(), 1);

    // A fresh push_history must clear the redo stack so the
    // history stays a single timeline.
    editor.push_history();
    assert!(editor.redo.is_empty());
}
