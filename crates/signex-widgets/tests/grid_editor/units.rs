use crate::gerber_viewer::{GridUnit, create_grid_definition};

#[test]
fn grid_settings_offer_mm_mil_and_inch_units() {
    assert_eq!(
        GridUnit::ALL.map(|unit| unit.to_string()),
        ["mm", "mil", "inch"],
    );
}

#[test]
fn inch_grid_distances_convert_to_physical_millimetres() {
    let grid = create_grid_definition("Inch grid", "0.1", "0.05", GridUnit::Inch, ".")
        .expect("inch grid should parse");

    assert!((grid.x_millimetres() - 2.54).abs() < f64::EPSILON);
    assert!((grid.y_millimetres() - 1.27).abs() < f64::EPSILON);
    assert_eq!(
        grid.display_label("."),
        "Inch grid: 0.1000 inch ⨯ 0.0500 inch (2.5400 mm ⨯ 1.2700 mm)",
    );
}

#[test]
fn changing_editor_unit_preserves_physical_spacing() {
    let viewer = crate::gerber_viewer::GerberViewerState::default();
    let mut editor = super::GerberGridEditorState::from_viewer(&viewer);
    editor.decimal_separator = ".".to_owned();
    editor.update(super::GerberGridEditorMessage::AddGrid);
    editor.update(super::GerberGridEditorMessage::SettingsXChanged(
        "25.4".to_owned(),
    ));
    editor.update(super::GerberGridEditorMessage::SettingsYChanged(
        "12.7".to_owned(),
    ));

    editor.update(super::GerberGridEditorMessage::SetSettingsUnit(
        GridUnit::Inch,
    ));

    assert_eq!(editor.settings_x, "1");
    assert_eq!(editor.settings_y, "0.5");
    assert_eq!(editor.settings_unit, GridUnit::Inch);
}
