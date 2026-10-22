use super::*;
use crate::gerber_viewer::{GerberViewerState, GridUnit};

#[test]
fn adds_edits_reorders_and_deletes_grid_definitions() {
    let viewer = GerberViewerState::default();
    let mut editor = GerberGridEditorState::from_viewer(&viewer);
    editor.decimal_separator = ".".to_owned();
    let original_count = editor.catalog.len();

    editor.update(GerberGridEditorMessage::AddGrid);
    editor.update(GerberGridEditorMessage::SetSettingsUnit(GridUnit::Inch));
    editor.update(GerberGridEditorMessage::SettingsNameChanged(
        "Routing".to_owned(),
    ));
    editor.update(GerberGridEditorMessage::SettingsXChanged(
        "0.125".to_owned(),
    ));
    editor.update(GerberGridEditorMessage::SettingsYChanged("0.25".to_owned()));

    assert_eq!(editor.catalog.len(), original_count + 1);
    assert_eq!(
        editor.catalog[editor.selected_index].name.as_deref(),
        Some("Routing")
    );
    assert_eq!(editor.catalog[editor.selected_index].unit, GridUnit::Inch);

    editor.update(GerberGridEditorMessage::SettingsNameChanged(
        "Fine routing".to_owned(),
    ));
    assert_eq!(
        editor.catalog[editor.selected_index].name.as_deref(),
        Some("Fine routing"),
    );

    editor.update(GerberGridEditorMessage::MoveGridUp);
    assert_eq!(editor.selected_index, original_count - 1);
    editor.update(GerberGridEditorMessage::MoveGridDown);
    assert_eq!(editor.selected_index, original_count);
    editor.update(GerberGridEditorMessage::DeleteGrid);
    assert_eq!(editor.catalog.len(), original_count);
}

#[test]
fn invalid_inline_edit_blocks_selection_and_keeps_last_valid_grid() {
    let viewer = GerberViewerState::default();
    let mut editor = GerberGridEditorState::from_viewer(&viewer);
    editor.decimal_separator = ".".to_owned();
    let selected = editor.selected_index;
    let original_grid = editor.catalog[selected].clone();

    editor.update(GerberGridEditorMessage::SettingsXChanged(
        "unfinished".to_owned(),
    ));
    editor.update(GerberGridEditorMessage::SelectGrid(0));

    assert_eq!(editor.selected_index, selected);
    assert_eq!(editor.catalog[selected], original_grid);
    assert!(editor.error.is_some());

    editor.update(GerberGridEditorMessage::SettingsXChanged("1".to_owned()));
    editor.update(GerberGridEditorMessage::SelectGrid(0));
    assert_eq!(editor.selected_index, 0);
    assert_eq!(editor.error, None);
}
