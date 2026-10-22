use super::GerberGridEditorMessage;
use crate::gerber_viewer::{
    DEFAULT_GRID_INDEX, GerberViewerState, GridSizeChoice, GridSizePreset, GridUnit,
    default_grid_catalog, grid_size_choices, persist_grid_catalog,
};

#[derive(Debug, Clone)]
pub struct GerberGridEditorState {
    pub(super) catalog: Vec<GridSizePreset>,
    pub(super) selected_index: usize,
    pub(super) decimal_separator: String,
    pub(super) settings_name: String,
    pub(super) settings_x: String,
    pub(super) settings_y: String,
    pub(super) settings_unit: GridUnit,
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GerberGridEditorOutcome {
    None,
    Apply,
    Cancel,
}

impl GerberGridEditorState {
    pub fn from_viewer(viewer: &GerberViewerState) -> Self {
        let mut state = Self {
            catalog: viewer.grid_catalog.clone(),
            selected_index: viewer.active_grid_index,
            decimal_separator: viewer.decimal_separator.clone(),
            settings_name: String::new(),
            settings_x: String::new(),
            settings_y: String::new(),
            settings_unit: GridUnit::Mil,
            error: None,
        };
        state.load_selected_settings();
        state
    }

    pub fn update(&mut self, message: GerberGridEditorMessage) -> GerberGridEditorOutcome {
        match message {
            GerberGridEditorMessage::SelectGrid(index) => {
                self.select_grid(index);
            }
            GerberGridEditorMessage::AddGrid => {
                self.catalog.push(GridSizePreset {
                    name: Some("New grid".to_owned()),
                    x: 1.0,
                    y: 1.0,
                    unit: GridUnit::Mm,
                });
                self.selected_index = self.catalog.len() - 1;
                self.load_selected_settings();
            }
            GerberGridEditorMessage::SettingsNameChanged(value) => {
                self.settings_name = value;
                self.sync_selected_grid();
            }
            GerberGridEditorMessage::SettingsXChanged(value) => {
                self.settings_x = value;
                self.sync_selected_grid();
            }
            GerberGridEditorMessage::SettingsYChanged(value) => {
                self.settings_y = value;
                self.sync_selected_grid();
            }
            GerberGridEditorMessage::SetSettingsUnit(unit) => {
                self.set_settings_unit(unit);
            }
            GerberGridEditorMessage::DeleteGrid => {
                self.delete_selected_grid();
            }
            GerberGridEditorMessage::MoveGridUp => {
                self.move_selected_grid(-1);
            }
            GerberGridEditorMessage::MoveGridDown => {
                self.move_selected_grid(1);
            }
            GerberGridEditorMessage::ResetDefaults => {
                self.catalog = default_grid_catalog();
                self.selected_index = DEFAULT_GRID_INDEX.min(self.catalog.len() - 1);
                self.load_selected_settings();
            }
            GerberGridEditorMessage::Apply => {
                if self.sync_selected_grid() {
                    return GerberGridEditorOutcome::Apply;
                }
            }
            GerberGridEditorMessage::Cancel => {
                return GerberGridEditorOutcome::Cancel;
            }
        }

        GerberGridEditorOutcome::None
    }

    pub fn apply_to(&self, viewer: &mut GerberViewerState) -> Result<(), String> {
        persist_grid_catalog(&self.catalog)?;
        let changed =
            viewer.grid_catalog != self.catalog || viewer.active_grid_index != self.selected_index;
        viewer.grid_catalog = self.catalog.clone();
        viewer.active_grid_index = self.selected_index.min(viewer.grid_catalog.len() - 1);
        if changed {
            viewer.redraw_generation = viewer.redraw_generation.wrapping_add(1);
        }
        viewer.status = "Applied Gerber grid settings.".to_owned();
        Ok(())
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    pub(super) fn catalog_choices(&self) -> Vec<GridSizeChoice> {
        grid_size_choices(&self.catalog, &self.decimal_separator)
    }

    pub(super) fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub(super) fn settings_name(&self) -> &str {
        &self.settings_name
    }

    pub(super) fn x_value(&self) -> &str {
        &self.settings_x
    }

    pub(super) fn y_value(&self) -> &str {
        &self.settings_y
    }

    pub(super) fn settings_unit(&self) -> GridUnit {
        self.settings_unit
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
