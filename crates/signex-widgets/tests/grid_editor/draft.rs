use super::*;
use crate::gerber_viewer::GerberViewerState;

#[test]
fn cancel_outcome_leaves_the_viewer_unchanged() {
    let viewer = GerberViewerState::default();
    let original_catalog = viewer.grid_catalog.clone();
    let original_index = viewer.active_grid_index;
    let mut editor = GerberGridEditorState::from_viewer(&viewer);

    editor.update(GerberGridEditorMessage::SelectGrid(0));
    editor.update(GerberGridEditorMessage::DeleteGrid);
    let outcome = editor.update(GerberGridEditorMessage::Cancel);

    assert_eq!(outcome, GerberGridEditorOutcome::Cancel);
    assert_eq!(viewer.grid_catalog, original_catalog);
    assert_eq!(viewer.active_grid_index, original_index);
}

#[test]
fn applying_is_an_explicit_editor_outcome() {
    let viewer = GerberViewerState::default();
    let mut editor = GerberGridEditorState::from_viewer(&viewer);

    assert_eq!(
        editor.update(GerberGridEditorMessage::Apply),
        GerberGridEditorOutcome::Apply,
    );
}
